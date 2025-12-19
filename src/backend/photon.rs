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
use std::path::{Path, PathBuf};

use crate::handler::Handler;
use crate::instance::Instance;

mod operations;
mod pipelines;
mod pure;
mod types;

// Re-export types for external use
pub use types::UnityBackend;

// Re-export key functions for direct access
pub use operations::bepinex_backend_available;
pub use pure::detect_unity_backend;

// Re-export pipelines for external use
pub use pipelines::generate_all_configs;

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

/// Photon backend implementation
pub struct Photon;

impl Photon {
    pub fn new() -> Self {
        Self
    }
}

impl Backend for Photon {
    fn name(&self) -> &str {
        "photon"
    }

    fn requires_overlay(&self) -> bool {
        true
    }

    fn create_all_overlays(
        &self,
        _handler: &Handler,
        instances: &[Instance],
        is_windows: bool,
        game_root: &Path,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        pipelines::create_all_overlays(instances, is_windows, game_root)
    }
}
