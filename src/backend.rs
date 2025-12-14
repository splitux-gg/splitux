//! Backend trait abstraction - HOW multiplayer is enabled
//!
//! Backends represent different multiplayer networking solutions:
//! - Goldberg: Steam P2P emulation via DLL replacement
//! - Photon: Unity Photon networking via BepInEx
//! - Facepunch: BepInEx patches for Facepunch.Steamworks
//!
//! Multiple backends can coexist (e.g., Goldberg + Facepunch).

use std::error::Error;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::handler::Handler;
use crate::instance::Instance;
use crate::profiles::generate_steam_id;

/// Multiplayer backend type (for backward compatibility with old YAML format)
#[derive(Clone, Serialize, Deserialize, Default, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum MultiplayerBackend {
    /// No multiplayer backend (direct launch)
    #[default]
    None,
    /// Goldberg Steam Emulator for Steam P2P games
    Goldberg,
    /// BepInEx + LocalMultiplayer for Photon-based Unity games
    Photon,
}

impl MultiplayerBackend {
    /// Get human-readable display name
    pub fn display_name(&self) -> &'static str {
        match self {
            MultiplayerBackend::None => "None",
            MultiplayerBackend::Goldberg => "Goldberg (Steam)",
            MultiplayerBackend::Photon => "Photon (BepInEx)",
        }
    }
}

/// Capability-based trait for multiplayer backends
pub trait Backend {
    /// Backend name for identification
    fn name(&self) -> &str;

    /// Does this backend require filesystem overlays per instance?
    fn requires_overlay(&self) -> bool;

    /// Create overlay directory for a specific instance
    /// Returns the overlay path to be added to fuse-overlayfs lowerdir
    fn create_overlay(
        &self,
        instance_idx: usize,
        handler_path: &PathBuf,
        game_root: &PathBuf,
        is_windows: bool,
    ) -> Result<PathBuf, Box<dyn Error>>;

    /// Cleanup temporary files after game session (optional)
    fn cleanup(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

// Backend module implementations
pub mod facepunch;
pub mod goldberg;
pub mod photon;

// Re-export settings types for use in Handler
pub use facepunch::FacepunchSettings;
pub use goldberg::GoldbergSettings;
pub use photon::PhotonSettings;

// Import top-level modules for overlay creation (avoiding conflicts with backend submodules)
use crate::facepunch as facepunch_mod;
use crate::goldberg as goldberg_mod;
use crate::photon as photon_mod;

/// Create overlay directories for all instances based on the handler's backend
///
/// Returns a vector of overlay path lists (one list per instance) to be added to
/// fuse-overlayfs lowerdir stack. Each inner vec contains overlays for that instance,
/// ordered by priority (first = highest priority).
///
/// Backend selection (Phase 7: new optional fields take precedence):
/// - `handler.goldberg.is_some()` enables Goldberg
/// - `handler.photon.is_some()` enables Photon
/// - `handler.facepunch.is_some()` enables Facepunch
/// - Multiple backends can coexist (e.g., Goldberg + Facepunch)
pub fn create_backend_overlays(
    handler: &Handler,
    instances: &[Instance],
    is_windows: bool,
) -> Result<Vec<Vec<PathBuf>>, Box<dyn Error>> {
    let num_instances = instances.len();

    // Initialize per-instance overlay lists
    let mut instance_overlays: Vec<Vec<PathBuf>> = (0..num_instances).map(|_| Vec::new()).collect();

    // Check for Goldberg backend (new optional field)
    if handler.has_goldberg() {
        let goldberg_overlays = create_goldberg_overlays(handler, instances, is_windows)?;
        for (i, overlay) in goldberg_overlays.into_iter().enumerate() {
            if i < num_instances {
                instance_overlays[i].push(overlay);
            }
        }
    }

    // Check for Photon backend (new optional field)
    if handler.has_photon() {
        let photon_overlays = create_photon_overlays(handler, instances, is_windows)?;
        for (i, overlay) in photon_overlays.into_iter().enumerate() {
            if i < num_instances {
                instance_overlays[i].push(overlay);
            }
        }
    }

    // Check for Facepunch backend (new optional field, can coexist with others)
    if handler.has_facepunch() {
        let game_root = PathBuf::from(handler.get_game_rootpath()?);
        let facepunch_overlays = facepunch_mod::create_all_overlays(handler, instances, is_windows, &game_root)?;

        // Facepunch overlays have highest priority (insert at front)
        for (i, fp_overlay) in facepunch_overlays.into_iter().enumerate() {
            if i < num_instances {
                instance_overlays[i].insert(0, fp_overlay);
            }
        }

        if handler.has_goldberg() || handler.has_photon() {
            println!("[splitux] Merging Facepunch overlays with other backends");
        }
    }

    Ok(instance_overlays)
}

/// Create Goldberg Steam Emulator overlays for all instances
fn create_goldberg_overlays(
    handler: &Handler,
    instances: &[Instance],
    is_windows: bool,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    // Get Goldberg settings from new optional field
    let goldberg_settings = handler.goldberg_ref()
        .ok_or("Goldberg backend not enabled")?;

    let game_root = PathBuf::from(handler.get_game_rootpath()?);
    let mut dlls = goldberg_mod::find_steam_api_dlls(&game_root)?;

    // Filter out NetworkingSockets unless explicitly enabled
    // Most games work better with disable_steam config patch instead
    if !goldberg_settings.networking_sockets {
        dlls.retain(|dll| dll.dll_type != goldberg_mod::SteamDllType::NetworkingSockets);
    }

    if dlls.is_empty() {
        println!("[splitux] Warning: Goldberg backend enabled but no Steam API DLLs found");
        return Ok(vec![]);
    }

    // Generate unique ports for each instance
    const BASE_PORT: u16 = 47584;
    let instance_ports: Vec<u16> = (0..instances.len())
        .map(|i| BASE_PORT + i as u16)
        .collect();

    // Build configs for each instance
    let configs: Vec<goldberg_mod::GoldbergConfig> = instances
        .iter()
        .enumerate()
        .map(|(i, instance)| {
            // Broadcast ports = all other instances' ports
            let broadcast_ports: Vec<u16> = instance_ports
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, &port)| port)
                .collect();

            goldberg_mod::GoldbergConfig {
                app_id: handler.steam_appid.unwrap_or(480),
                steam_id: generate_steam_id(&instance.profname),
                account_name: instance.profname.clone(),
                listen_port: instance_ports[i],
                broadcast_ports,
            }
        })
        .collect();

    goldberg_mod::create_all_overlays(
        &dlls,
        &configs,
        is_windows,
        &goldberg_settings.settings,
        goldberg_settings.disable_networking,
    )
}

/// Create Photon/BepInEx overlays for all instances
fn create_photon_overlays(
    handler: &Handler,
    instances: &[Instance],
    is_windows: bool,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let game_root = PathBuf::from(handler.get_game_rootpath()?);
    photon_mod::create_all_overlays(handler, instances, is_windows, &game_root)
}
