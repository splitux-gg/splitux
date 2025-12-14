//! Goldberg backend type definitions

use std::path::PathBuf;

/// Information about a Steam API DLL found in the game directory
#[derive(Debug, Clone)]
pub struct SteamApiDll {
    /// Relative path from game root to the DLL
    pub rel_path: PathBuf,
    /// True for 64-bit, false for 32-bit
    pub is_64bit: bool,
    /// Type of DLL (steam_api or steamnetworkingsockets)
    pub dll_type: SteamDllType,
}

/// Type of Steam-related DLL
#[derive(Debug, Clone, PartialEq)]
pub enum SteamDllType {
    /// Standard Steam API (steam_api.dll, steam_api64.dll, libsteam_api.so)
    SteamApi,
    /// GameNetworkingSockets (GameNetworkingSockets.dll)
    NetworkingSockets,
}

/// Configuration for a Goldberg instance
#[derive(Debug, Clone)]
pub struct GoldbergConfig {
    pub app_id: u32,
    pub steam_id: u64,
    pub account_name: String,
    pub listen_port: u16,
    /// Ports of other instances for LAN discovery
    pub broadcast_ports: Vec<u16>,
}

impl GoldbergConfig {
    /// Create a new Goldberg config for an instance
    pub fn new(
        app_id: u32,
        steam_id: u64,
        account_name: String,
        listen_port: u16,
        broadcast_ports: Vec<u16>,
    ) -> Self {
        Self {
            app_id,
            steam_id,
            account_name,
            listen_port,
            broadcast_ports,
        }
    }
}
