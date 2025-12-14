//! Photon setup pipeline
//!
//! High-level orchestration for creating Photon overlays and configs.

use std::path::{Path, PathBuf};

use crate::paths::PATH_PARTY;

use super::super::operations::{
    bepinex_backend_available, create_instance_overlay, generate_instance_config,
    setup_shared_files, PhotonAppIds,
};
use super::super::pure::detect_unity_backend;
use super::super::types::{PhotonConfig, PHOTON_BASE_PORT};

/// Instance info needed for Photon setup
pub struct PhotonInstance {
    pub profile_name: String,
}

/// Create Photon overlays for all instances
pub fn create_all_overlays(
    instances: &[PhotonInstance],
    is_windows: bool,
    game_dir: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
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
        .map(|i| PHOTON_BASE_PORT + i as u16)
        .collect();

    let mut overlays = Vec::new();

    for (i, instance) in instances.iter().enumerate() {
        let broadcast_ports: Vec<u16> = instance_ports
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, &port)| port)
            .collect();

        let config = PhotonConfig::new(
            i,
            instance.profile_name.clone(),
            instance_ports[i],
            broadcast_ports,
        );

        let overlay = create_instance_overlay(i, &config, is_windows, backend)?;
        overlays.push(overlay);
    }

    Ok(overlays)
}

/// Generate Photon configs for all instances at launch time
pub fn generate_all_configs(
    instances: &[PhotonInstance],
    config_path_pattern: &str,
    shared_files: &[String],
    photon_ids: &PhotonAppIds,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate Photon App IDs
    if photon_ids.pun_app_id.is_empty() {
        return Err(
            "Photon PUN App ID not configured. Go to Settings > Photon to set it up.".into(),
        );
    }

    // Set up shared files first (before instance configs)
    if !shared_files.is_empty() {
        let profile_names: Vec<String> = instances.iter().map(|i| i.profile_name.clone()).collect();
        setup_shared_files(shared_files, &profile_names)?;
    }

    // Generate config for each instance
    for (i, instance) in instances.iter().enumerate() {
        let profile_path = PATH_PARTY.join("profiles").join(&instance.profile_name);
        generate_instance_config(
            &profile_path,
            config_path_pattern,
            photon_ids,
            i,
            instances.len(),
        )?;
    }

    Ok(())
}
