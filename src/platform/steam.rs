//! Steam platform implementation
//!
//! Provides game path resolution and artwork caching for Steam games.

use super::Platform;
use std::error::Error;
use std::path::PathBuf;

mod cache;
mod locate;

// Re-export submodule functions for direct access if needed
pub use locate::{find_game_path, get_install_dir_name};

/// Steam platform implementation
pub struct SteamPlatform {
    pub app_id: u32,
}

impl SteamPlatform {
    pub fn new(app_id: u32) -> Self {
        Self { app_id }
    }
}

impl Platform for SteamPlatform {
    fn name(&self) -> &str {
        "steam"
    }

    fn game_root_path(&self) -> Result<PathBuf, Box<dyn Error>> {
        locate::find_game_path(self.app_id)
    }

    fn icon_uri(&self) -> Option<String> {
        cache::icon_uri(self.app_id)
    }

    fn logo_uri(&self) -> Option<String> {
        cache::logo_uri(self.app_id)
    }

    fn hero_uri(&self) -> Option<String> {
        cache::hero_uri(self.app_id)
    }

    fn box_art_uri(&self) -> Option<String> {
        cache::box_art_uri(self.app_id)
    }

    fn app_identifier(&self) -> Option<String> {
        Some(self.app_id.to_string())
    }
}
