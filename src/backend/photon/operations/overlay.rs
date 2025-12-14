//! Photon overlay creation
//!
//! Creates per-instance overlay directories with BepInEx configuration.

use std::fs;
use std::path::PathBuf;

use crate::paths::PATH_PARTY;

use super::super::types::{PhotonConfig, UnityBackend};
use super::bepinex::{install_bepinex_core, install_doorstop, write_doorstop_config};

/// Create BepInEx overlay for a single instance
///
/// This creates an overlay with:
/// 1. BepInEx core files (from bundled resources, Mono or IL2CPP)
/// 2. Doorstop loader (winhttp.dll for Windows)
/// 3. BepInEx configuration
///
/// The LocalMultiplayer mod should be in the handler's overlay/BepInEx/plugins/
pub fn create_instance_overlay(
    instance_idx: usize,
    config: &PhotonConfig,
    is_windows: bool,
    backend: UnityBackend,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let overlay_dir = PATH_PARTY
        .join("tmp")
        .join(format!("photon-overlay-{}", instance_idx));

    // Clean previous overlay
    if overlay_dir.exists() {
        fs::remove_dir_all(&overlay_dir)?;
    }
    fs::create_dir_all(&overlay_dir)?;

    // 1. Install BepInEx core
    install_bepinex_core(&overlay_dir, backend)?;

    // 2. Install doorstop loader
    install_doorstop(&overlay_dir, backend, is_windows)?;

    // 3. Write doorstop config
    write_doorstop_config(&overlay_dir, is_windows, backend)?;

    println!(
        "[splitux] Photon overlay {} created: Player {}, Port {}, Backend: {}",
        instance_idx,
        config.player_name,
        config.listen_port,
        backend.display_name()
    );

    Ok(overlay_dir)
}
