//! Platform abstraction - WHERE games come from
//!
//! Platforms represent different game distribution services:
//! - Steam: Valve's platform, uses steamlocate for path resolution
//! - GOG: CD Projekt's DRM-free platform (future)
//! - Epic: Epic Games Store (future)
//! - Manual: Direct path specification

use std::error::Error;
use std::path::PathBuf;

/// Platform trait - represents where a game comes from
pub trait Platform {
    /// Platform name for identification
    fn name(&self) -> &str;

    /// Get the game's root directory path
    fn game_root_path(&self) -> Result<PathBuf, Box<dyn Error>>;

    /// Get icon URI for display (optional)
    fn icon_uri(&self) -> Option<String> {
        None
    }

    /// Get logo image URI (optional)
    fn logo_uri(&self) -> Option<String> {
        None
    }

    /// Get hero/banner image URI (optional)
    fn hero_uri(&self) -> Option<String> {
        None
    }

    /// Get box art URI (optional)
    fn box_art_uri(&self) -> Option<String> {
        None
    }

    /// Platform-specific identifier (appid, product id, etc.)
    fn app_identifier(&self) -> Option<String> {
        None
    }
}

/// Enum for serde deserialization of platform configs
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "platform")]
pub enum PlatformConfig {
    #[serde(rename = "steam")]
    Steam { steam_appid: u32 },

    #[serde(rename = "gog")]
    Gog { gog_id: String },

    #[serde(rename = "epic")]
    Epic { epic_app_name: String },

    #[serde(rename = "manual")]
    Manual {
        #[serde(default)]
        path_gameroot: String,
    },
}

impl Default for PlatformConfig {
    fn default() -> Self {
        PlatformConfig::Manual {
            path_gameroot: String::new(),
        }
    }
}

impl PlatformConfig {
    /// Convert platform config enum to a trait object
    pub fn as_platform(&self) -> Box<dyn Platform> {
        match self {
            PlatformConfig::Steam { steam_appid } => Box::new(SteamPlatform::new(*steam_appid)),
            PlatformConfig::Manual { path_gameroot } => {
                Box::new(ManualPlatform::new(path_gameroot.clone()))
            }
            PlatformConfig::Gog { .. } => {
                // Future: Box::new(GogPlatform::new(gog_id.clone()))
                unimplemented!("GOG platform not yet implemented")
            }
            PlatformConfig::Epic { .. } => {
                // Future: Box::new(EpicPlatform::new(epic_app_name.clone()))
                unimplemented!("Epic platform not yet implemented")
            }
        }
    }

    /// Check if this is a Steam platform
    pub fn is_steam(&self) -> bool {
        matches!(self, PlatformConfig::Steam { .. })
    }

    /// Get Steam app ID if this is a Steam platform
    pub fn steam_appid(&self) -> Option<u32> {
        match self {
            PlatformConfig::Steam { steam_appid } => Some(*steam_appid),
            _ => None,
        }
    }
}

mod manual;
mod steam;

pub use manual::ManualPlatform;
pub use steam::SteamPlatform;

// Re-export steam submodule functions for convenience
pub use steam::{find_game_path, get_install_dir_name};
