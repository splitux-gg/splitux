// Atomic I/O operations for save synchronization
// Functions that interact with the filesystem

use crate::handler::Handler;
use crate::paths::PATH_PARTY;
use std::error::Error;
use std::path::PathBuf;
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

        // Check if filename has a Steam ID prefix
        let filename = entry
            .path()
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        let (new_filename, detected_id) = if let Some((original_id, rest)) =
            extract_steam_id_from_filename(&filename)
        {
            // Store the detected original ID (use first one found)
            if detected_original_id.is_none() {
                detected_original_id = Some(original_id);
                println!(
                    "[splitux] Detected original Steam ID in saves: {}",
                    original_id
                );
            }

            let remapped = format!("{}{}", target_steam_id, rest);
            println!(
                "[splitux] Remapping save file: {} -> {}",
                filename, remapped
            );
            (remapped, Some(original_id))
        } else {
            (filename.clone(), None)
        };

        // Build the new path with potentially remapped filename
        let new_rel_path = if detected_id.is_some() {
            rel_path.parent().map_or_else(
                || PathBuf::from(&new_filename),
                |parent| parent.join(&new_filename),
            )
        } else {
            rel_path.to_path_buf()
        };

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

/// Detect the original Steam ID from save files in a directory
pub fn detect_original_steam_id(path: &PathBuf) -> Option<u64> {
    if !path.exists() {
        return None;
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
