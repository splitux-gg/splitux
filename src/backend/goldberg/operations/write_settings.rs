//! Goldberg steam_settings file generation
//!
//! Writes configuration files for Goldberg Steam emulator.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::super::types::GoldbergConfig;

/// Write Goldberg steam_settings configuration files to a directory
///
/// Creates the following files:
/// - steam_appid.txt
/// - configs.user.ini (account name, steam id)
/// - configs.main.ini (networking settings)
/// - custom_broadcasts.txt (LAN discovery ports)
/// - auto_accept_invite.txt, auto_send_invite.txt
/// - Any custom handler settings files
pub fn write_steam_settings(
    dir: &Path,
    config: &GoldbergConfig,
    handler_settings: &HashMap<String, String>,
    disable_networking: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // steam_appid.txt
    fs::write(dir.join("steam_appid.txt"), config.app_id.to_string())?;

    // configs.user.ini
    let user_ini = format!(
        "[user::general]\naccount_name={}\naccount_steamid={}\n",
        config.account_name, config.steam_id
    );
    fs::write(dir.join("configs.user.ini"), user_ini)?;

    // configs.main.ini
    let disable_networking_val = if disable_networking { 1 } else { 0 };
    let main_ini = format!(
        r#"[main::general]
new_app_ticket=1
gc_token=1
matchmaking_server_list_actual_type=0
matchmaking_server_details_via_source_query=0

[main::connectivity]
disable_lan_only=0
disable_networking={}
listen_port={}
offline=0
disable_lobby_creation=0
disable_source_query=0
share_leaderboards_over_network=0
"#,
        disable_networking_val, config.listen_port
    );
    fs::write(dir.join("configs.main.ini"), main_ini)?;

    // custom_broadcasts.txt - list of other instances' ports for LAN discovery
    if !config.broadcast_ports.is_empty() {
        let broadcasts: String = config
            .broadcast_ports
            .iter()
            .map(|p| format!("127.0.0.1:{}", p))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(dir.join("custom_broadcasts.txt"), broadcasts)?;
    }

    // Auto-accept and auto-send invites for seamless multiplayer
    fs::write(dir.join("auto_accept_invite.txt"), "")?;
    fs::write(dir.join("auto_send_invite.txt"), "")?;

    // Write handler-specific Goldberg settings files
    for (filename, content) in handler_settings {
        fs::write(dir.join(filename), content)?;
        println!(
            "[splitux] Goldberg custom setting: {} = {:?}",
            filename,
            if content.is_empty() {
                "(empty)"
            } else {
                content.as_str()
            }
        );
    }

    Ok(())
}
