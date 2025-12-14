//! Facepunch setup pipeline
//!
//! High-level orchestration for creating Facepunch overlays.

use std::path::{Path, PathBuf};

use crate::backend::photon::{bepinex_backend_available, detect_unity_backend};

use super::super::operations::create_instance_overlay;
use super::super::types::FacepunchConfig;

/// Instance info needed for Facepunch setup
pub struct FacepunchInstance {
    pub profile_name: String,
    pub steam_id: u64,
}

/// Create Facepunch overlays for all instances
pub fn create_all_overlays(
    instances: &[FacepunchInstance],
    is_windows: bool,
    game_dir: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    // Detect Unity backend type
    let backend = detect_unity_backend(game_dir);

    // Check if BepInEx resources exist for this backend
    if !bepinex_backend_available(backend) {
        return Err(format!(
            "BepInEx resources not found for {} backend. Run ./splitux.sh build",
            backend.display_name()
        )
        .into());
    }

    let mut overlays = Vec::new();

    for (i, instance) in instances.iter().enumerate() {
        let config = FacepunchConfig::new(i, instance.profile_name.clone(), instance.steam_id);

        let overlay = create_instance_overlay(i, &config, is_windows, backend)?;
        overlays.push(overlay);
    }

    Ok(overlays)
}
