//! Backend trait abstraction - HOW multiplayer is enabled
//!
//! Backends represent different multiplayer networking solutions:
//! - Goldberg: Steam P2P emulation via DLL replacement
//! - EOS: Epic Online Services emulation via DLL replacement
//! - Photon: Unity Photon networking via BepInEx
//! - Facepunch: BepInEx patches for Facepunch.Steamworks
//!
//! Multiple backends can coexist (e.g., Goldberg + Facepunch).

use std::error::Error;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::handler::Handler;
use crate::instance::Instance;

/// Multiplayer backend type (for backward compatibility with old YAML format)
#[derive(Clone, Serialize, Deserialize, Default, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum MultiplayerBackend {
    /// No multiplayer backend (direct launch)
    #[default]
    None,
    /// Goldberg Steam Emulator for Steam P2P games
    Goldberg,
    /// BepInEx + LocalMultiplayer for Photon-based Unity games
    Photon,
}

/// Capability-based trait for multiplayer backends
pub trait Backend {
    /// Backend name for identification
    fn name(&self) -> &str;

    /// Does this backend require filesystem overlays per instance?
    fn requires_overlay(&self) -> bool;

    /// Priority level for overlay stacking (higher = closer to top of overlay stack)
    /// Default is 0 (normal priority). Facepunch uses 10 (high priority).
    fn priority(&self) -> u8 {
        0
    }

    /// Create overlay directories for all instances (batch operation)
    /// Returns a vector of overlay paths, one per instance.
    fn create_all_overlays(
        &self,
        handler: &Handler,
        instances: &[Instance],
        is_windows: bool,
        game_root: &Path,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>>;
}

// Backend module implementations
pub mod eos;
pub mod facepunch;
pub mod goldberg;
pub mod operations;
pub mod photon;
pub mod standalone;

// Re-export settings types for use in Handler
pub use eos::EosSettings;
pub use facepunch::FacepunchSettings;
pub use goldberg::GoldbergSettings;
pub use photon::PhotonSettings;
pub use standalone::StandaloneSettings;

// Use the modular backend implementations
use self::eos as eos_mod;
use self::facepunch as facepunch_mod;
use self::goldberg as goldberg_mod;
use self::photon as photon_mod;
use self::standalone as standalone_mod;

/// Collect enabled backends from handler as trait objects, sorted by priority
fn collect_enabled_backends(handler: &Handler) -> Vec<Box<dyn Backend>> {
    let mut backends: Vec<Box<dyn Backend>> = Vec::new();

    // Collect enabled backends
    if let Some(settings) = handler.goldberg_ref() {
        backends.push(Box::new(goldberg_mod::Goldberg::new(settings.clone())));
    }
    if let Some(settings) = handler.eos_ref() {
        backends.push(Box::new(eos_mod::Eos::new(settings.clone())));
    }
    if handler.photon_ref().is_some() {
        backends.push(Box::new(photon_mod::Photon::new()));
    }
    if let Some(settings) = handler.facepunch_ref() {
        let patches = handler.runtime_patches.clone();
        backends.push(Box::new(facepunch_mod::Facepunch::new(settings.clone(), patches)));
    }
    if let Some(settings) = handler.standalone_ref() {
        backends.push(Box::new(standalone_mod::Standalone::new(settings.clone())));
    }

    // Sort by priority (highest first)
    backends.sort_by(|a, b| b.priority().cmp(&a.priority()));

    backends
}

/// Create overlay directories for all instances based on the handler's backend
///
/// Returns a vector of overlay path lists (one list per instance) to be added to
/// fuse-overlayfs lowerdir stack. Each inner vec contains overlays for that instance,
/// ordered by priority (first = highest priority).
///
/// Backend selection (Phase 7: new optional fields take precedence):
/// - `handler.goldberg.is_some()` enables Goldberg
/// - `handler.photon.is_some()` enables Photon
/// - `handler.facepunch.is_some()` enables Facepunch
/// - Multiple backends can coexist (e.g., Goldberg + Facepunch)
pub fn create_backend_overlays(
    handler: &Handler,
    instances: &[Instance],
    is_windows: bool,
) -> Result<Vec<Vec<PathBuf>>, Box<dyn Error>> {
    let num_instances = instances.len();
    let game_root = PathBuf::from(handler.get_game_rootpath()?);

    // Initialize per-instance overlay lists
    let mut instance_overlays: Vec<Vec<PathBuf>> = (0..num_instances).map(|_| Vec::new()).collect();

    // Collect and process backends via trait dispatch
    let backends = collect_enabled_backends(handler);

    if backends.len() > 1 {
        let names: Vec<&str> = backends.iter().map(|b| b.name()).collect();
        println!("[splitux] Multiple backends enabled: {:?}", names);
    }

    for backend in &backends {
        if backend.requires_overlay() {
            let overlays = backend.create_all_overlays(handler, instances, is_windows, &game_root)?;

            for (i, overlay) in overlays.into_iter().enumerate() {
                if i < num_instances {
                    // Higher priority backends are processed first (due to sorting),
                    // so their overlays go at the front
                    if backend.priority() > 0 {
                        instance_overlays[i].insert(0, overlay);
                    } else {
                        instance_overlays[i].push(overlay);
                    }
                }
            }
        }
    }

    Ok(instance_overlays)
}
