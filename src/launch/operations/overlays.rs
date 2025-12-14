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
    h: &Handler,
    instances: &Vec<Instance>,
    backend_overlays: &[Vec<PathBuf>],
) -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = PATH_PARTY.join("tmp");
    let game_root = h.get_game_rootpath()?;
    let game_root_path = Path::new(&game_root);
    let gamename = h.handler_dir_name().to_string();

    // Apply game patches if defined (creates patched files in temp overlay)
    let patches_overlay = if !h.game_patches.is_empty() {
        let patches_dir = tmp_dir.join("game-patches");
        std::fs::create_dir_all(&patches_dir)?;

        // Clear previous patches
        if patches_dir.exists() {
            std::fs::remove_dir_all(&patches_dir)?;
            std::fs::create_dir_all(&patches_dir)?;
        }

        game_patches::apply_game_patches(game_root_path, &patches_dir, &h.game_patches)?;
        Some(patches_dir)
    } else {
        None
    };

    for (i, instance) in instances.iter().enumerate() {
        // Build lowerdir stack (leftmost has highest priority)
        let mut lowerdir_parts: Vec<String> = Vec::new();

        // 1. Game patches overlay first (highest priority)
        if let Some(ref patches_dir) = patches_overlay {
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

        std::fs::create_dir_all(&path_game_mnt)?;
        std::fs::create_dir_all(&path_workdir)?;

        let mut cmd = Command::new("fuse-overlayfs");
        cmd.arg("-o");
        cmd.arg(format!("lowerdir={}", path_lowerdir));
        cmd.arg("-o");
        cmd.arg(format!("upperdir={}", path_upperdir.display()));
        cmd.arg("-o");
        cmd.arg(format!("workdir={}", path_workdir.display()));
        cmd.arg(&path_game_mnt);

        println!(
            "[splitux] Mounting overlay for instance {}: lowerdir={}",
            i, path_lowerdir
        );

        let status = cmd
            .status()
            .map_err(|_| "Fuse-overlayfs executable not found; Please install fuse-overlayfs through your distro's package manager.")?;
        if !status.success() {
            return Err("fuse-overlayfs mount failed.".into());
        }
    }

    Ok(())
}
