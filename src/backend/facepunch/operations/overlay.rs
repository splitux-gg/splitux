//! Facepunch overlay creation
//!
//! Creates per-instance overlay directories with BepInEx and SplituxFacepunch.

use std::fs;
use std::path::PathBuf;

use crate::backend::operations::prepare_overlay_dir;

// Re-use UnityBackend from photon module
use crate::backend::photon::UnityBackend;

use super::super::pure::generate_config_content;
use super::super::types::FacepunchConfig;
use super::bepinex::{
    install_bepinex_core, install_doorstop, install_splitux_plugin, write_doorstop_config,
};

/// Create a Facepunch overlay for a single instance
///
/// This creates an overlay with:
/// 1. BepInEx core files (Mono or IL2CPP backend)
/// 2. Doorstop loader (winhttp.dll for Windows, libdoorstop.so for Linux)
/// 3. BepInEx/config/splitux.cfg with instance-specific settings
/// 4. SplituxFacepunch.dll plugin
pub fn create_instance_overlay(
    instance_idx: usize,
    config: &FacepunchConfig,
    is_windows: bool,
    backend: UnityBackend,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let overlay_dir = prepare_overlay_dir("facepunch", instance_idx)?;

    // 1. Install BepInEx core
    install_bepinex_core(&overlay_dir, is_windows, backend)?;

    // 2. Install doorstop loader
    install_doorstop(&overlay_dir, is_windows, backend)?;

    // 3. Write doorstop config
    write_doorstop_config(&overlay_dir, is_windows, backend)?;

    // 4. Write splitux.cfg
    let config_content = generate_config_content(config);
    let bepinex_config_dir = overlay_dir.join("BepInEx").join("config");
    fs::write(bepinex_config_dir.join("splitux.cfg"), &config_content)?;

    // 5. Write BepInEx.cfg to disable console logging (prevents CStreamWriter crash on Linux)
    // This is critical for native Linux games - BepInEx's LinuxConsoleDriver crashes
    // with CStreamWriter if console logging is enabled
    let bepinex_cfg = r#"[Logging.Console]
Enabled = false

[Logging.Disk]
Enabled = true
LogLevels = All

[Chainloader]
HideManagerGameObject = true
"#;
    fs::write(bepinex_config_dir.join("BepInEx.cfg"), bepinex_cfg)?;

    // 6. Install SplituxFacepunch plugin
    install_splitux_plugin(&overlay_dir)?;

    println!(
        "[splitux] Facepunch overlay {} created: Player {}, SteamID {}, Backend: {}",
        instance_idx,
        config.account_name,
        config.steam_id,
        backend.display_name()
    );

    Ok(overlay_dir)
}
