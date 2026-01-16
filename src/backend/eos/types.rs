//! EOS backend type definitions

use std::path::PathBuf;

/// Information about an EOS SDK DLL found in the game directory
#[derive(Debug, Clone)]
pub struct EosDll {
    /// Relative path from game root to the DLL
    pub rel_path: PathBuf,
    /// True for 64-bit, false for 32-bit
    pub is_64bit: bool,
}

/// Configuration for an EOS emulator instance
#[derive(Debug, Clone)]
pub struct EosConfig {
    pub appid: String,
    pub username: String,
    pub epicid: String,
    pub productuserid: String,
    pub listen_port: u16,
    /// Ports of other instances for LAN discovery
    pub broadcast_ports: Vec<u16>,
}
