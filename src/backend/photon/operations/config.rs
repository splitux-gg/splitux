//! Photon config generation
//!
//! Generates LocalMultiplayer config files for each instance.

use std::fs;
use std::path::Path;

use crate::app::load_photon_ids;
use crate::handler::Handler;

use super::super::types::PHOTON_BASE_PORT;

/// Generate the LocalMultiplayer config for an instance
///
/// This writes the config file to the profile's windata directory at the path
/// specified in handler.photon.config_path
pub fn generate_instance_config(
    profile_path: &Path,
    handler: &Handler,
    instance_idx: usize,
    total_instances: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let photon_ids = load_photon_ids();

    // Get Photon settings from new optional field (Phase 7)
    let photon_settings = handler
        .photon_ref()
        .ok_or("Photon backend not enabled")?;

    // Get config path from handler settings
    let config_path_pattern = &photon_settings.config_path;
    if config_path_pattern.is_empty() {
        return Err("Handler must specify photon.config_path for Photon backend".into());
    }

    // Build the full path within the profile's windata directory
    let config_path = profile_path.join("windata").join(config_path_pattern);

    // Create parent directories
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Generate config content
    let config = format!(
        r#"[Photon]
AppId={}
VoiceAppId={}

[LocalMultiplayer]
PlayerIndex={}
TotalPlayers={}
ListenPort={}
"#,
        photon_ids.pun_app_id,
        photon_ids.voice_app_id,
        instance_idx,
        total_instances,
        PHOTON_BASE_PORT + instance_idx as u16,
    );

    fs::write(&config_path, config)?;
    println!(
        "[splitux] Photon config written: {}",
        config_path.display()
    );

    Ok(())
}
