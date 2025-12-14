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
    /// Type of DLL (steam_api or steamnetworkingsockets)
    pub dll_type: SteamDllType,
}

/// Type of Steam-related DLL
#[derive(Debug, Clone, PartialEq)]
pub enum SteamDllType {
    /// Standard Steam API (steam_api.dll, steam_api64.dll, libsteam_api.so)
    SteamApi,
    /// GameNetworkingSockets (GameNetworkingSockets.dll)
    NetworkingSockets,
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

/// Detect whether a Steam API DLL is 64-bit based on filename, directory hints, or ELF header
fn detect_bitness(path: &Path, filename: &str) -> bool {
    // 1. Check filename first (most reliable for Windows)
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
    // Linux: libsteam_api.so - read ELF header to determine bitness
    if filename == "libsteam_api.so" {
        if let Ok(data) = fs::read(path) {
            // ELF magic: 0x7F 'E' 'L' 'F'
            // e_ident[EI_CLASS] at offset 4: 1 = 32-bit, 2 = 64-bit
            if data.len() >= 5 && &data[0..4] == b"\x7FELF" {
                return data[4] == 2; // ELFCLASS64
            }
        }
        // Fallback to directory hints if ELF read fails
        let path_str = path.to_string_lossy().to_lowercase();
        return path_str.contains("64") || path_str.contains("x86_64");
    }
    // Other files: use directory hints
    let path_str = path.to_string_lossy().to_lowercase();
    path_str.contains("64") || path_str.contains("x86_64")
}

/// Find all Steam API DLLs in the game directory
///
/// Searches recursively for steam_api.dll, steam_api64.dll, libsteam_api.so,
/// and GameNetworkingSockets.dll
/// Returns relative paths from the game root along with bitness information
pub fn find_steam_api_dlls(game_dir: &Path) -> Result<Vec<SteamApiDll>, Box<dyn std::error::Error>> {
    let mut dlls = Vec::new();
    // Track directories containing 64-bit steam_api DLLs (for inferring GameNetworkingSockets bitness)
    let mut dirs_with_64bit_steam_api: Vec<PathBuf> = Vec::new();
    // Defer GameNetworkingSockets detection until we know steam_api bitness
    let mut networking_sockets_paths: Vec<(PathBuf, PathBuf)> = Vec::new(); // (full_path, rel_path)

    for entry in WalkDir::new(game_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
            let filename_lower = filename.to_lowercase();

            // Standard Steam API DLLs
            if filename_lower == "steam_api.dll"
                || filename_lower == "steam_api64.dll"
                || filename_lower == "libsteam_api.so"
            {
                if let Ok(rel_path) = path.strip_prefix(game_dir) {
                    let is_64bit = detect_bitness(path, &filename_lower);
                    if is_64bit {
                        if let Some(dir) = rel_path.parent() {
                            dirs_with_64bit_steam_api.push(dir.to_path_buf());
                        }
                    }
                    dlls.push(SteamApiDll {
                        rel_path: rel_path.to_path_buf(),
                        is_64bit,
                        dll_type: SteamDllType::SteamApi,
                    });
                    println!(
                        "[splitux] Found Steam API: {} ({})",
                        rel_path.display(),
                        if is_64bit { "64-bit" } else { "32-bit" }
                    );
                }
            }
            // GameNetworkingSockets.dll - defer bitness detection
            else if filename_lower == "gamenetworkingsockets.dll" {
                if let Ok(rel_path) = path.strip_prefix(game_dir) {
                    networking_sockets_paths.push((path.to_path_buf(), rel_path.to_path_buf()));
                }
            }
        }
    }

    // Now process GameNetworkingSockets.dll with knowledge of steam_api locations
    for (full_path, rel_path) in networking_sockets_paths {
        let dll_dir = rel_path.parent().map(|p| p.to_path_buf());

        // Infer bitness: if steam_api64.dll exists in same directory, it's 64-bit
        let is_64bit = if let Some(ref dir) = dll_dir {
            dirs_with_64bit_steam_api.iter().any(|d| d == dir)
        } else {
            // Fallback to path-based detection
            detect_bitness(&full_path, "gamenetworkingsockets.dll")
        };

        dlls.push(SteamApiDll {
            rel_path: rel_path.clone(),
            is_64bit,
            dll_type: SteamDllType::NetworkingSockets,
        });
        println!(
            "[splitux] Found GameNetworkingSockets: {} ({})",
            rel_path.display(),
            if is_64bit { "64-bit" } else { "32-bit" }
        );
    }

    Ok(dlls)
}

/// Write Goldberg steam_settings configuration files
fn write_steam_settings(
    dir: &Path,
    config: &GoldbergConfig,
    handler_settings: &HashMap<String, String>,
    disable_networking: bool,
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
    let disable_networking_val = if disable_networking { 1 } else { 0 };
    let main_ini = format!(
        r#"[main::general]
new_app_ticket=1
gc_token=1
matchmaking_server_list_actual_type=0
matchmaking_server_details_via_source_query=0

[main::connectivity]
disable_lan_only=0
disable_networking={}
listen_port={}
offline=0
disable_lobby_creation=0
disable_source_query=0
share_leaderboards_over_network=0
"#,
        disable_networking_val,
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
    disable_networking: bool,
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
            write_steam_settings(&root_settings_dir, config, handler_settings, disable_networking)?;
            println!(
                "[splitux] Goldberg overlay {}: Also created steam_settings at game root",
                instance_idx
            );
        }
        // Also create steam_appid.txt at game root (some games need this)
        fs::write(overlay_dir.join("steam_appid.txt"), config.app_id.to_string())?;
    }

    println!(
        "[splitux] Goldberg overlay {} created: Steam ID {}, Port {}, Broadcasts: {:?}, disable_networking: {}",
        instance_idx, config.steam_id, config.listen_port, config.broadcast_ports, disable_networking
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
    disable_networking: bool,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut overlays = Vec::new();

    for (i, config) in configs.iter().enumerate() {
        let overlay = create_instance_overlay(i, dlls, config, is_windows, handler_settings, disable_networking)?;
        overlays.push(overlay);
    }

    Ok(overlays)
}
