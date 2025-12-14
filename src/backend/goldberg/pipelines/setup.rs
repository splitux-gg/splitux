//! Goldberg setup pipeline
//!
//! High-level orchestration for creating Goldberg overlays.

use std::collections::HashMap;
use std::path::PathBuf;

use super::super::operations::create_instance_overlay;
use super::super::types::{GoldbergConfig, SteamApiDll};

/// Create Goldberg overlays for all game instances
///
/// Returns a vector of overlay directory paths, one per instance.
/// Each overlay contains Goldberg DLLs and steam_settings configured
/// for that specific instance (unique Steam ID, port, etc.).
pub fn create_all_overlays(
    dlls: &[SteamApiDll],
    configs: &[GoldbergConfig],
    is_windows: bool,
    handler_settings: &HashMap<String, String>,
    disable_networking: bool,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut overlays = Vec::new();

    for (i, config) in configs.iter().enumerate() {
        let overlay =
            create_instance_overlay(i, dlls, config, is_windows, handler_settings, disable_networking)?;
        overlays.push(overlay);
    }

    Ok(overlays)
}
