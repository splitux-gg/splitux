//! Goldberg Steam Emulator backend
//!
//! Provides LAN multiplayer via Steam API DLL replacement.
//!
//! ## Module Structure
//! - `types.rs`: SteamApiDll, SteamDllType, GoldbergConfig
//! - `pure/`: Pure functions (bitness detection)
//! - `operations/`: Atomic I/O operations (find DLLs, write settings, create overlay)
//! - `pipelines/`: High-level orchestration (create_all_overlays)

use super::Backend;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

mod operations;
mod pipelines;
mod pure;
mod types;

// Re-export types for external use
pub use types::{GoldbergConfig, SteamApiDll};

// Re-export key functions for direct access
pub use operations::{create_instance_overlay, find_steam_api_dlls};
pub use pipelines::create_all_overlays;

/// Goldberg settings from handler YAML (dot-notation: goldberg.*)
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GoldbergSettings {
    /// Disable Steam networking (goldberg.disable_networking)
    #[serde(default)]
    pub disable_networking: bool,

    /// Also replace GameNetworkingSockets.dll (goldberg.networking_sockets)
    #[serde(default)]
    pub networking_sockets: bool,

    /// Custom Goldberg settings files (goldberg.settings.*)
    #[serde(default)]
    pub settings: HashMap<String, String>,
}

/// Goldberg backend implementation
pub struct Goldberg {
    pub settings: GoldbergSettings,
}

impl Goldberg {
    pub fn new(settings: GoldbergSettings) -> Self {
        Self { settings }
    }

    /// Find Steam API DLLs in a game directory
    pub fn find_dlls(&self, game_dir: &PathBuf) -> Result<Vec<SteamApiDll>, Box<dyn Error>> {
        find_steam_api_dlls(game_dir)
    }

    /// Create overlays for all instances
    pub fn create_overlays(
        &self,
        dlls: &[SteamApiDll],
        configs: &[GoldbergConfig],
        is_windows: bool,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        create_all_overlays(
            dlls,
            configs,
            is_windows,
            &self.settings.settings,
            self.settings.disable_networking,
        )
    }
}

impl Backend for Goldberg {
    fn name(&self) -> &str {
        "goldberg"
    }

    fn requires_overlay(&self) -> bool {
        true
    }

    fn create_overlay(
        &self,
        instance_idx: usize,
        handler_path: &PathBuf,
        game_root: &PathBuf,
        is_windows: bool,
    ) -> Result<PathBuf, Box<dyn Error>> {
        // Find Steam API DLLs in the game directory
        let dlls = find_steam_api_dlls(game_root)?;

        if dlls.is_empty() {
            return Err("No Steam API DLLs found in game directory".into());
        }

        // Generate a config for this instance
        // Note: In real usage, the caller should provide proper configs with
        // unique Steam IDs, ports, etc. This is a simplified single-instance version.
        let config = GoldbergConfig {
            app_id: 480, // Default to Spacewar for testing
            steam_id: 76561198000000000 + instance_idx as u64,
            account_name: format!("Player{}", instance_idx + 1),
            listen_port: 47584 + instance_idx as u16,
            broadcast_ports: vec![], // Will be populated by caller
        };

        create_instance_overlay(
            instance_idx,
            &dlls,
            &config,
            is_windows,
            &self.settings.settings,
            self.settings.disable_networking,
        )
    }
}
