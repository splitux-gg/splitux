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

/// Configuration for an EOS emulator instance.
///
/// The splitux EOS emu (eos_sdk_emu) is hand-rolled and configured via its
/// native EOSLAN_* env at launch (see build_cmds: EOSLAN_USERNAME per instance,
/// plus the handler's EOSLAN_LOCALHOST_MODE / EOSLAN_P2P_BASE_PORT). No
/// Nemirtingas JSON is written, so the old epic/product-id + appid fields are
/// gone — identity is derived deterministically by the emu from the username.
#[derive(Debug, Clone)]
pub struct EosConfig {
    pub username: String,
    pub listen_port: u16,
}
