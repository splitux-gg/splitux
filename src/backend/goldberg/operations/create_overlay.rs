//! Goldberg overlay creation
//!
//! Creates per-instance overlay directories with Goldberg DLLs and configuration.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::backend::operations::prepare_overlay_dir;
use crate::paths::PATH_RES;

use super::super::types::{GoldbergConfig, SteamApiDll, SteamDllType};
use super::write_settings::write_steam_settings;

/// Create a Goldberg overlay for a single game instance
///
/// This creates a directory structure that mirrors the game's Steam API DLL locations,
/// replacing them with Goldberg's DLLs and adding steam_settings configuration.
/// The overlay is meant to be used as a lowerdir in fuse-overlayfs.
pub fn create_instance_overlay(
    instance_idx: usize,
    dlls: &[SteamApiDll],
    config: &GoldbergConfig,
    is_windows: bool,
    handler_settings: &HashMap<String, String>,
    disable_networking: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let overlay_dir = prepare_overlay_dir("goldberg", instance_idx)?;

    for dll in dlls {
        let dll_dir = dll.rel_path.parent().unwrap_or(Path::new(""));
        let target_dir = overlay_dir.join(dll_dir);
        fs::create_dir_all(&target_dir)?;

        // Original DLL name (what the game expects)
        let dll_name = dll.rel_path.file_name().unwrap();

        // Determine source path based on DLL type, platform, and bitness
        let src_path = match dll.dll_type {
            SteamDllType::SteamApi => {
                // Standard Steam API: use dll_name directly from goldberg/{platform}
                let goldberg_src = if is_windows {
                    PATH_RES.join("goldberg/win")
                } else if dll.is_64bit {
                    PATH_RES.join("goldberg/linux64")
                } else {
                    PATH_RES.join("goldberg/linux32")
                };
                goldberg_src.join(dll_name)
            }
            SteamDllType::NetworkingSockets => {
                // GameNetworkingSockets.dll -> libsteamnetworkingsockets.dll
                let arch = if dll.is_64bit { "x64" } else { "x32" };
                PATH_RES.join(format!(
                    "goldberg/steamnetworkingsockets/{}/libsteamnetworkingsockets.dll",
                    arch
                ))
            }
        };

        let dest_path = target_dir.join(dll_name);

        if src_path.exists() {
            fs::copy(&src_path, &dest_path)?;
            println!(
                "[splitux] Goldberg overlay {}: {} -> {}",
                instance_idx,
                src_path.display(),
                dest_path.display()
            );
        } else {
            println!(
                "[splitux] Warning: Goldberg DLL not found: {}",
                src_path.display()
            );
        }

        // Create steam_settings next to DLL
        let settings_dir = target_dir.join("steam_settings");
        fs::create_dir_all(&settings_dir)?;
        write_steam_settings(&settings_dir, config, handler_settings, disable_networking)?;
    }

    // For native Linux games, also create steam_settings and steam_appid.txt at overlay root
    // Goldberg looks for config next to the executable, not just next to the .so
    if !is_windows {
        let root_settings_dir = overlay_dir.join("steam_settings");
        if !root_settings_dir.exists() {
            fs::create_dir_all(&root_settings_dir)?;
            write_steam_settings(
                &root_settings_dir,
                config,
                handler_settings,
                disable_networking,
            )?;
            println!(
                "[splitux] Goldberg overlay {}: Also created steam_settings at game root",
                instance_idx
            );
        }
        // Also create steam_appid.txt at game root (some games need this)
        fs::write(
            overlay_dir.join("steam_appid.txt"),
            config.app_id.to_string(),
        )?;
    }

    println!(
        "[splitux] Goldberg overlay {} created: Steam ID {}, Port {}, Broadcasts: {:?}, disable_networking: {}",
        instance_idx, config.steam_id, config.listen_port, config.broadcast_ports, disable_networking
    );

    Ok(overlay_dir)
}
