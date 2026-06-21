//! Steam game path resolution
//!
//! Uses steamlocate crate to find installed Steam games.

use std::error::Error;
use std::path::PathBuf;

/// Find the installation path for a Steam game by app ID
///
/// Uses steamlocate to search all Steam library folders for the app.
/// Returns the resolved app directory path if found.
pub fn find_game_path(app_id: u32) -> Result<PathBuf, Box<dyn Error>> {
    let steam_dir = steamlocate::SteamDir::locate()?;

    if let Some((app, library)) = steam_dir.find_app(app_id).ok().flatten() {
        let path = library.resolve_app_dir(&app);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(format!("Steam app {} not found or not installed", app_id).into())
}

/// Resolve the Proton prefix's Windows user directory for a Steam app:
/// `<library>/steamapps/compatdata/<appid>/pfx/drive_c/users/steamuser`.
///
/// This is the root under which a Proton/Windows game's saves live (`AppData/`,
/// `Documents/`, `Saved Games/`, ...). It mirrors what splitux bind-mounts a
/// profile's `windata` over, so a directed sub-path (e.g.
/// `AppData/LocalLow/Stunlock Studios/VRising`) appended here is the absolute
/// original-save path the save-sync engine anchors. Returns the dir only if it
/// actually exists (a Proton game that has been run at least once).
pub fn find_compat_steamuser(app_id: u32) -> Result<PathBuf, Box<dyn Error>> {
    let steam_dir = steamlocate::SteamDir::locate()?;

    if let Some((_app, library)) = steam_dir.find_app(app_id).ok().flatten() {
        let steamuser = library
            .path()
            .join("steamapps/compatdata")
            .join(app_id.to_string())
            .join("pfx/drive_c/users/steamuser");
        if steamuser.is_dir() {
            return Ok(steamuser);
        }
    }

    Err(format!("no Proton compatdata for app {} (never launched, or native)", app_id).into())
}

/// Resolve the local Steam Cloud / Remote Storage staging dir for a Steam app:
/// `<steam>/userdata/<account_id>/<appid>/remote/`. Games like Risk of Rain 2 keep
/// their save here (synced by Steam Cloud) rather than under the Proton prefix.
/// Picks the userdata account dir that actually has this app's `remote/` folder.
pub fn find_steam_userdata_remote(app_id: u32) -> Result<PathBuf, Box<dyn Error>> {
    use crate::paths::PATH_STEAM;
    let userdata = PATH_STEAM.join("userdata");
    if let Ok(entries) = std::fs::read_dir(&userdata) {
        for e in entries.flatten() {
            // account dirs are numeric (skip "0", "anonymous", etc.)
            let name = e.file_name();
            if !name.to_string_lossy().chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let remote = e.path().join(app_id.to_string()).join("remote");
            if remote.is_dir() {
                return Ok(remote);
            }
        }
    }
    Err(format!("no Steam Cloud remote dir for app {} (never synced on this account?)", app_id).into())
}
