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
use std::path::PathBuf;

mod operations;
mod pipelines;
mod pure;
mod types;

// Re-export types for external use
pub use types::{FacepunchConfig, RuntimePatch};

// Note: Additional re-exports commented out until migration complete (see Phase 9.5)
// pub use operations::{create_instance_overlay, get_linux_bepinex_env};
// pub use pipelines::create_all_overlays;

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

impl FacepunchSettings {
    pub fn is_default(&self) -> bool {
        !self.spoof_identity && !self.force_valid && !self.photon_bypass
    }
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

    fn create_overlay(
        &self,
        instance_idx: usize,
        _handler_path: &PathBuf,
        game_root: &PathBuf,
        is_windows: bool,
    ) -> Result<PathBuf, Box<dyn Error>> {
        use crate::backend::photon::{bepinex_backend_available, detect_unity_backend};

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
        // Note: Real usage should provide proper Steam IDs via generate_steam_id
        let config = FacepunchConfig::new(
            instance_idx,
            format!("Player{}", instance_idx + 1),
            76561198000000000 + instance_idx as u64,
        );

        create_instance_overlay(instance_idx, &config, is_windows, backend)
    }
}

/// Check if handler uses Facepunch backend
pub fn uses_facepunch(settings: &FacepunchSettings, runtime_patches: &[RuntimePatch]) -> bool {
    !settings.is_default() || !runtime_patches.is_empty()
}
