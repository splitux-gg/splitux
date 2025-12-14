//! Photon backend type definitions

/// Unity scripting backend type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnityBackend {
    /// Mono backend (older games, has GAME_Data/Managed/*.dll)
    Mono,
    /// IL2CPP backend (newer games, has GameAssembly.dll)
    Il2Cpp,
}

impl UnityBackend {
    /// Get the BepInEx subdirectory name for this backend
    pub fn bepinex_subdir(&self) -> &'static str {
        match self {
            UnityBackend::Mono => "mono",
            UnityBackend::Il2Cpp => "il2cpp",
        }
    }

    /// Get display name for this backend
    pub fn display_name(&self) -> &'static str {
        match self {
            UnityBackend::Mono => "Mono",
            UnityBackend::Il2Cpp => "IL2CPP",
        }
    }
}

/// Configuration for a Photon instance
#[derive(Debug, Clone)]
pub struct PhotonConfig {
    /// Instance index (0-based)
    pub instance_idx: usize,
    /// Player name for this instance
    pub player_name: String,
    /// Listen port for local networking
    pub listen_port: u16,
    /// Ports of other instances for discovery
    pub broadcast_ports: Vec<u16>,
}

impl PhotonConfig {
    /// Create a new Photon config
    pub fn new(
        instance_idx: usize,
        player_name: String,
        listen_port: u16,
        broadcast_ports: Vec<u16>,
    ) -> Self {
        Self {
            instance_idx,
            player_name,
            listen_port,
            broadcast_ports,
        }
    }
}

/// Base port for Photon networking (different range from Goldberg)
pub const PHOTON_BASE_PORT: u16 = 47684;
