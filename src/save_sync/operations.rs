// Atomic I/O operations for save synchronization
// Functions that interact with the filesystem

use crate::handler::Handler;
use crate::paths::PATH_PARTY;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::pure::{extract_steam_id_from_filename, get_profile_save_path};

/// Check if a profile already has save data for this handler
pub fn profile_has_existing_saves(profile_name: &str, h: &Handler) -> bool {
    let (profile_save_path, _) = get_profile_save_path(profile_name, h);
    if !profile_save_path.exists() {
        return false;
    }
    // Check if directory has any files (not just exists but empty)
    std::fs::read_dir(&profile_save_path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

/// Copy a directory recursively with Steam ID remapping in filenames
/// If original_steam_id is detected in a filename, it's replaced with target_steam_id
pub fn copy_dir_with_steam_id_remap(
    src: &PathBuf,
    dest: &PathBuf,
    target_steam_id: u64,
) -> Result<Option<u64>, Box<dyn Error>> {
    let mut detected_original_id: Option<u64> = None;

    let walk_path = walkdir::WalkDir::new(src).min_depth(1).follow_links(false);

    for entry in walk_path {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(src)?;

        // Remap EVERY path component that is a Steam ID — not just the leaf. This
        // handles both filename-keyed saves (DRG: "<id>_Player.sav") AND
        // directory-keyed saves (V Rising: "CloudSaves/<id>/...", Unreal:
        // "Saved/SaveGames/<id>/..."). The old leaf-only logic left the renamed
        // dir empty and recreated the original-id dir for its contents.
        let mut new_rel_path = PathBuf::new();
        for comp in rel_path.components() {
            let name = comp.as_os_str().to_string_lossy();
            if let Some((original_id, rest)) = extract_steam_id_from_filename(&name) {
                if detected_original_id.is_none() {
                    detected_original_id = Some(original_id);
                    println!("[splitux] Detected original Steam ID in saves: {}", original_id);
                }
                let remapped = format!("{}{}", target_steam_id, rest);
                println!("[splitux] Remapping save component: {} -> {}", name, remapped);
                new_rel_path.push(remapped);
            } else {
                new_rel_path.push(comp.as_os_str());
            }
        }

        let new_path = dest.join(&new_rel_path);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&new_path)?;
        } else if entry.file_type().is_symlink() {
            let symlink_src = std::fs::read_link(entry.path())?;
            std::os::unix::fs::symlink(symlink_src, &new_path)?;
        } else {
            if let Some(parent) = new_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if new_path.exists() {
                std::fs::remove_file(&new_path)?;
            }
            std::fs::copy(entry.path(), &new_path)?;
        }
    }

    Ok(detected_original_id)
}

/// Copy a directory recursively (standard copy without remapping)
pub fn copy_dir_recursive(src: &PathBuf, dest: &PathBuf) -> Result<(), Box<dyn Error>> {
    let walk_path = walkdir::WalkDir::new(src).min_depth(1).follow_links(false);

    for entry in walk_path {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(src)?;
        let new_path = dest.join(rel_path);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&new_path)?;
        } else if entry.file_type().is_symlink() {
            let symlink_src = std::fs::read_link(entry.path())?;
            std::os::unix::fs::symlink(symlink_src, new_path)?;
        } else {
            if let Some(parent) = new_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if new_path.exists() {
                std::fs::remove_file(&new_path)?;
            }
            std::fs::copy(entry.path(), new_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("splitux_savesync_test_{name}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![b'x'; bytes]).unwrap();
    }

    #[test]
    fn primary_save_size_ignores_bak_and_picks_largest() {
        let d = tmp("primary");
        write(&d.join("save_v3_0.json"), 1000);
        write(&d.join("save_v2.json"), 500);
        write(&d.join("save_v3_0_01.json.bak"), 9000); // bak must be ignored
        write(&d.join("run_v3_0.json"), 8000); // not a save* file
        assert_eq!(primary_save_size(&d), 1000);
    }

    #[test]
    fn canonical_folder_picks_most_progressed_not_first() {
        // Mirrors the incident: a real 400hr folder + stale low-progress folders.
        let d = tmp("canonical");
        write(&d.join("76561198035859048/save_v3_0.json"), 68299); // real
        write(&d.join("76561198743325131/save_v3_0.json"), 12000); // stale/spoofed
        write(&d.join("76561198344367125/save_v3_0.json"), 11000); // stale
        let (id, folder) = canonical_save_folder(&d).expect("a canonical folder");
        assert_eq!(id, 76561198035859048);
        assert!(folder.ends_with("76561198035859048"));
        // detect_original_steam_id must agree (no longer "first walked")
        assert_eq!(detect_original_steam_id(&d), Some(76561198035859048));
    }

    #[test]
    fn regression_proxy_flags_smaller_incoming() {
        // The guard's core comparison: incoming < original - 1% => regression.
        let orig = tmp("reg_orig");
        let incoming = tmp("reg_in");
        write(&orig.join("76561198035859048/save_v3_0.json"), 68299);
        write(&incoming.join("76561198154692317/save_v3_0.json"), 63685);
        let o = primary_save_size(&orig);
        let i = primary_save_size(&incoming);
        assert!(i < o.saturating_sub(o / 100), "63685 must trip the >1% regression guard vs 68299");
        // equal-or-greater must NOT trip it
        let grown = tmp("reg_grown");
        write(&grown.join("76561198154692317/save_v3_0.json"), 70000);
        let g = primary_save_size(&grown);
        assert!(!(g < o.saturating_sub(o / 100)), "a grown save must pass");
    }
}

/// Backup saves before overwriting
pub fn backup_saves(path: &PathBuf) -> Result<PathBuf, Box<dyn Error>> {
    let backup_base = PATH_PARTY.join("save_backups");
    std::fs::create_dir_all(&backup_base)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "saves".to_string());

    let backup_path = backup_base.join(format!("{}_{}", name, timestamp));

    println!("[splitux] Backing up: {}", backup_path.display());

    std::fs::create_dir_all(&backup_path)?;
    copy_dir_recursive(path, &backup_path)?;

    Ok(backup_path)
}

/// A coarse "how much progress is here" proxy for a save folder: the size of its
/// largest non-backup `save*.json` file. More progress generally means a larger
/// primary save (more unlocks/state). Used to (a) pick the canonical save folder
/// among several and (b) detect a regression before sync-back clobbers a save.
/// `.bak` rotations are excluded so a folder full of stale backups can't masquerade
/// as the most-progressed.
pub fn primary_save_size(dir: &Path) -> u64 {
    if !dir.exists() {
        return 0;
    }
    let mut best = 0u64;
    for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        // Primary save files only — skip rotated backups and run snapshots.
        let is_save = name.starts_with("save") && name.ends_with(".json") && !name.ends_with(".bak");
        if is_save {
            if let Ok(md) = entry.metadata() {
                best = best.max(md.len());
            }
        }
    }
    best
}

/// The most-progressed steam-id SUBFOLDER of `path`, as `(steam_id, folder)`.
///
/// A real save dir can accumulate several steam-id subfolders from prior sessions
/// (generated/spoofed ids). We score each by [`primary_save_size`] and return the
/// richest — overwhelmingly the user's real save — so callers never pick (or remap
/// onto) a stale low-progress folder. Returns `None` for flat / filename-keyed
/// layouts that have no steam-id subfolders.
pub fn canonical_save_folder(path: &Path) -> Option<(u64, PathBuf)> {
    if !path.exists() {
        return None;
    }
    let mut best: Option<(u64, PathBuf, u64)> = None; // (id, folder, weight)
    if let Ok(entries) = std::fs::read_dir(path) {
        for e in entries.filter_map(|e| e.ok()) {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if let Some((steam_id, rest)) = extract_steam_id_from_filename(&name) {
                if !rest.is_empty() {
                    continue; // folder name must BE a bare steam id
                }
                let weight = primary_save_size(&e.path());
                if best.as_ref().map(|(_, _, w)| weight > *w).unwrap_or(true) {
                    best = Some((steam_id, e.path(), weight));
                }
            }
        }
    }
    best.map(|(id, folder, _)| (id, folder))
}

/// Detect the original Steam ID from save files in a directory.
///
/// Prefers the most-progressed steam-id subfolder ([`canonical_save_folder`]);
/// falls back to a steam id embedded in any filename for flat / filename-keyed
/// layouts. Replaces the old "first steam id walked" which could pick a stale
/// spoofed-id folder and make sync-back remap onto the wrong save.
pub fn detect_original_steam_id(path: &PathBuf) -> Option<u64> {
    if !path.exists() {
        return None;
    }
    if let Some((id, _)) = canonical_save_folder(path) {
        return Some(id);
    }
    for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if let Some(filename) = entry.path().file_name().and_then(|f| f.to_str()) {
            if let Some((steam_id, _)) = extract_steam_id_from_filename(filename) {
                return Some(steam_id);
            }
        }
    }
    None
}

/// Copy an original save into a profile WITHOUT pulling in stale per-session
/// steam-id folders. When the original is organised as `<steam_id>/…` subfolders,
/// copy ONLY the canonical (most-progressed) one into `dest/<target_steam_id>/` —
/// the previous whole-dir remap merged every id folder into one, which could
/// corrupt the save (a contributor to the 400hr loss). Flat / filename-keyed
/// layouts fall back to the whole-dir remap.
pub fn copy_canonical_save_to_profile(
    original_path: &PathBuf,
    dest: &PathBuf,
    target_steam_id: u64,
) -> Result<Option<u64>, Box<dyn Error>> {
    if let Some((id, folder)) = canonical_save_folder(original_path) {
        let dest_folder = dest.join(target_steam_id.to_string());
        copy_dir_with_steam_id_remap(&folder, &dest_folder, target_steam_id)?;
        return Ok(Some(id));
    }
    copy_dir_with_steam_id_remap(original_path, dest, target_steam_id)
}
