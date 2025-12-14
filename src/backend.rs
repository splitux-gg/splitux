//! Backend trait abstraction - HOW multiplayer is enabled
//!
//! Backends represent different multiplayer networking solutions:
//! - Goldberg: Steam P2P emulation via DLL replacement
//! - Photon: Unity Photon networking via BepInEx
//! - Facepunch: BepInEx patches for Facepunch.Steamworks
//!
//! Multiple backends can coexist (e.g., Goldberg + Facepunch).

use std::error::Error;
use std::path::PathBuf;

// Re-export legacy types for backward compatibility
// All existing code using crate::backend::MultiplayerBackend continues to work
pub use crate::backend_legacy::*;

/// Capability-based trait for multiplayer backends
pub trait Backend {
    /// Backend name for identification
    fn name(&self) -> &str;

    /// Does this backend require filesystem overlays per instance?
    fn requires_overlay(&self) -> bool;

    /// Create overlay directory for a specific instance
    /// Returns the overlay path to be added to fuse-overlayfs lowerdir
    fn create_overlay(
        &self,
        instance_idx: usize,
        handler_path: &PathBuf,
        game_root: &PathBuf,
        is_windows: bool,
    ) -> Result<PathBuf, Box<dyn Error>>;

    /// Cleanup temporary files after game session (optional)
    fn cleanup(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

// Backend module implementations
pub mod facepunch;
pub mod goldberg;
pub mod photon;

// Re-export settings types for use in Handler
pub use facepunch::{Facepunch, FacepunchSettings, RuntimePatch};
pub use goldberg::{Goldberg, GoldbergSettings};
pub use photon::{Photon, PhotonSettings};
