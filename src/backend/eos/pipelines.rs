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
    is_windows: bool,
    game_root: &Path,
    appid: &str,
    enable_lan: bool,
    disable_online_networking: bool,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    // Find EOS SDK DLLs in the game directory
    let dlls = find_eos_dlls(game_root)?;

    if dlls.is_empty() {
        println!("[splitux] Warning: EOS backend enabled but no EOS SDK DLLs found");
        return Ok(vec![]);
    }

    // Generate unique ports for each instance
    const BASE_PORT: u16 = 55789;
    let instance_ports: Vec<u16> = (0..instances.len())
        .map(|i| BASE_PORT + i as u16)
        .collect();

    let mut overlay_dirs = Vec::new();

    for (idx, instance) in instances.iter().enumerate() {
        // Generate unique IDs for each instance
        let epicid = format!("{:040x}", idx + 1);
        let productuserid = format!("{:040x}", idx + 0x1000);

        // Build broadcast ports (all other instance ports)
        let broadcast_ports: Vec<u16> = instance_ports
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != idx)
            .map(|(_, p)| *p)
            .collect();

        let config = EosConfig {
            appid: appid.to_string(),
            username: instance.profname.clone(),
            epicid,
            productuserid,
            listen_port: instance_ports[idx],
            broadcast_ports,
        };

        let overlay_dir = create_instance_overlay(
            idx,
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
