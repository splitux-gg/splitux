//! Goldberg setup pipeline
//!
//! High-level orchestration for creating Goldberg overlays.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::super::operations::create_instance_overlay;
use super::super::types::{GoldbergConfig, SteamApiDll};

use crate::bepinex::{install_plugin_dlls, UnityBackend};
use crate::mods::{self, filter_dll_files, PluginSource};

/// Detect Unity backend from game directory
fn detect_unity_backend(game_dir: &Path) -> Option<UnityBackend> {
    // Check for IL2CPP (GameAssembly.dll in root)
    if game_dir.join("GameAssembly.dll").exists() {
        return Some(UnityBackend::Il2Cpp);
    }

    // Check for Mono (*_Data/Managed/ directory)
    if let Ok(entries) = fs::read_dir(game_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.ends_with("_Data") && path.join("Managed").exists() {
                    return Some(UnityBackend::Mono);
                }
            }
        }
    }

    None
}

/// Fetch plugin DLLs if a plugin source is specified
fn fetch_plugin_if_needed(
    plugin_source: &Option<PluginSource>,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    match plugin_source {
        Some(source) if !source.is_empty() => {
            eprintln!("[goldberg] Fetching plugin: {}", source.display_name());
            let cache_base = mods::cache_base();
            let all_files = mods::fetch_plugin(source, &cache_base)?;
            let dlls: Vec<PathBuf> = filter_dll_files(&all_files)
                .into_iter()
                .cloned()
                .collect();
            eprintln!("[goldberg] Found {} plugin DLL(s)", dlls.len());
            Ok(dlls)
        }
        _ => Ok(Vec::new()),
    }
}

/// Install BepInExPack from Thunderstore to overlay
///
/// This fetches the community-specific BepInExPack and copies all files to the overlay.
/// The BepInExPack includes proper game-specific entry point configurations.
fn install_bepinex_from_thunderstore(
    overlay_dir: &Path,
    community: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let cache_base = mods::cache_base();
    let bepinex_dir = mods::fetch_bepinex_pack(community, &cache_base)?;

    // Copy winhttp.dll (doorstop loader)
    let winhttp_src = bepinex_dir.join("winhttp.dll");
    if winhttp_src.exists() {
        fs::copy(&winhttp_src, overlay_dir.join("winhttp.dll"))?;
    }

    // Copy doorstop_config.ini
    let config_src = bepinex_dir.join("doorstop_config.ini");
    if config_src.exists() {
        fs::copy(&config_src, overlay_dir.join("doorstop_config.ini"))?;
    }

    // Copy .doorstop_version if present
    let version_src = bepinex_dir.join(".doorstop_version");
    if version_src.exists() {
        fs::copy(&version_src, overlay_dir.join(".doorstop_version"))?;
    }

    // Copy entire BepInEx directory structure
    let bepinex_src = bepinex_dir.join("BepInEx");
    if bepinex_src.exists() {
        copy_dir_recursive(&bepinex_src, &overlay_dir.join("BepInEx"))?;
    }

    Ok(())
}

/// Copy directory recursively
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }

    Ok(())
}

/// Create Goldberg overlays for all game instances
///
/// Returns a vector of overlay directory paths, one per instance.
/// Each overlay contains Goldberg DLLs and steam_settings configured
/// for that specific instance (unique Steam ID, port, etc.).
///
/// If a plugin is specified, BepInEx will be installed from Thunderstore
/// and the plugin DLLs installed to BepInEx/plugins/.
pub fn create_all_overlays(
    dlls: &[SteamApiDll],
    configs: &[GoldbergConfig],
    is_windows: bool,
    handler_settings: &HashMap<String, String>,
    disable_networking: bool,
    plugin_source: &Option<PluginSource>,
    game_dir: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    // Fetch plugin DLLs if specified
    let plugin_dlls = fetch_plugin_if_needed(plugin_source)?;
    let needs_bepinex = !plugin_dlls.is_empty();

    // Get community for BepInExPack download
    let community = plugin_source
        .as_ref()
        .map(|s| s.community.as_str())
        .unwrap_or("");

    // Detect Unity backend if we need BepInEx (for logging only)
    let unity_backend = if needs_bepinex {
        detect_unity_backend(game_dir).unwrap_or(UnityBackend::Mono)
    } else {
        UnityBackend::Mono
    };

    let mut overlays = Vec::new();

    for (i, config) in configs.iter().enumerate() {
        // Create base Goldberg overlay
        let overlay = create_instance_overlay(
            i,
            dlls,
            config,
            is_windows,
            handler_settings,
            disable_networking,
        )?;

        // Install BepInEx + plugin if needed
        if needs_bepinex {
            eprintln!(
                "[goldberg] Installing BepInEx ({}) for instance {} from Thunderstore",
                unity_backend.display_name(),
                i
            );

            // Install BepInExPack from Thunderstore (includes game-specific configs)
            install_bepinex_from_thunderstore(&overlay, community)?;

            // Install plugin DLLs to BepInEx/plugins/
            install_plugin_dlls(&overlay, &plugin_dlls)?;
        }

        overlays.push(overlay);
    }

    Ok(overlays)
}
