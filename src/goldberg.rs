//! Goldberg Steam Emulator integration
//!
//! This module handles finding Steam API DLLs in game directories and creating
//! per-instance overlays with Goldberg's replacement DLLs and configuration.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::paths::{PATH_PARTY, PATH_RES};

/// Information about a Steam API DLL found in the game directory
#[derive(Debug, Clone)]
pub struct SteamApiDll {
    /// Relative path from game root to the DLL
    pub rel_path: PathBuf,
    /// True for 64-bit, false for 32-bit
    pub is_64bit: bool,
}

/// Configuration for a Goldberg instance
#[derive(Debug, Clone)]
pub struct GoldbergConfig {
    pub app_id: u32,
    pub steam_id: u64,
    pub account_name: String,
    pub listen_port: u16,
    /// Ports of other instances for LAN discovery
    pub broadcast_ports: Vec<u16>,
}

/// Detect whether a Steam API DLL is 64-bit based on filename and directory hints
fn detect_bitness(path: &Path, filename: &str) -> bool {
    // 1. Check filename first (most reliable)
    if filename == "steam_api64.dll" {
        return true;
    }
    if filename == "steam_api.dll" {
        // 2. Check directory structure for hints
        let path_str = path.to_string_lossy().to_lowercase();
        if path_str.contains("win64")
            || path_str.contains("x64")
            || path_str.contains("x86_64")
            || path_str.contains("/64/")
        {
            return true;
        }
        // Default to 32-bit for steam_api.dll
        return false;
    }
    // Linux: libsteam_api.so - check directory
    let path_str = path.to_string_lossy().to_lowercase();
    path_str.contains("64") || path_str.contains("x86_64")
}

/// Find all Steam API DLLs in the game directory
///
/// Searches recursively for steam_api.dll, steam_api64.dll, and libsteam_api.so
/// Returns relative paths from the game root along with bitness information
pub fn find_steam_api_dlls(game_dir: &Path) -> Result<Vec<SteamApiDll>, Box<dyn std::error::Error>> {
    let mut dlls = Vec::new();

    for entry in WalkDir::new(game_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
            let filename_lower = filename.to_lowercase();

            if filename_lower == "steam_api.dll"
                || filename_lower == "steam_api64.dll"
                || filename_lower == "libsteam_api.so"
            {
                if let Ok(rel_path) = path.strip_prefix(game_dir) {
                    let is_64bit = detect_bitness(path, &filename_lower);
                    dlls.push(SteamApiDll {
                        rel_path: rel_path.to_path_buf(),
                        is_64bit,
                    });
                    println!(
                        "[splitux] Found Steam API: {} ({})",
                        rel_path.display(),
                        if is_64bit { "64-bit" } else { "32-bit" }
                    );
                }
            }
        }
    }

    Ok(dlls)
}

/// Write Goldberg steam_settings configuration files
fn write_steam_settings(
    dir: &Path,
    config: &GoldbergConfig,
    handler_settings: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // steam_appid.txt
    fs::write(dir.join("steam_appid.txt"), config.app_id.to_string())?;

    // configs.user.ini
    let user_ini = format!(
        "[user::general]\naccount_name={}\naccount_steamid={}\n",
        config.account_name, config.steam_id
    );
    fs::write(dir.join("configs.user.ini"), user_ini)?;

    // configs.main.ini
    let main_ini = format!(
        r#"[main::general]
new_app_ticket=1
gc_token=1
matchmaking_server_list_actual_type=0
matchmaking_server_details_via_source_query=0

[main::connectivity]
disable_lan_only=0
disable_networking=0
listen_port={}
offline=0
disable_lobby_creation=0
disable_source_query=0
share_leaderboards_over_network=0
"#,
        config.listen_port
    );
    fs::write(dir.join("configs.main.ini"), main_ini)?;

    // custom_broadcasts.txt - list of other instances' ports for LAN discovery
    if !config.broadcast_ports.is_empty() {
        let broadcasts: String = config
            .broadcast_ports
            .iter()
            .map(|p| format!("127.0.0.1:{}", p))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(dir.join("custom_broadcasts.txt"), broadcasts)?;
    }

    // Auto-accept and auto-send invites for seamless multiplayer
    fs::write(dir.join("auto_accept_invite.txt"), "")?;
    fs::write(dir.join("auto_send_invite.txt"), "")?;

    // Write handler-specific Goldberg settings files
    for (filename, content) in handler_settings {
        fs::write(dir.join(filename), content)?;
        println!(
            "[splitux] Goldberg custom setting: {} = {:?}",
            filename,
            if content.is_empty() { "(empty)" } else { content.as_str() }
        );
    }

    Ok(())
}

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
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let overlay_dir = PATH_PARTY
        .join("tmp")
        .join(format!("goldberg-overlay-{}", instance_idx));

    // Clean previous overlay
    if overlay_dir.exists() {
        fs::remove_dir_all(&overlay_dir)?;
    }

    for dll in dlls {
        let dll_dir = dll.rel_path.parent().unwrap_or(Path::new(""));
        let target_dir = overlay_dir.join(dll_dir);
        fs::create_dir_all(&target_dir)?;

        // Select correct Goldberg source based on platform and bitness
        let goldberg_src = if is_windows {
            PATH_RES.join("goldberg/win")
        } else if dll.is_64bit {
            PATH_RES.join("goldberg/linux64")
        } else {
            PATH_RES.join("goldberg/linux32")
        };

        // Copy Goldberg DLL (preserving original filename)
        let dll_name = dll.rel_path.file_name().unwrap();
        let src_path = goldberg_src.join(dll_name);
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
        write_steam_settings(&settings_dir, config, handler_settings)?;
    }

    println!(
        "[splitux] Goldberg overlay {} created: Steam ID {}, Port {}, Broadcasts: {:?}",
        instance_idx, config.steam_id, config.listen_port, config.broadcast_ports
    );

    Ok(overlay_dir)
}

/// Create Goldberg overlays for all game instances
///
/// Returns a vector of overlay directory paths, one per instance
pub fn create_all_overlays(
    dlls: &[SteamApiDll],
    configs: &[GoldbergConfig],
    is_windows: bool,
    handler_settings: &HashMap<String, String>,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut overlays = Vec::new();

    for (i, config) in configs.iter().enumerate() {
        let overlay = create_instance_overlay(i, dlls, config, is_windows, handler_settings)?;
        overlays.push(overlay);
    }

    Ok(overlays)
}
