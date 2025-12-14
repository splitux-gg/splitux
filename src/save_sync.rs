// Save game synchronization module
// Handles copying original saves to profiles and syncing back after sessions
//
// User provides: original_save_path (full path to saves)
// We auto-detect:
//   - If inside game directory → copy to gamesaves/{handler}/{relative}
//   - If under HOME → copy to home/{relative}
//   - If Windows AppData style → copy to windata/{path}
//
// Steam ID Remapping:
//   Some games (like DRG) tie save files to Steam IDs by embedding the ID in filenames.
//   When using Goldberg, each profile gets a unique Steam ID. We detect save files with
//   Steam ID prefixes and remap them to match the profile's Goldberg Steam ID.

use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::{PATH_HOME, PATH_PARTY};
use crate::profiles::generate_steam_id;
use std::error::Error;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use regex::Regex;

/// Expand ~ and $HOME in path
fn expand_path(path: &str) -> PathBuf {
    let mut s = path.to_string();
    if s.starts_with("~/") {
        s = s.replacen("~", &PATH_HOME.to_string_lossy(), 1);
    }
    s = s.replace("$HOME", &PATH_HOME.to_string_lossy());
    PathBuf::from(s)
}

/// Get the game root directory from handler
fn get_game_root(h: &Handler) -> Option<PathBuf> {
    if !h.path_gameroot.is_empty() {
        return Some(PathBuf::from(&h.path_gameroot));
    }
    // Game root is resolved elsewhere for steam_appid games
    // The handler should have path_gameroot populated by launch time
    None
}

/// Get handler directory name (used for gamesaves subdir)
fn get_handler_name(h: &Handler) -> String {
    PathBuf::from(&h.path_handler)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Determine where to copy saves in the profile
/// Returns (profile_save_path, is_inside_game_dir)
fn get_profile_save_path(profile_name: &str, h: &Handler) -> (PathBuf, bool) {
    let profile_path = PATH_PARTY.join("profiles").join(profile_name);
    let original = expand_path(&h.original_save_path);
    let handler_name = get_handler_name(h);

    // Check if save path is inside game directory
    if let Some(game_root) = get_game_root(h) {
        if let Ok(relative) = original.strip_prefix(&game_root) {
            // Saves are inside game dir → goes to gamesaves overlay upperdir
            let dest = profile_path
                .join("gamesaves")
                .join(&handler_name)
                .join(relative);
            return (dest, true);
        }
    }

    // Check if under HOME (Linux native games)
    if let Ok(relative) = original.strip_prefix(&*PATH_HOME) {
        let dest = profile_path.join("home").join(relative);
        return (dest, false);
    }

    // For Windows games or other paths, use windata if it looks like AppData
    if h.win() || h.original_save_path.contains("AppData") {
        // Keep the relative structure for windata
        let dest = profile_path.join("windata").join(&h.original_save_path);
        return (dest, false);
    }

    // Fallback: put in gamesaves
    let dest = profile_path.join("gamesaves").join(&handler_name);
    (dest, false)
}

/// Get the original save path (just expand variables)
pub fn get_original_save_path(h: &Handler) -> Option<PathBuf> {
    if h.original_save_path.is_empty() {
        return None;
    }
    Some(expand_path(&h.original_save_path))
}

/// Steam64 ID regex pattern - matches 17-digit Steam IDs starting with 7656119
/// Format: 76561197960265728 + account_id (0 to ~4 billion)
fn steam_id_regex() -> Regex {
    Regex::new(r"^(7656119\d{10})(.*)$").unwrap()
}

/// Detect if a filename has a Steam ID prefix
/// Returns Some((steam_id, rest_of_filename)) if detected
fn extract_steam_id_from_filename(filename: &str) -> Option<(u64, String)> {
    let re = steam_id_regex();
    if let Some(caps) = re.captures(filename) {
        if let (Some(id_match), Some(rest_match)) = (caps.get(1), caps.get(2)) {
            if let Ok(steam_id) = id_match.as_str().parse::<u64>() {
                return Some((steam_id, rest_match.as_str().to_string()));
            }
        }
    }
    None
}

/// Copy a directory recursively with Steam ID remapping in filenames
/// If original_steam_id is detected in a filename, it's replaced with target_steam_id
fn copy_dir_with_steam_id_remap(
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
fn copy_dir_recursive(src: &PathBuf, dest: &PathBuf) -> Result<(), Box<dyn Error>> {
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

/// Copy original saves to a profile
pub fn copy_original_saves_to_profile(
    h: &Handler,
    profile_name: &str,
) -> Result<(), Box<dyn Error>> {
    let original_path = match get_original_save_path(h) {
        Some(p) => p,
        None => return Ok(()),
    };

    if !original_path.exists() {
        println!(
            "[splitux] Save path does not exist (first run?): {}",
            original_path.display()
        );
        return Ok(());
    }

    let (profile_save_path, is_game_dir) = get_profile_save_path(profile_name, h);

    println!(
        "[splitux] Copying saves: {} -> {} {}",
        original_path.display(),
        profile_save_path.display(),
        if is_game_dir { "(game dir overlay)" } else { "" }
    );

    std::fs::create_dir_all(&profile_save_path)?;

    if h.save_steam_id_remap {
        // Use Steam ID remapping - replace original Steam ID with profile's Goldberg Steam ID
        let profile_steam_id = generate_steam_id(profile_name);
        println!(
            "[splitux] Steam ID remap enabled for profile '{}' (ID: {})",
            profile_name, profile_steam_id
        );
        copy_dir_with_steam_id_remap(&original_path, &profile_save_path, profile_steam_id)?;
    } else {
        copy_dir_recursive(&original_path, &profile_save_path)?;
    }

    Ok(())
}

/// Copy original saves to all profiles
pub fn copy_original_saves_to_all_profiles(
    h: &Handler,
    instances: &[Instance],
) -> Result<(), Box<dyn Error>> {
    if h.original_save_path.is_empty() {
        return Ok(());
    }

    println!("[splitux] Copying original saves to profiles...");

    for instance in instances {
        if let Err(e) = copy_original_saves_to_profile(h, &instance.profname) {
            println!(
                "[splitux] Warning: Failed to copy saves to '{}': {}",
                instance.profname, e
            );
        }
    }

    Ok(())
}

/// Find first named (non-guest) profile
pub fn find_first_named_profile(instances: &[Instance]) -> Option<&str> {
    instances
        .iter()
        .find(|i| !i.profname.starts_with('.'))
        .map(|i| i.profname.as_str())
}

/// Backup saves before overwriting
fn backup_saves(path: &PathBuf) -> Result<PathBuf, Box<dyn Error>> {
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
fn detect_original_steam_id(path: &PathBuf) -> Option<u64> {
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

/// Sync saves from first named profile back to original location
pub fn sync_saves_back(h: &Handler, instances: &[Instance]) -> Result<(), Box<dyn Error>> {
    if !h.save_sync_back || h.original_save_path.is_empty() {
        return Ok(());
    }

    let profile_name = match find_first_named_profile(instances) {
        Some(name) => name,
        None => {
            println!("[splitux] No named profiles, skipping sync back");
            return Ok(());
        }
    };

    let original_path = match get_original_save_path(h) {
        Some(p) => p,
        None => return Ok(()),
    };

    let (profile_save_path, _) = get_profile_save_path(profile_name, h);

    if !profile_save_path.exists() {
        println!(
            "[splitux] Profile saves not found: {}",
            profile_save_path.display()
        );
        return Ok(());
    }

    println!(
        "[splitux] Syncing back: {} -> {}",
        profile_save_path.display(),
        original_path.display()
    );

    // Detect original Steam ID before we modify anything (for remapping back)
    let original_steam_id = if h.save_steam_id_remap {
        detect_original_steam_id(&original_path)
    } else {
        None
    };

    // Always backup before overwriting
    if original_path.exists() {
        if let Err(e) = backup_saves(&original_path) {
            println!("[splitux] Warning: Backup failed: {}", e);
        }
    }

    // Clear and copy
    if original_path.exists() {
        for entry in std::fs::read_dir(&original_path)? {
            let p = entry?.path();
            if p.is_dir() {
                std::fs::remove_dir_all(&p)?;
            } else {
                std::fs::remove_file(&p)?;
            }
        }
    } else {
        std::fs::create_dir_all(&original_path)?;
    }

    if h.save_steam_id_remap {
        if let Some(target_steam_id) = original_steam_id {
            // Remap profile's Goldberg Steam ID back to original user's Steam ID
            println!(
                "[splitux] Remapping saves back to original Steam ID: {}",
                target_steam_id
            );
            copy_dir_with_steam_id_remap(&profile_save_path, &original_path, target_steam_id)?;
        } else {
            // No original Steam ID found - copy without remapping
            // This happens on first run when there are no original saves
            println!(
                "[splitux] No original Steam ID detected, copying without remap"
            );
            copy_dir_recursive(&profile_save_path, &original_path)?;
        }
    } else {
        copy_dir_recursive(&profile_save_path, &original_path)?;
    }

    println!("[splitux] Sync complete");

    Ok(())
}
