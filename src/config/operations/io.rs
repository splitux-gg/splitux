use crate::config::types::{PhotonAppIds, SplituxConfig, WindowManagerType};
use crate::paths::PATH_PARTY;

use std::error::Error;
use std::fs::File;
use std::io::BufReader;

/// Load Photon App IDs from config (convenience function)
pub fn load_photon_ids() -> PhotonAppIds {
    load_cfg().photon_app_ids
}

pub fn load_cfg() -> SplituxConfig {
    let path = PATH_PARTY.join("settings.json");

    if let Ok(file) = File::open(path) {
        if let Ok(mut config) = serde_json::from_reader::<_, SplituxConfig>(BufReader::new(file)) {
            // Migrate old enable_kwin_script setting to window_manager
            // If enable_kwin_script is false and window_manager is Auto, set to GamescopeOnly
            if !config.enable_kwin_script && config.window_manager == WindowManagerType::Auto {
                config.window_manager = WindowManagerType::GamescopeOnly;
            }
            return config;
        }
    }

    // Return default settings if file doesn't exist or has error
    SplituxConfig::default()
}

pub fn save_cfg(config: &SplituxConfig) -> Result<(), Box<dyn Error>> {
    let path = PATH_PARTY.join("settings.json");
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, config)?;
    Ok(())
}
