//! Backend trait abstraction - HOW multiplayer is enabled
//!
//! Backends represent different multiplayer networking solutions:
//! - Goldberg: Steam P2P emulation via DLL replacement
//! - EOS: Epic Online Services emulation via DLL replacement
//! - Photon: Unity Photon networking via BepInEx
//! - Facepunch: BepInEx patches for Facepunch.Steamworks
//!
//! Multiple backends can coexist (e.g., Goldberg + Facepunch).

use std::error::Error;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::handler::Handler;
use crate::instance::Instance;

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

/// Capability-based trait for multiplayer backends
pub trait Backend {
    /// Backend name for identification
    fn name(&self) -> &str;

    /// Does this backend require filesystem overlays per instance?
    fn requires_overlay(&self) -> bool;

    /// Priority level for overlay stacking (higher = closer to top of overlay stack)
    /// Default is 0 (normal priority). Facepunch uses 10 (high priority).
    fn priority(&self) -> u8 {
        0
    }

    /// Create overlay directories for all instances (batch operation)
    /// Returns a vector of overlay paths, one per instance (parallel to
    /// `instances`).
    ///
    /// `global_indices[k]` is the GLOBAL (launch-wide) index of `instances[k]`.
    /// In multi-game mode this batch is called once PER GAME with that game's
    /// instances, so the local position `k` and the global index differ; the
    /// global index names per-instance scratch dirs (and goldberg ports) so two
    /// games never collide on `…-overlay-0`/port. Single-game: `global_indices`
    /// is `[0,1,…]`, identical to the old local index.
    fn create_all_overlays(
        &self,
        handler: &Handler,
        instances: &[Instance],
        global_indices: &[usize],
        is_windows: bool,
        game_root: &Path,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>>;

    /// Extra CLI args this backend injects into the game launch (e.g. Keen's
    /// `--keenonline-server-data-file <file>`). Default: none. `is_windows` is
    /// the game's Proton/wine-ness so backends can emit Windows-style paths.
    fn extra_launch_args(&self, _handler: &Handler, _is_windows: bool) -> Vec<String> {
        Vec::new()
    }

    /// Start any shared sidecar services this backend needs (e.g. Keen's auth
    /// server). Returns child processes the session owns and kills at teardown.
    /// Default: none (in-process DLL backends like Goldberg/EOS need nothing).
    fn start_services(&self, _handler: &Handler) -> std::io::Result<Vec<std::process::Child>> {
        Ok(Vec::new())
    }
}

// Backend module implementations
pub mod eos;
pub mod facepunch;
pub mod goldberg;
pub mod keen;
pub mod operations;
pub mod photon;
pub mod standalone;

// Re-export settings types for use in Handler
pub use eos::EosSettings;
pub use facepunch::FacepunchSettings;
pub use goldberg::GoldbergSettings;
pub use keen::KeenSettings;
pub use photon::PhotonSettings;
pub use standalone::StandaloneSettings;

// Use the modular backend implementations
use self::eos as eos_mod;
use self::facepunch as facepunch_mod;
use self::goldberg as goldberg_mod;
use self::keen as keen_mod;
use self::photon as photon_mod;
use self::standalone as standalone_mod;

/// Collect enabled backends from handler as trait objects, sorted by priority
fn collect_enabled_backends(handler: &Handler) -> Vec<Box<dyn Backend>> {
    let mut backends: Vec<Box<dyn Backend>> = Vec::new();

    // Collect enabled backends
    if let Some(settings) = handler.goldberg_ref() {
        backends.push(Box::new(goldberg_mod::Goldberg::new(settings.clone())));
    }
    if let Some(settings) = handler.eos_ref() {
        backends.push(Box::new(eos_mod::Eos::new(settings.clone())));
    }
    if handler.photon_ref().is_some() {
        backends.push(Box::new(photon_mod::Photon::new()));
    }
    if let Some(settings) = handler.facepunch_ref() {
        let patches = handler.runtime_patches.clone();
        backends.push(Box::new(facepunch_mod::Facepunch::new(settings.clone(), patches)));
    }
    if let Some(settings) = handler.standalone_ref() {
        backends.push(Box::new(standalone_mod::Standalone::new(settings.clone())));
    }
    if let Some(settings) = handler.keen_ref() {
        backends.push(Box::new(keen_mod::Keen::new(settings.clone())));
    }

    // Sort by priority (highest first)
    backends.sort_by(|a, b| b.priority().cmp(&a.priority()));

    backends
}

/// Collect all backend-injected launch args for a game's handler, in backend
/// priority order. Called per-instance when building the launch command.
pub fn collect_backend_launch_args(handler: &Handler, is_windows: bool) -> Vec<String> {
    let mut args = Vec::new();
    for backend in collect_enabled_backends(handler) {
        args.extend(backend.extra_launch_args(handler, is_windows));
    }
    args
}

/// Start shared sidecar services for all games in the launch (e.g. the Keen
/// auth server). Deduplicated by backend name so a single shared sidecar is
/// started once even with multiple instances/games. Returns the spawned child
/// processes for the session to kill at teardown.
pub fn start_backend_services(handlers: &[Handler]) -> Vec<std::process::Child> {
    let mut children = Vec::new();
    let mut started: std::collections::HashSet<String> = std::collections::HashSet::new();
    for handler in handlers {
        for backend in collect_enabled_backends(handler) {
            if !started.insert(backend.name().to_string()) {
                continue;
            }
            match backend.start_services(handler) {
                Ok(cs) => children.extend(cs),
                Err(e) => println!(
                    "[splitux] backend '{}' start_services failed: {}",
                    backend.name(),
                    e
                ),
            }
        }
    }
    children
}

/// Create overlay directories for all instances, per UNIT (game).
///
/// Returns a GLOBAL-indexed vector of overlay path lists (one list per instance,
/// parallel to `instances`) to be added to the fuse-overlayfs lowerdir stack.
/// Each inner vec is ordered by priority (first = highest priority).
///
/// Multi-game: instances are grouped by `Instance.game`, and each group's
/// overlays are built from THAT game's handler (`handlers[game]`) — so mixed
/// backends across games work (one goldberg game + one photon game) and, for
/// goldberg, each unit's LAN lobby stays self-contained (`create_all_overlays`
/// only sees its own game's instances, so `broadcast_ports` never crosses into
/// another game). The GLOBAL instance index names per-instance scratch dirs and
/// goldberg ports, keeping units disjoint on disk and on the wire.
///
/// A game whose handler is not a saved-handler contributes empty overlay lists
/// (the caller's mount step skips it), matching the legacy single-game gate.
///
/// Backend selection per game (Phase 7: optional fields take precedence):
/// - `handler.goldberg.is_some()` enables Goldberg
/// - `handler.photon.is_some()` enables Photon
/// - `handler.facepunch.is_some()` enables Facepunch
/// - Multiple backends can coexist (e.g., Goldberg + Facepunch)
pub fn create_backend_overlays(
    handlers: &[Handler],
    instances: &[Instance],
) -> Result<Vec<Vec<PathBuf>>, Box<dyn Error>> {
    let num_instances = instances.len();

    // Initialize per-instance (GLOBAL-indexed) overlay lists
    let mut instance_overlays: Vec<Vec<PathBuf>> = (0..num_instances).map(|_| Vec::new()).collect();

    // Group GLOBAL instance indices by game (unit), in first-seen launch order.
    let mut games_in_order: Vec<usize> = Vec::new();
    let mut idxs_by_game: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for (gi, inst) in instances.iter().enumerate() {
        idxs_by_game
            .entry(inst.game)
            .or_insert_with(|| {
                games_in_order.push(inst.game);
                Vec::new()
            })
            .push(gi);
    }

    for game in games_in_order {
        let handler = &handlers[game];
        // Non-saved-handler games have no overlays to build (legacy gate, now
        // per-game): leave their instances' lists empty.
        if !handler.is_saved_handler() {
            continue;
        }
        let global_idxs = &idxs_by_game[&game];
        // This unit's instances, in launch order, parallel to `global_idxs`.
        let unit_instances: Vec<Instance> =
            global_idxs.iter().map(|&gi| instances[gi].clone()).collect();
        let game_root = PathBuf::from(handler.get_game_rootpath()?);
        // Windows-ness is per game (one unit may be a Proton title, another
        // native), so derive it from this game's handler, not a launch-wide flag.
        let is_windows = handler.win();

        let backends = collect_enabled_backends(handler);
        if backends.len() > 1 {
            let names: Vec<&str> = backends.iter().map(|b| b.name()).collect();
            println!("[splitux] Game {}: multiple backends enabled: {:?}", game, names);
        }

        for backend in &backends {
            if backend.requires_overlay() {
                let overlays = backend.create_all_overlays(
                    handler,
                    &unit_instances,
                    global_idxs,
                    is_windows,
                    &game_root,
                )?;

                // Scatter this unit's (locally-indexed) overlays back to GLOBAL
                // instance slots.
                for (local_k, overlay) in overlays.into_iter().enumerate() {
                    let Some(&gi) = global_idxs.get(local_k) else {
                        continue;
                    };
                    // Higher-priority backends are processed first (sorted), so
                    // their overlays go at the front of the lowerdir stack.
                    if backend.priority() > 0 {
                        instance_overlays[gi].insert(0, overlay);
                    } else {
                        instance_overlays[gi].push(overlay);
                    }
                }
            }
        }
    }

    Ok(instance_overlays)
}
