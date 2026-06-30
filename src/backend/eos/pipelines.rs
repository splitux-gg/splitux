//! EOS pipelines - high-level orchestration
//!
//! Combines operations into complete workflows.

use std::path::{Path, PathBuf};

use super::operations::{create_instance_overlay, find_eos_dlls};
use super::types::EosConfig;
use crate::instance::Instance;

/// Create EOS overlays for all instances
///
/// Finds EOS DLLs in the game directory and creates per-instance overlays
/// with the Nemirtingas emulator and appropriate configuration.
pub fn create_all_overlays(
    instances: &[Instance],
    global_indices: &[usize],
    is_windows: bool,
    game_root: &Path,
    // Accepted from the handler's `eos.appid` for documentation/forward-compat,
    // but the splitux EOS emu derives identity from the username and ignores it.
    _appid: &str,
    enable_lan: bool,
    disable_online_networking: bool,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    // Find EOS SDK DLLs in the game directory
    let dlls = find_eos_dlls(game_root)?;

    if dlls.is_empty() {
        println!("[splitux] Warning: EOS backend enabled but no EOS SDK DLLs found");
        return Ok(vec![]);
    }

    // Generate unique ports per instance, keyed by GLOBAL index so two concurrent
    // games never reuse a port.
    const BASE_PORT: u16 = 55789;
    let instance_ports: Vec<u16> = global_indices
        .iter()
        .map(|&gi| BASE_PORT + gi as u16)
        .collect();

    let mut overlay_dirs = Vec::new();

    for (idx, instance) in instances.iter().enumerate() {
        // The emu derives its per-instance EpicAccountId + ProductUserId
        // deterministically from the username (set as EOSLAN_USERNAME at launch);
        // no ids/ports are written to a config file here.
        let config = EosConfig {
            username: instance.profname.clone(),
            listen_port: instance_ports[idx],
        };

        let overlay_dir = create_instance_overlay(
            global_indices[idx],
            &dlls,
            &config,
            is_windows,
            enable_lan,
            disable_online_networking,
        )?;

        overlay_dirs.push(overlay_dir);
    }

    Ok(overlay_dirs)
}
