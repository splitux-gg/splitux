//! Photon setup pipeline
//!
//! High-level orchestration for creating Photon overlays and configs.

use std::path::{Path, PathBuf};

use crate::app::load_photon_ids;
use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::PATH_PARTY;

use super::super::operations::{
    bepinex_backend_available, create_instance_overlay, generate_instance_config,
    setup_shared_files,
};
use super::super::pure::detect_unity_backend;
use super::super::types::PhotonConfig;

/// Base port for Photon networking (different range from Goldberg)
const BASE_PORT: u16 = 47684;

/// Create Photon overlays for all instances
pub fn create_all_overlays(
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
