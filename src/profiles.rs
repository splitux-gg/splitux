use std::error::Error;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

use crate::{handler::Handler, paths::*, util::copy_dir_recursive};

// Simple hash function for generating unique values from profile name
fn hash_name(name: &str) -> u64 {
    let mut hash: u64 = 5381;
    for byte in name.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    hash
}

// Generate a unique Steam64 ID based on the profile name
// Steam64 IDs start at 76561197960265728 (base) + account_id
pub fn generate_steam_id(name: &str) -> u64 {
    const STEAM64_BASE: u64 = 76561197960265728;
    // Limit to valid account ID range (roughly 0 to 1 billion)
    let account_id = (hash_name(name) % 1_000_000_000) + 1;
    STEAM64_BASE + account_id
}

// Generate a unique Goldberg listen port based on profile name
// Ports range from 47584 to 48583 (1000 possible ports)
pub fn generate_listen_port(name: &str) -> u16 {
    const BASE_PORT: u16 = 47584;
    const PORT_RANGE: u16 = 1000;
    BASE_PORT + (hash_name(name) % PORT_RANGE as u64) as u16
}

// Makes a folder and sets up Goldberg Steam Emu profile for Steam games
pub fn create_profile(name: &str) -> Result<(), std::io::Error> {
    if PATH_PARTY.join(format!("profiles/{name}")).exists() {
        return Ok(());
    }

    println!("[splitux] Creating profile {name}");
    let path_profile = PATH_PARTY.join(format!("profiles/{name}"));
    // Goldberg expects settings in {GseAppPath}/steam_settings/
    let path_steam = path_profile.join("steam/steam_settings");

    std::fs::create_dir_all(path_profile.join("windata/AppData/Local/Temp"))?;
    std::fs::create_dir_all(path_profile.join("windata/AppData/LocalLow"))?;
    std::fs::create_dir_all(path_profile.join("windata/AppData/Roaming"))?;
    std::fs::create_dir_all(path_profile.join("windata/Documents"))?;
    std::fs::create_dir_all(path_profile.join("windata/Saved Games"))?;
    std::fs::create_dir_all(path_profile.join("windata/Desktop"))?;
    std::fs::create_dir_all(path_profile.join("home/.local/share"))?;
    std::fs::create_dir_all(path_profile.join("home/.config"))?;
    std::fs::create_dir_all(path_steam.clone())?;

    // Create symlinks for Steam API access inside sandboxed HOME
    // This allows native Linux games to initialize the Steam API while using isolated saves
    if let Ok(real_home) = std::env::var("HOME") {
        let real_home = PathBuf::from(real_home);

        // Symlink ~/.steam -> real ~/.steam
        let steam_dir = real_home.join(".steam");
        let profile_steam = path_profile.join("home/.steam");
        if steam_dir.exists() && !profile_steam.exists() {
            let _ = symlink(&steam_dir, &profile_steam);
        }

        // Symlink ~/.local/share/Steam -> real Steam
        let steam_share = real_home.join(".local/share/Steam");
        let profile_steam_share = path_profile.join("home/.local/share/Steam");
        if steam_share.exists() && !profile_steam_share.exists() {
            let _ = symlink(&steam_share, &profile_steam_share);
        }
    }

    // Generate unique Steam ID and listen port for this profile
    let steam_id = generate_steam_id(name);
    let listen_port = generate_listen_port(name);

    // User settings (account name and Steam ID)
    let usersettings = format!(
        "[user::general]\naccount_name={name}\naccount_steamid={steam_id}"
    );
    std::fs::write(path_steam.join("configs.user.ini"), usersettings)?;

    // Main settings (unique listen port for LAN multiplayer)
    let mainsettings = format!(
        r#"[main::general]
new_app_ticket=1
gc_token=1
matchmaking_server_list_actual_type=0
matchmaking_server_details_via_source_query=0

[main::connectivity]
disable_lan_only=0
disable_networking=0
listen_port={listen_port}
offline=0
disable_lobby_creation=0
disable_source_query=0
share_leaderboards_over_network=0
"#
    );
    std::fs::write(path_steam.join("configs.main.ini"), mainsettings)?;

    // Auto-accept and auto-send invites for seamless multiplayer
    std::fs::write(path_steam.join("auto_accept_invite.txt"), "")?;
    std::fs::write(path_steam.join("auto_send_invite.txt"), "")?;

    println!("[splitux] Profile created: Steam ID {steam_id}, Port {listen_port}");
    Ok(())
}

// Creates the "game save" folder for per-profile game data to go into
pub fn create_profile_gamesave(name: &str, h: &Handler) -> Result<(), Box<dyn Error>> {
    let uid = h.handler_dir_name();
    let path_prof = PATH_PARTY.join("profiles").join(name);
    let path_gamesave = path_prof.join("gamesaves").join(&uid);
    let path_home = path_prof.join("home");
    let path_windata = path_prof.join("windata");

    if path_gamesave.exists() {
        return Ok(());
    }
    println!("[splitux] Creating game save {} for {}", uid, name);

    std::fs::create_dir_all(&path_gamesave)?;
    
    if let Some(appid) = h.steam_appid && h.use_goldberg {
        let path_exec = path_gamesave.join(&h.exec);
        let path_execdir = path_exec.parent().ok_or_else(|| "couldn't get parent")?;
        if !path_execdir.exists() {
            std::fs::create_dir_all(&path_execdir)?;
        }
        std::fs::write(path_execdir.join("steam_appid.txt"), appid.to_string())?;
    }

    let profile_copy_gamesave = PathBuf::from(&h.path_handler).join("profile_copy_gamesave");
    if profile_copy_gamesave.exists() {
        copy_dir_recursive(&profile_copy_gamesave, &path_gamesave)?;
    }

    let profile_copy_home = PathBuf::from(&h.path_handler).join("profile_copy_home");
    if profile_copy_home.exists() {
        copy_dir_recursive(&profile_copy_home, &path_home)?;
    }

    let profile_copy_windata = PathBuf::from(&h.path_handler).join("profile_copy_windata");
    if profile_copy_windata.exists() {
        copy_dir_recursive(&profile_copy_windata, &path_windata)?;
    }

    println!("[splitux] Profile save data created successfully");
    Ok(())
}

// Gets a vector of all available profiles.
// include_guest true for building the profile selector dropdown, false for the profile viewer.
pub fn scan_profiles(include_guest: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(PATH_PARTY.join("profiles")) {
        for entry in entries {
            if let Ok(entry) = entry
                && entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                && let Some(name) = entry.file_name().to_str()
            {
                out.push(name.to_string());
            }
        }
    }

    out.sort();

    if include_guest {
        out.insert(0, "Guest".to_string());
    }

    out
}

pub fn remove_guest_profiles() -> Result<(), Box<dyn Error>> {
    let path_profiles = PATH_PARTY.join("profiles");
    let entries = std::fs::read_dir(&path_profiles)?;
    for entry in entries.flatten() {
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with(".") {
            std::fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

pub static GUEST_NAMES: [&str; 33] = [
    "Blinky", "Pinky", "Inky", "Clyde", "Beatrice", "Battler", "Miyao", "Rena", "Ellie", "Joel",
    "Leon", "Ada", "Madeline", "Theo", "Yokatta", "Wyrm", "Brodiee", "Supreme", "Conk", "Gort",
    "Lich", "Smores", "Canary", "Trico", "Yorda", "Wander", "Agro", "Jak", "Daxter", "Soap",
    "Ghost", "Tomi", "Masaki",
];
