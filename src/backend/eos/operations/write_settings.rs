//! EOS emulator settings writer
//!
//! Writes NemirtingasEpicEmu.json configuration files.

use std::fs;
use std::path::Path;

use super::super::types::EosConfig;

/// Write EOS emulator settings to the specified directory
///
/// Creates NemirtingasEpicEmu.json with instance-specific configuration.
pub fn write_eos_settings(
    settings_dir: &Path,
    config: &EosConfig,
    enable_lan: bool,
    disable_online_networking: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(settings_dir)?;

    // Build custom_broadcast from broadcast_ports (for future use)
    let _custom_broadcasts: Vec<String> = config
        .broadcast_ports
        .iter()
        .map(|p| format!("127.0.0.1:{}", p))
        .collect();

    let settings = serde_json::json!({
        "appid": config.appid,
        "username": config.username,
        "epicid": config.epicid,
        "productuserid": config.productuserid,
        "language": "en",
        "savepath": "appdata",
        "enable_overlay": false,
        "unlock_dlcs": true,
        "enable_lan": enable_lan,
        "disable_online_networking": disable_online_networking,
        "listen_port": config.listen_port,
        "log_level": "INFO"
    });

    let settings_path = settings_dir.join("NemirtingasEpicEmu.json");
    let json = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, json)?;

    println!(
        "[splitux] EOS settings written to: {}",
        settings_path.display()
    );

    Ok(())
}
