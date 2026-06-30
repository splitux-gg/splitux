//! Overlay mounting operations

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::game_patches;
use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::PATH_PARTY;

/// Mount game directories with fuse-overlayfs
///
/// Creates overlay mounts for each instance with:
/// 1. Game patches overlay (if defined) - YAML-defined config file modifications
/// 2. Backend overlay (if enabled) - Goldberg DLLs or BepInEx files
/// 3. Handler overlay (if exists) - binary files from required_mods
/// 4. Base game directory - read-only game files
/// 5. Upper dir - per-profile save data (read-write)
pub fn fuse_overlayfs_mount_gamedirs(
    handlers: &[Handler],
    instances: &[Instance],
    backend_overlays: &[Vec<PathBuf>],
) -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = crate::paths::launch_tmp_dir();

    // Per-game patch overlay cache. Each game's handler may define its own
    // patches; apply them ONCE per game (not per instance), into a
    // game-namespaced dir so two games' patches never collide. Single-game uses
    // `game-patches-g0`.
    let mut patches_by_game: std::collections::HashMap<usize, Option<PathBuf>> =
        std::collections::HashMap::new();

    for (i, instance) in instances.iter().enumerate() {
        // This instance's unit handler (single-game: handlers[0]).
        let h = &handlers[instance.game];
        // Non-saved-handler games aren't overlay-mounted — they launch from their
        // real game root (matches the `gamedir` selection in build_cmds). Skip so
        // a mixed launch (one saved game + one not) only mounts the saved ones.
        if !h.is_saved_handler() {
            continue;
        }
        let game_root = h.get_game_rootpath()?;
        let game_root_path = Path::new(&game_root);
        let gamename = h.handler_dir_name().to_string();

        // Apply this game's patches the first time we see the game, then reuse.
        if let std::collections::hash_map::Entry::Vacant(e) = patches_by_game.entry(instance.game) {
            let computed = if !h.game_patches.is_empty() {
                let patches_dir = tmp_dir.join(format!("game-patches-g{}", instance.game));
                // Clear any previous patches for this game.
                if patches_dir.exists() {
                    std::fs::remove_dir_all(&patches_dir)?;
                }
                std::fs::create_dir_all(&patches_dir)?;
                game_patches::apply_game_patches(game_root_path, &patches_dir, &h.game_patches)?;
                Some(patches_dir)
            } else {
                None
            };
            e.insert(computed);
        }
        let patches_overlay = &patches_by_game[&instance.game];

        // Build lowerdir stack (leftmost has highest priority)
        let mut lowerdir_parts: Vec<String> = Vec::new();

        // 1. Game patches overlay first (highest priority)
        if let Some(patches_dir) = patches_overlay {
            lowerdir_parts.push(patches_dir.display().to_string());
        }

        // 2. Backend overlays (Goldberg DLLs, BepInEx files, etc.)
        if let Some(overlays) = backend_overlays.get(i) {
            for overlay in overlays {
                lowerdir_parts.push(overlay.display().to_string());
            }
        }

        // 3. Handler overlay for required_mods binary files (if exists)
        let handler_overlay = h.path_handler.join("overlay");
        if handler_overlay.exists() {
            lowerdir_parts.push(handler_overlay.display().to_string());
        }

        // 4. Base game directory (lowest priority)
        lowerdir_parts.push(game_root.clone());

        let path_lowerdir = lowerdir_parts.join(":");

        let path_game_mnt = tmp_dir.join(format!("game-{}", i));
        let path_workdir = tmp_dir.join(format!("work-{}", i));
        let path_prof = PATH_PARTY.join("profiles").join(&instance.profname);
        let path_upperdir = path_prof.join("gamesaves").join(&gamename);

        // Self-heal: a previous session that was force-killed (not torn down)
        // can leave game-N as a stale mount. Mounting over it fails and strands the
        // launch — so unmount any stale mount here before remounting. Non-blocking
        // (timeout-guarded) and handles both kernel-overlay and fuse mounts.
        if crate::util::is_mount_point(&path_game_mnt).unwrap_or(false) {
            println!("[splitux] Clearing stale mount at {}", path_game_mnt.display());
            crate::util::unmount_best_effort(&path_game_mnt);
        }

        std::fs::create_dir_all(&path_game_mnt)?;
        std::fs::create_dir_all(&path_workdir)?;
        std::fs::create_dir_all(&path_upperdir)?; // kernel overlay requires upperdir to exist

        println!(
            "[splitux] Mounting overlay for instance {}: lowerdir={}",
            i, path_lowerdir
        );

        // Prefer KERNEL overlayfs over fuse-overlayfs: it has NO userspace daemon,
        // so the game's asset I/O can't stall in uninterruptible D-state and hang
        // the GPU pipeline. (A fuse-overlayfs daemon stalling under concurrent
        // multi-instance load while a game thread held a GPU fence is what
        // hard-locked the host — the "Fence fallback timer" storm.) Kernel overlay
        // is also faster (no userspace round-trip per read). Requires passwordless
        // sudo (same model as netns) and upperdir+workdir on one fs (guaranteed —
        // both live under PATH_PARTY). Falls back to fuse-overlayfs otherwise.
        if !mount_overlay_kernel(&path_lowerdir, &path_upperdir, &path_workdir, &path_game_mnt) {
            mount_overlay_fuse(&path_lowerdir, &path_upperdir, &path_workdir, &path_game_mnt)?;
        }
    }

    Ok(())
}

/// Mount a kernel overlayfs via passwordless sudo. Returns true on success, false
/// if sudo/kernel overlay is unavailable so the caller can fall back to
/// fuse-overlayfs. `timeout(1)` guards against any mount hang.
fn mount_overlay_kernel(lowerdir: &str, upperdir: &Path, workdir: &Path, target: &Path) -> bool {
    // index=off: splitux's goldberg-overlay lowerdir path is unique per launch, so
    // the kernel's upper-root-origin verification (the `index` feature) ESTALEs on a
    // profile's 2nd+ reuse ("failed to verify upper root origin"). Disabling index
    // skips that check — the persistent save upperdir is reused across launches with
    // a changing lowerdir, which is exactly what index forbids. redirect_dir=off for
    // the same reuse-safety.
    let opts = format!(
        "lowerdir={lowerdir},upperdir={},workdir={},index=off,redirect_dir=off",
        upperdir.display(),
        workdir.display()
    );
    let status = Command::new("timeout")
        .args(["20", "sudo", "-n", "mount", "-t", "overlay", "overlay", "-o"])
        .arg(&opts)
        .arg(target)
        .status();
    match status {
        Ok(s) if s.success() => {
            println!(
                "[splitux] kernel overlayfs mounted at {} (no FUSE daemon)",
                target.display()
            );
            true
        }
        _ => {
            println!(
                "[splitux] kernel overlayfs unavailable (sudo -n mount failed) — falling back to fuse-overlayfs"
            );
            false
        }
    }
}

/// Mount fuse-overlayfs (userspace fallback when kernel overlay is unavailable).
fn mount_overlay_fuse(
    lowerdir: &str,
    upperdir: &Path,
    workdir: &Path,
    target: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new("fuse-overlayfs");
    cmd.arg("-o").arg(format!("lowerdir={lowerdir}"));
    cmd.arg("-o").arg(format!("upperdir={}", upperdir.display()));
    cmd.arg("-o").arg(format!("workdir={}", workdir.display()));
    cmd.arg(target);
    let status = cmd.status().map_err(|_| {
        "Fuse-overlayfs executable not found; Please install fuse-overlayfs through your distro's package manager."
    })?;
    if !status.success() {
        return Err("fuse-overlayfs mount failed.".into());
    }
    Ok(())
}
