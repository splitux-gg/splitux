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

/// Get the Steam installation directory
pub fn steam_dir() -> Result<PathBuf, Box<dyn Error>> {
    let steam_dir = steamlocate::SteamDir::locate()?;
    Ok(steam_dir.path().to_path_buf())
}

/// Get the app's install directory name (for display/naming purposes)
pub fn get_install_dir_name(app_id: u32) -> Option<String> {
    let steam_dir = steamlocate::SteamDir::locate().ok()?;
    let (app, _) = steam_dir.find_app(app_id).ok()??;
    Some(app.install_dir)
}
