use crate::paths::*;

use std::error::Error;
use std::fs::File;
use std::io::BufReader;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum PadFilterType {
    All,
    NoSteamInput,
    OnlySteamInput,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum WindowManagerType {
    #[default]
    Auto,
    KWin,
    Hyprland,
    GamescopeOnly,
}

/// Photon App IDs for LocalMultiplayer mod
/// Get free App IDs from https://dashboard.photonengine.com
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct PhotonAppIds {
    /// Photon PUN App ID (required for Photon games)
    #[serde(default)]
    pub pun_app_id: String,
    /// Photon Voice App ID (optional, for voice chat)
    #[serde(default)]
    pub voice_app_id: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PartyConfig {
    #[serde(default)]
    pub window_manager: WindowManagerType,
    // Keep enable_kwin_script for backwards compatibility (will be migrated)
    #[serde(default = "default_enable_kwin_script")]
    pub enable_kwin_script: bool,
    pub gamescope_fix_lowres: bool,
    pub gamescope_sdl_backend: bool,
    pub gamescope_force_grab_cursor: bool,
    pub kbm_support: bool,
    pub proton_version: String,
    pub proton_separate_pfxs: bool,
    #[serde(default)]
    pub vertical_two_player: bool,
    pub pad_filter_type: PadFilterType,
    #[serde(default)]
    pub allow_multiple_instances_on_same_device: bool,
    pub disable_mount_gamedirs: bool,
    /// Photon App IDs for games using Photon networking
    #[serde(default)]
    pub photon_app_ids: PhotonAppIds,
}

fn default_enable_kwin_script() -> bool {
    true
}

impl Default for PartyConfig {
    fn default() -> Self {
        PartyConfig {
            window_manager: WindowManagerType::Auto,
            enable_kwin_script: true,
            gamescope_fix_lowres: true,
            gamescope_sdl_backend: true,
            gamescope_force_grab_cursor: false,
            kbm_support: true,
            proton_version: "".to_string(),
            proton_separate_pfxs: true,
            vertical_two_player: false,
            pad_filter_type: PadFilterType::NoSteamInput,
            allow_multiple_instances_on_same_device: false,
            disable_mount_gamedirs: false,
            photon_app_ids: PhotonAppIds::default(),
        }
    }
}

/// Load Photon App IDs from config (convenience function)
pub fn load_photon_ids() -> PhotonAppIds {
    load_cfg().photon_app_ids
}

pub fn load_cfg() -> PartyConfig {
    let path = PATH_PARTY.join("settings.json");

    if let Ok(file) = File::open(path) {
        if let Ok(mut config) = serde_json::from_reader::<_, PartyConfig>(BufReader::new(file)) {
            // Migrate old enable_kwin_script setting to window_manager
            // If enable_kwin_script is false and window_manager is Auto, set to GamescopeOnly
            if !config.enable_kwin_script && config.window_manager == WindowManagerType::Auto {
                config.window_manager = WindowManagerType::GamescopeOnly;
            }
            return config;
        }
    }

    // Return default settings if file doesn't exist or has error
    PartyConfig::default()
}

pub fn save_cfg(config: &PartyConfig) -> Result<(), Box<dyn Error>> {
    let path = PATH_PARTY.join("settings.json");
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, config)?;
    Ok(())
}
