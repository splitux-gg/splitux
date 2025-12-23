//! Photon backend type definitions

// Re-export UnityBackend from shared bepinex module
pub use crate::bepinex::UnityBackend;

/// Configuration for a Photon instance
#[derive(Debug, Clone)]
pub struct PhotonConfig {
    /// Player name for this instance
    pub player_name: String,
    /// Listen port for local networking
    pub listen_port: u16,
}

/// Base port for Photon networking (different range from Goldberg)
pub const PHOTON_BASE_PORT: u16 = 47684;
