//! Photon setup pipeline
//!
//! High-level orchestration for creating Photon overlays and configs.

use std::fs;
use std::path::{Path, PathBuf};

use crate::app::load_photon_ids;
use crate::handler::Handler;
use crate::instance::Instance;
use crate::mods::{self, filter_dll_files, PluginSource};
use crate::paths::PATH_PARTY;

use super::super::operations::{
    bepinex_backend_available, create_instance_overlay, generate_instance_config,
    setup_shared_files,
};
use super::super::pure::detect_unity_backend;
use super::super::types::PhotonConfig;

/// Base port for Photon networking (different range from Goldberg)
const BASE_PORT: u16 = 47684;

/// Fetch plugin from source if specified, returns list of DLL paths
fn fetch_plugin_if_needed(
    plugin_source: &Option<PluginSource>,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    match plugin_source {
        Some(source) if !source.is_empty() => {
            let cache_base = mods::cache_base();
            let all_files = mods::fetch_plugin(source, &cache_base)?;
            // Filter to just DLLs and clone the paths
            let dlls: Vec<PathBuf> = filter_dll_files(&all_files)
                .into_iter()
                .cloned()
                .collect();
            Ok(dlls)
        }
        _ => Ok(Vec::new()),
    }
}

/// Copy plugin DLLs to overlay's BepInEx/plugins directory
fn install_plugin_to_overlay(
    overlay_dir: &Path,
    dll_files: &[PathBuf],
) -> Result<(), Box<dyn std::error::Error>> {
    if dll_files.is_empty() {
        return Ok(());
    }

    let plugins_dir = overlay_dir.join("BepInEx").join("plugins");
    fs::create_dir_all(&plugins_dir)?;

    for dll_path in dll_files {
        let filename = dll_path.file_name().ok_or("Invalid DLL path")?;
        let dest = plugins_dir.join(filename);
        fs::copy(dll_path, &dest)?;
        eprintln!("[mods] Installed plugin: {:?}", filename);
    }

    Ok(())
}

/// Create Photon overlays for all instances
pub fn create_all_overlays(
    handler: &Handler,
    instances: &[Instance],
    is_windows: bool,
    game_dir: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    // Check if Photon App IDs are configured
    let photon_ids = load_photon_ids();
    if photon_ids.pun_app_id.is_empty() {
        return Err(
            "Photon PUN App ID not configured. Go to Settings > Photon to set it up.".into(),
        );
    }

    // Detect Unity backend type
    let backend = detect_unity_backend(game_dir);

    // Check if BepInEx resources exist for this backend
    if !bepinex_backend_available(backend) {
        return Err(format!(
            "BepInEx resources not found for {} backend. Run ./splitux.sh build",
            backend.display_name()
        )
        .into());
    }

    // Fetch plugin if specified in handler
    let plugin_dlls = match handler.photon_ref() {
        Some(settings) => fetch_plugin_if_needed(&settings.plugin)?,
        None => Vec::new(),
    };

    // Generate ports for each instance
    let instance_ports: Vec<u16> = (0..instances.len())
        .map(|i| BASE_PORT + i as u16)
        .collect();

    let mut overlays = Vec::new();

    for (i, instance) in instances.iter().enumerate() {
        let config = PhotonConfig {
            player_name: instance.profname.clone(),
            listen_port: instance_ports[i],
        };

        let overlay = create_instance_overlay(i, &config, is_windows, backend)?;

        // Install plugin DLLs to overlay
        install_plugin_to_overlay(&overlay, &plugin_dlls)?;

        overlays.push(overlay);
    }

    Ok(overlays)
}

/// Generate Photon configs for all instances at launch time
pub fn generate_all_configs(
    handler: &Handler,
    instances: &[Instance],
) -> Result<(), Box<dyn std::error::Error>> {
    // Get Photon settings from new optional field (Phase 7)
    let photon_settings = handler
        .photon_ref()
        .ok_or("Photon backend not enabled")?;

    // Set up shared files first (before instance configs)
    if !photon_settings.shared_files.is_empty() {
        setup_shared_files(handler, instances)?;
    }

    for (i, instance) in instances.iter().enumerate() {
        let profile_path = PATH_PARTY.join("profiles").join(&instance.profname);
        generate_instance_config(&profile_path, handler, i, instances.len())?;
    }

    Ok(())
}
