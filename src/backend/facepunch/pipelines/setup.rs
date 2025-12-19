//! Facepunch setup pipeline
//!
//! High-level orchestration for creating Facepunch overlays.

use std::path::{Path, PathBuf};

use crate::backend::photon::{bepinex_backend_available, detect_unity_backend};
use crate::handler::RuntimePatch;
use crate::instance::Instance;
use crate::profiles::generate_steam_id;

use super::super::operations::create_instance_overlay;
use super::super::types::FacepunchConfig;
use super::super::FacepunchSettings;

/// Create Facepunch overlays for all instances
pub fn create_all_overlays(
    settings: &FacepunchSettings,
    runtime_patches: &[RuntimePatch],
    instances: &[Instance],
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
        let config = FacepunchConfig {
            player_index: i,
            account_name: instance.profname.clone(),
            steam_id: generate_steam_id(&instance.profname),
            settings: settings.clone(),
            runtime_patches: runtime_patches.to_vec(),
        };

        let overlay = create_instance_overlay(i, &config, is_windows, backend)?;
        overlays.push(overlay);
    }

    Ok(overlays)
}
