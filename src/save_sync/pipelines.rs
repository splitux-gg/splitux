// Orchestration pipelines for save synchronization
// Combines pure logic and operations to accomplish higher-level tasks

use crate::handler::Handler;
use crate::instance::Instance;
use crate::profiles::generate_steam_id;
use std::error::Error;

use super::operations::{
    backup_saves, copy_dir_recursive, copy_dir_with_steam_id_remap, detect_original_steam_id,
    profile_has_existing_saves,
};
use super::pure::{find_first_named_profile, get_original_save_path, get_profile_save_path};

/// Copy original saves to a profile
/// For named profiles: skips if profile already has saves (preserves existing progress)
/// For guest profiles (starting with '.'): always copies fresh from original
pub fn copy_original_saves_to_profile(
    h: &Handler,
    profile_name: &str,
) -> Result<(), Box<dyn Error>> {
    let original_path = match get_original_save_path(h) {
        Some(p) => p,
        None => return Ok(()),
    };

    // Guest profiles always get fresh copies, named profiles preserve existing saves
    let is_guest = profile_name.starts_with('.');
    if !is_guest && profile_has_existing_saves(profile_name, h) {
        println!(
            "[splitux] Profile '{}' already has saves, skipping copy (preserving existing progress)",
            profile_name
        );
        return Ok(());
    }

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

/// Sync master profile from original - always overwrites existing profile saves
/// This is called at session start to ensure master has latest PC saves
/// Both the original and profile saves are backed up before any modifications
fn sync_master_from_original(h: &Handler, master: &str) -> Result<(), Box<dyn Error>> {
    let original_path = match get_original_save_path(h) {
        Some(p) => p,
        None => return Ok(()),
    };

    if !original_path.exists() {
        println!(
            "[splitux] Original save path does not exist: {}",
            original_path.display()
        );
        return Ok(());
    }

    let (profile_save_path, is_game_dir) = get_profile_save_path(master, h);

    println!(
        "[splitux] Syncing master '{}' from original: {} -> {} {}",
        master,
        original_path.display(),
        profile_save_path.display(),
        if is_game_dir { "(game dir overlay)" } else { "" }
    );

    // Backup original saves (the machine's save) before any operation
    if let Err(e) = backup_saves(&original_path) {
        println!(
            "[splitux] Warning: Failed to backup original saves: {}",
            e
        );
        // Continue anyway - backup failure shouldn't block the sync
    }

    // Backup profile saves before overwriting (preserves any unsaved progress)
    if profile_save_path.exists() {
        // Check if profile has actual content to backup
        let has_content = std::fs::read_dir(&profile_save_path)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false);

        if has_content {
            if let Err(e) = backup_saves(&profile_save_path) {
                println!(
                    "[splitux] Warning: Failed to backup profile saves: {}",
                    e
                );
            }
        }
    }

    // Clear existing and copy fresh
    if profile_save_path.exists() {
        std::fs::remove_dir_all(&profile_save_path)?;
    }
    std::fs::create_dir_all(&profile_save_path)?;

    if h.save_steam_id_remap {
        let profile_steam_id = generate_steam_id(master);
        copy_dir_with_steam_id_remap(&original_path, &profile_save_path, profile_steam_id)?;
    } else {
        copy_dir_recursive(&original_path, &profile_save_path)?;
    }

    Ok(())
}

/// Copy saves from one profile to another profile
/// Handles Steam ID remapping if enabled
fn copy_profile_saves_to_profile(
    h: &Handler,
    source_profile: &str,
    target_profile: &str,
) -> Result<(), Box<dyn Error>> {
    let (source_path, _) = get_profile_save_path(source_profile, h);
    let (target_path, _) = get_profile_save_path(target_profile, h);

    if !source_path.exists() {
        println!(
            "[splitux] Source profile '{}' has no saves to copy",
            source_profile
        );
        return Ok(());
    }

    // Check for non-empty source
    let has_content = std::fs::read_dir(&source_path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    if !has_content {
        return Ok(());
    }

    println!(
        "[splitux] Inheriting saves: {} -> {}",
        source_profile, target_profile
    );

    std::fs::create_dir_all(&target_path)?;

    if h.save_steam_id_remap {
        let target_steam_id = generate_steam_id(target_profile);
        copy_dir_with_steam_id_remap(&source_path, &target_path, target_steam_id)?;
    } else {
        copy_dir_recursive(&source_path, &target_path)?;
    }
    Ok(())
}

/// Initialize profile saves using master-based inheritance
///
/// Flow:
/// 1. If master profile is set and has no saves -> copy from original to master
/// 2. For each instance:
///    - Guest profiles -> always fresh copy from master (or original if no master)
///    - Named profiles with no saves -> inherit from master (or original if no master)
///    - Named profiles with saves -> keep existing (no copy)
pub fn initialize_profile_saves(
    h: &Handler,
    instances: &[Instance],
    master_profile: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if h.original_save_path.is_empty() {
        return Ok(());
    }

    println!(
        "[splitux] Initializing profile saves (master: {:?})...",
        master_profile
    );

    // Step 1: Master profile ALWAYS syncs from original at session start
    // This ensures master always has the latest PC saves
    if let Some(master) = master_profile {
        if let Err(e) = sync_master_from_original(h, master) {
            println!("[splitux] Warning: Failed to sync master from original: {}", e);
        }
    }

    // Step 2: Initialize each instance's profile
    for instance in instances {
        let is_guest = instance.profname.starts_with('.');
        let is_master = master_profile == Some(instance.profname.as_str());

        if is_master {
            // Master was already handled above
            continue;
        }

        if is_guest {
            // Guest profiles always get fresh copies
            if let Some(master) = master_profile {
                // Copy from master
                if let Err(e) = copy_profile_saves_to_profile(h, master, &instance.profname) {
                    println!(
                        "[splitux] Warning: Failed to copy saves to guest '{}': {}",
                        instance.profname, e
                    );
                }
            } else {
                // No master, copy from original
                // Need to clear any existing guest saves first
                let (guest_path, _) = get_profile_save_path(&instance.profname, h);
                if guest_path.exists() {
                    let _ = std::fs::remove_dir_all(&guest_path);
                }
                if let Err(e) = copy_original_saves_to_profile(h, &instance.profname) {
                    println!(
                        "[splitux] Warning: Failed to copy saves to guest '{}': {}",
                        instance.profname, e
                    );
                }
            }
        } else if !profile_has_existing_saves(&instance.profname, h) {
            // Named profile with no saves - inherit from master or original
            if let Some(master) = master_profile {
                if let Err(e) = copy_profile_saves_to_profile(h, master, &instance.profname) {
                    println!(
                        "[splitux] Warning: Failed to inherit saves for '{}': {}",
                        instance.profname, e
                    );
                }
            } else {
                if let Err(e) = copy_original_saves_to_profile(h, &instance.profname) {
                    println!(
                        "[splitux] Warning: Failed to setup saves for '{}': {}",
                        instance.profname, e
                    );
                }
            }
        } else {
            println!(
                "[splitux] Profile '{}' already has saves, keeping existing",
                instance.profname
            );
        }
    }

    Ok(())
}

/// Sync master profile's saves back to original location after game ends
///
/// Only syncs if:
/// - save_sync_back is enabled
/// - original_save_path is set
/// - master profile participated in the session
pub fn sync_master_saves_back(
    h: &Handler,
    instances: &[Instance],
    master_profile: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if !h.save_sync_back || h.original_save_path.is_empty() {
        return Ok(());
    }

    let master = match master_profile {
        Some(m) => m,
        None => {
            // No master designated - fall back to first named profile (legacy behavior)
            return sync_saves_back(h, instances);
        }
    };

    // Check if master profile participated in this session
    if !instances.iter().any(|i| i.profname == master) {
        println!(
            "[splitux] Master profile '{}' not in session, skipping sync back",
            master
        );
        return Ok(());
    }

    let original_path = match get_original_save_path(h) {
        Some(p) => p,
        None => return Ok(()),
    };

    let (profile_save_path, _) = get_profile_save_path(master, h);

    if !profile_save_path.exists() {
        println!(
            "[splitux] Master profile saves not found: {}",
            profile_save_path.display()
        );
        return Ok(());
    }

    println!(
        "[splitux] Syncing master '{}' back to original: {}",
        master,
        original_path.display()
    );

    // Detect original Steam ID before we modify anything (for remapping back)
    let original_steam_id = if h.save_steam_id_remap {
        detect_original_steam_id(&original_path)
    } else {
        None
    };

    // Backup master profile saves before sync (preserves session progress)
    if let Err(e) = backup_saves(&profile_save_path) {
        println!("[splitux] Warning: Failed to backup master profile: {}", e);
    }

    // Backup original saves before overwriting
    if original_path.exists() {
        if let Err(e) = backup_saves(&original_path) {
            println!("[splitux] Warning: Failed to backup original: {}", e);
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
            println!(
                "[splitux] Remapping saves back to original Steam ID: {}",
                target_steam_id
            );
            copy_dir_with_steam_id_remap(&profile_save_path, &original_path, target_steam_id)?;
        } else {
            println!("[splitux] No original Steam ID detected, copying without remap");
            copy_dir_recursive(&profile_save_path, &original_path)?;
        }
    } else {
        copy_dir_recursive(&profile_save_path, &original_path)?;
    }

    println!("[splitux] Master sync complete");

    Ok(())
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
            println!("[splitux] No original Steam ID detected, copying without remap");
            copy_dir_recursive(&profile_save_path, &original_path)?;
        }
    } else {
        copy_dir_recursive(&profile_save_path, &original_path)?;
    }

    println!("[splitux] Sync complete");

    Ok(())
}
