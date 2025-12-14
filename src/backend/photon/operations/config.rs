//! Photon config generation
//!
//! Generates LocalMultiplayer config files for each instance.

use std::fs;
use std::path::Path;

use super::super::types::PHOTON_BASE_PORT;

/// Photon App IDs for LocalMultiplayer mod
#[derive(Debug, Clone, Default)]
pub struct PhotonAppIds {
    pub pun_app_id: String,
    pub voice_app_id: String,
}

/// Generate the LocalMultiplayer config for an instance
///
/// This writes the config file to the profile's windata directory at the path
/// specified in the config_path parameter.
pub fn generate_instance_config(
    profile_path: &Path,
    config_path_pattern: &str,
    photon_ids: &PhotonAppIds,
    instance_idx: usize,
    total_instances: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if config_path_pattern.is_empty() {
        return Err(
            "Handler must specify photon_settings.config_path for Photon backend".into(),
        );
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
