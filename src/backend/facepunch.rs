//! Facepunch.Steamworks backend for Unity games
//!
//! Provides multiplayer via BepInEx with SplituxFacepunch plugin.
//!
//! ## Module Structure
//! - `types.rs`: FacepunchConfig, RuntimePatch
//! - `pure/`: Pure functions (config generation)
//! - `operations/`: Atomic I/O operations (BepInEx install, overlay creation)
//! - `pipelines/`: High-level orchestration (create_all_overlays)

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
pub use types::RuntimePatch;

// Re-exports for external use
pub use operations::get_linux_bepinex_env;

/// Facepunch settings from handler YAML (dot-notation: facepunch.*)
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct FacepunchSettings {
    /// Spoof SteamClient.SteamId and SteamClient.Name to unique per-instance values
    #[serde(default)]
    pub spoof_identity: bool,

    /// Force SteamClient.IsValid and IsLoggedOn to return true
    #[serde(default)]
    pub force_valid: bool,

    /// Bypass Photon Steam authentication (AuthType=255)
    #[serde(default)]
    pub photon_bypass: bool,
}

/// Facepunch backend implementation
pub struct Facepunch {
    pub settings: FacepunchSettings,
    pub runtime_patches: Vec<RuntimePatch>,
}

impl Facepunch {
    pub fn new(settings: FacepunchSettings, runtime_patches: Vec<RuntimePatch>) -> Self {
        Self {
            settings,
            runtime_patches,
        }
    }
}

impl Backend for Facepunch {
    fn name(&self) -> &str {
        "facepunch"
    }

    fn requires_overlay(&self) -> bool {
        true
    }

    /// Facepunch has highest priority (overlays go at front of stack)
    fn priority(&self) -> u8 {
        10
    }

    fn create_all_overlays(
        &self,
        _handler: &Handler,
        instances: &[Instance],
        is_windows: bool,
        game_root: &Path,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        pipelines::create_all_overlays(
            &self.settings,
            &self.runtime_patches,
            instances,
            is_windows,
            game_root,
        )
    }
}
