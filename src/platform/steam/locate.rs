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
