//! Photon/BepInEx backend for Unity games
//!
//! Provides multiplayer via BepInEx mods for Unity Photon networking.
//!
//! ## Module Structure
//! - `types.rs`: UnityBackend, PhotonConfig, PHOTON_BASE_PORT
//! - `pure/`: Pure functions (Unity backend detection)
//! - `operations/`: Atomic I/O operations (BepInEx install, config gen, symlinks)
//! - `pipelines/`: High-level orchestration (create_all_overlays, generate_all_configs)

use super::Backend;
use std::error::Error;
use std::path::PathBuf;

mod operations;
mod pipelines;
mod pure;
mod types;

// Re-export types for external use
pub use types::{PhotonConfig, UnityBackend, PHOTON_BASE_PORT};

// Re-export key functions for direct access
pub use operations::{
    bepinex_available, bepinex_backend_available, create_instance_overlay,
    generate_instance_config, get_bepinex_res_path, setup_shared_files, PhotonAppIds,
};
pub use pipelines::{create_all_overlays, generate_all_configs, PhotonInstance};
pub use pure::detect_unity_backend;

/// Photon settings from handler YAML (dot-notation: photon.*)
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PhotonSettings {
    /// Path pattern for LocalMultiplayer config file within profile's windata
    /// Example: "AppData/LocalLow/CompanyName/GameName/LocalMultiplayer/global.cfg"
    #[serde(default)]
    pub config_path: String,

    /// Files that should be shared between all instances (relative to windata)
    #[serde(default)]
    pub shared_files: Vec<String>,
}

impl PhotonSettings {
    pub fn is_empty(&self) -> bool {
        self.config_path.is_empty() && self.shared_files.is_empty()
    }
}

/// Photon backend implementation
pub struct Photon {
    pub settings: PhotonSettings,
}

impl Photon {
    pub fn new(settings: PhotonSettings) -> Self {
        Self { settings }
    }
}

impl Backend for Photon {
    fn name(&self) -> &str {
        "photon"
    }

    fn requires_overlay(&self) -> bool {
        true
    }

    fn create_overlay(
        &self,
        instance_idx: usize,
        _handler_path: &PathBuf,
        game_root: &PathBuf,
        is_windows: bool,
    ) -> Result<PathBuf, Box<dyn Error>> {
        // Detect Unity backend
        let backend = detect_unity_backend(game_root);

        // Check if BepInEx is available for this backend
        if !bepinex_backend_available(backend) {
            return Err(format!(
                "BepInEx resources not found for {} backend. Run ./splitux.sh build",
                backend.display_name()
            )
            .into());
        }

        // Create a config for this instance
        let config = PhotonConfig::new(
            instance_idx,
            format!("Player{}", instance_idx + 1),
            PHOTON_BASE_PORT + instance_idx as u16,
            vec![], // Broadcast ports populated by caller
        );

        create_instance_overlay(instance_idx, &config, is_windows, backend)
    }
}
