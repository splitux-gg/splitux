//! Photon/BepInEx integration for Unity games using Photon networking
//!
//! This module handles:
//! - Installing BepInEx into game overlay
//! - Generating per-instance config files for LocalMultiplayer mod
//!
//! Note: The LocalMultiplayer mod itself should be placed in the handler's
//! overlay/BepInEx/plugins/ directory by the user.

use std::fs;
use std::path::{Path, PathBuf};

use crate::app::load_photon_ids;
use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::{PATH_PARTY, PATH_RES};

/// Configuration for a Photon instance
#[derive(Debug, Clone)]
pub struct PhotonConfig {
    /// Instance index (0-based)
    pub instance_idx: usize,
    /// Player name for this instance
    pub player_name: String,
    /// Listen port for local networking
    pub listen_port: u16,
    /// Ports of other instances for discovery
    pub broadcast_ports: Vec<u16>,
}

/// Check if BepInEx resources are available
pub fn bepinex_available() -> bool {
    let bepinex_path = PATH_RES.join("bepinex");
    bepinex_path.exists() && bepinex_path.join("core").exists()
}

/// Get the path to bundled BepInEx resources
fn get_bepinex_res_path() -> PathBuf {
    PATH_RES.join("bepinex")
}

/// Create BepInEx overlay for a single instance
///
/// This creates an overlay with:
/// 1. BepInEx core files (from bundled resources)
/// 2. Doorstop loader (winhttp.dll for Windows)
/// 3. BepInEx configuration
///
/// The LocalMultiplayer mod should be in the handler's overlay/BepInEx/plugins/
fn create_instance_overlay(
    instance_idx: usize,
    handler: &Handler,
    config: &PhotonConfig,
    is_windows: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let overlay_dir = PATH_PARTY
        .join("tmp")
        .join(format!("photon-overlay-{}", instance_idx));

    // Clean previous overlay
    if overlay_dir.exists() {
        fs::remove_dir_all(&overlay_dir)?;
    }
    fs::create_dir_all(&overlay_dir)?;

    let bepinex_res = get_bepinex_res_path();

    if !bepinex_res.exists() {
        return Err("BepInEx resources not found. Please ensure res/bepinex/ exists.".into());
    }

    // 1. Copy BepInEx core
    let bepinex_core_src = bepinex_res.join("core");
    let bepinex_core_dest = overlay_dir.join("BepInEx").join("core");
    if bepinex_core_src.exists() {
        copy_dir_recursive(&bepinex_core_src, &bepinex_core_dest)?;
    }

    // 2. Copy doorstop loader
    if is_windows {
        let winhttp_src = bepinex_res.join("winhttp.dll");
        if winhttp_src.exists() {
            fs::copy(&winhttp_src, overlay_dir.join("winhttp.dll"))?;
        }
    }

    // 3. Write doorstop config
    write_doorstop_config(&overlay_dir, is_windows)?;

    // 4. Create BepInEx config directory
    let bepinex_config_dir = overlay_dir.join("BepInEx").join("config");
    fs::create_dir_all(&bepinex_config_dir)?;

    println!(
        "[splitux] Photon overlay {} created: Player {}, Port {}",
        instance_idx, config.player_name, config.listen_port
    );

    Ok(overlay_dir)
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

/// Write BepInEx doorstop configuration
fn write_doorstop_config(overlay_dir: &Path, is_windows: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = if is_windows {
        r#"[General]
enabled=true
target_assembly=BepInEx\core\BepInEx.Preloader.dll
"#
    } else {
        r#"[General]
enabled=true
target_assembly=BepInEx/core/BepInEx.Preloader.dll
"#
    };

    fs::write(overlay_dir.join("doorstop_config.ini"), config)?;
    Ok(())
}

/// Create Photon overlays for all instances
pub fn create_all_overlays(
    handler: &Handler,
    instances: &[Instance],
    is_windows: bool,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    // Check if Photon App IDs are configured
    let photon_ids = load_photon_ids();
    if photon_ids.pun_app_id.is_empty() {
        return Err(
            "Photon PUN App ID not configured. Go to Settings > Photon to set it up.".into(),
        );
    }

    // Check if BepInEx resources exist
    if !bepinex_available() {
        return Err("BepInEx resources not found in res/bepinex/. Cannot use Photon backend.".into());
    }

    // Generate ports for each instance (different range from Goldberg)
    const BASE_PORT: u16 = 47684;
    let instance_ports: Vec<u16> = (0..instances.len())
        .map(|i| BASE_PORT + i as u16)
        .collect();

    let mut overlays = Vec::new();

    for (i, instance) in instances.iter().enumerate() {
        let broadcast_ports: Vec<u16> = instance_ports
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, &port)| port)
            .collect();

        let config = PhotonConfig {
            instance_idx: i,
            player_name: instance.profname.clone(),
            listen_port: instance_ports[i],
            broadcast_ports,
        };

        let overlay = create_instance_overlay(i, handler, &config, is_windows)?;
        overlays.push(overlay);
    }

    Ok(overlays)
}

/// Generate the LocalMultiplayer config for an instance
///
/// This writes the config file to the profile's windata directory at the path
/// specified in handler.photon_settings.config_path
pub fn generate_instance_config(
    profile_path: &Path,
    handler: &Handler,
    instance_idx: usize,
    total_instances: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let photon_ids = load_photon_ids();

    // Get config path from handler settings
    let config_path_pattern = &handler.photon_settings.config_path;
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
        47684 + instance_idx as u16,
    );

    fs::write(&config_path, config)?;
    println!(
        "[splitux] Photon config written: {}",
        config_path.display()
    );

    Ok(())
}

/// Generate Photon configs for all instances at launch time
pub fn generate_all_configs(
    handler: &Handler,
    instances: &[Instance],
) -> Result<(), Box<dyn std::error::Error>> {
    for (i, instance) in instances.iter().enumerate() {
        let profile_path = PATH_PARTY.join("profiles").join(&instance.profname);
        generate_instance_config(&profile_path, handler, i, instances.len())?;
    }
    Ok(())
}
