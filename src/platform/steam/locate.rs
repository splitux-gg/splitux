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
