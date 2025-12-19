//! Facepunch backend type definitions

use super::FacepunchSettings;

// Re-export RuntimePatch from handler (canonical definition)
pub use crate::handler::RuntimePatch;

/// Configuration for a Facepunch instance
#[derive(Debug, Clone)]
pub struct FacepunchConfig {
    /// Instance index (0-based)
    pub player_index: usize,
    /// Player name for this instance
    pub account_name: String,
    /// Spoofed Steam ID
    pub steam_id: u64,
    /// Facepunch settings from handler
    pub settings: FacepunchSettings,
    /// Runtime patches from handler
    pub runtime_patches: Vec<RuntimePatch>,
}
