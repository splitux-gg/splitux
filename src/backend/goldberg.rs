//! Goldberg Steam Emulator backend
//!
//! Provides LAN multiplayer via Steam API DLL replacement.
//!
//! ## Module Structure
//! - `types.rs`: Internal types (SteamApiDll, SteamDllType, GoldbergConfig)
//! - `pure/`: Pure functions (bitness detection)
//! - `operations/`: Atomic I/O operations (find DLLs, write settings, create overlay)
//! - `pipelines/`: High-level orchestration (create_all_overlays)

use super::Backend;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use crate::handler::Handler;
use crate::instance::Instance;
use crate::mods::PluginSource;
use crate::profiles::generate_steam_id;

mod operations;
mod pipelines;
mod pure;
mod types;

use operations::find_steam_api_dlls;
use pipelines::create_all_overlays as pipeline_create_all_overlays;
use types::{GoldbergConfig, SteamDllType};

fn default_true() -> bool {
    true
}

/// Goldberg settings from handler YAML (dot-notation: goldberg.*)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GoldbergSettings {
    /// Disable Steam networking (goldberg.disable_networking)
    #[serde(default)]
    pub disable_networking: bool,

    /// Also replace GameNetworkingSockets.dll (goldberg.networking_sockets)
    #[serde(default)]
    pub networking_sockets: bool,

    /// Custom Goldberg settings files (goldberg.settings.*)
    #[serde(default)]
    pub settings: HashMap<String, String>,

    /// Per-instance network namespace + veth into a shared Linux bridge
    /// (goldberg.bridged_lan). Each co-located instance becomes a distinct LAN
    /// host (own IP, own loopback) so each can bind the game port and goldberg's
    /// broadcast LAN discovery flows between them over the bridge. Off by default.
    #[serde(default)]
    pub bridged_lan: bool,

    /// Enable goldberg's opt-in raw-UDP <-> legacy-Steam-P2P bridge by setting
    /// GSE_IP_P2P_BRIDGE=1 for the game (goldberg.p2p_bridge). For IP-LAN games
    /// whose host listens via legacy ISteamNetworking P2P while joiners connect
    /// raw. Off by default.
    #[serde(default)]
    pub p2p_bridge: bool,

    /// Deploy a goldberg `steamclient64.dll`/`steamclient.dll` for games that
    /// resolve Steam through the **steamclient** path instead of `steam_api`
    /// (goldberg.steamclient). Some Windows/Proton titles (e.g. Facepunch /
    /// certain IL2CPP Unity games) never load `steam_api64.dll`; their
    /// `lsteamclient` loads `C:\…\Steam\steamclient64.dll` directly, which under
    /// Proton is the REAL Steam client copied from
    /// `STEAM_COMPAT_CLIENT_INSTALL_PATH/legacycompat/` — so the game falls
    /// through to real Steam (steam://run) and goldberg never engages. When set,
    /// splitux shadows that copy source with goldberg's experimental steamclient
    /// (ro-bind inside the sandbox), so Proton copies goldberg's steamclient into
    /// the prefix and the game loads the emulator offline. Per-instance identity
    /// still flows through GseAppPath/steam_settings. Off by default. See the
    /// goldberg-steamclient-gap memory. Requires the steamclient DLLs to be present
    /// in the goldberg/win asset dir (shipped from gbe_fork's experimental build).
    #[serde(default)]
    pub steamclient: bool,

    /// Plugin source for BepInEx-based plugins (goldberg.plugin.*)
    /// When specified, BepInEx will be installed and the plugin fetched from the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginSource>,

    /// Auto-generate steam_settings/steam_interfaces.txt by scanning the game's own
    /// steam_api DLL/.so at deploy time (goldberg.generate_interfaces). On by default.
    /// steam_interfaces.txt is a derived fact about the binary (like steam_appid.txt),
    /// so it lives in the deploy layer — but the handler config layer stays the source
    /// of truth: a handler that declares goldberg.settings."steam_interfaces.txt"
    /// overrides the generated file, and `false` disables generation entirely.
    /// Generating it lets goldberg resolve every interface version the game requests,
    /// instead of fatally exiting on an unknown one (the "Missing interface" class).
    #[serde(default = "default_true")]
    pub generate_interfaces: bool,

    /// Override the goldberg save/userdata base directory (goldberg.save_path).
    /// splitux always pins GseSavePath to a stable absolute per-profile dir so
    /// goldberg's GetUserDataFolder/saves are deterministic and never fall back to
    /// its in-sandbox default path resolution — which can degrade to a relative
    /// module-name base ("libsteam_api.so/userdata/...") and crash games that
    /// build their data dir from it (e.g. Chronicon). This overrides that default
    /// for the rare game that needs its saves elsewhere. Absolute path; None = the
    /// per-profile default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save_path: Option<String>,
}

impl Default for GoldbergSettings {
    fn default() -> Self {
        Self {
            disable_networking: false,
            networking_sockets: false,
            settings: std::collections::HashMap::new(),
            bridged_lan: false,
            p2p_bridge: false,
            steamclient: false,
            plugin: None,
            generate_interfaces: true,
            save_path: None,
        }
    }
}

/// Goldberg backend implementation
pub struct Goldberg {
    pub settings: GoldbergSettings,
}

impl Goldberg {
    pub fn new(settings: GoldbergSettings) -> Self {
        Self { settings }
    }
}

impl Backend for Goldberg {
    fn name(&self) -> &str {
        "goldberg"
    }

    fn requires_overlay(&self) -> bool {
        true
    }

    fn create_all_overlays(
        &self,
        handler: &Handler,
        instances: &[Instance],
        global_indices: &[usize],
        is_windows: bool,
        game_root: &Path,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        // Find Steam API DLLs in the game directory
        let mut dlls = find_steam_api_dlls(game_root)?;

        // Filter out NetworkingSockets unless explicitly enabled
        if !self.settings.networking_sockets {
            dlls.retain(|dll| dll.dll_type != SteamDllType::NetworkingSockets);
        }

        if dlls.is_empty() {
            println!("[splitux] Warning: Goldberg backend enabled but no Steam API DLLs found");
            return Ok(vec![]);
        }

        // Generate unique ports for each instance, keyed by the GLOBAL index so
        // two concurrent games never reuse a port. `broadcast_ports` below is
        // built from THIS call's instances only (one game), so a unit's LAN
        // discovery stays inside its own lobby — the multi-game isolation.
        const BASE_PORT: u16 = 47584;
        let instance_ports: Vec<u16> = global_indices
            .iter()
            .map(|&gi| BASE_PORT + gi as u16)
            .collect();

        // Build configs for each instance
        let configs: Vec<GoldbergConfig> = instances
            .iter()
            .enumerate()
            .map(|(i, instance)| {
                // Broadcast ports = all other instances' ports IN THIS GAME
                let broadcast_ports: Vec<u16> = instance_ports
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, &port)| port)
                    .collect();

                GoldbergConfig {
                    app_id: handler.get_steam_appid().unwrap_or(480),
                    // First instance of THIS game owns the canonical save → report
                    // the REAL Steam id (so a save_steam_id_remap game reads the
                    // real save in place; goldberg uses the real passwd home, not
                    // $HOME). Extra instances keep a generated id so their lobby
                    // identities stay distinct. `i` is local-to-this-game, so
                    // `i == 0` is the per-game first instance — matching
                    // build_cmds.rs's `is_first_in_game`.
                    steam_id: if i == 0 {
                        crate::save_sync::pure::effective_steam_id(handler, &instance.profname)
                    } else {
                        generate_steam_id(&instance.profname)
                    },
                    account_name: instance.profname.clone(),
                    listen_port: instance_ports[i],
                    broadcast_ports,
                }
            })
            .collect();

        let overlays = pipeline_create_all_overlays(
            &dlls,
            &configs,
            global_indices,
            is_windows,
            &self.settings.settings,
            self.settings.disable_networking,
            &self.settings.plugin,
            game_root,
            self.settings.generate_interfaces,
            self.settings.steamclient,
        )?;

        // SteamStub DRM shadow: some games ship a SteamStub-wrapped exe (a `.bind`
        // section). Run under goldberg it errors "Application load error 3:..." (PE)
        // or hard-needs real Steam (native ELF — e.g. Overcooked! 2), because the
        // stub validates ownership with Steam before the game runs. The fix is a
        // DRM-free exe: Windows PE → Steamless under Proton; native ELF → repoint
        // the entry point to the real _start (the stub never runs). To keep the real
        // install pristine we DON'T patch in place — `ensure_stripped` produces
        // (once) a stripped copy cached under `{PATH_PARTY}/steamless/{appid}/`, and
        // we copy it into each instance overlay at the exe's path so fuse-overlayfs
        // serves it over the real one (the real _Data dir still resolves via the
        // overlay lowerdir). Returns None when the exe isn't DRM-wrapped.
        // Gate: Windows steamclient games (the original case) OR any native goldberg
        // game (ELF SteamStub is detected cheaply and is a no-op when absent).
        if (self.settings.steamclient || !is_windows)
            && let Some(appid) = handler.get_steam_appid() {
                let exec_rel = std::path::Path::new(&handler.exec);
                if let Some(stripped) = operations::ensure_stripped(game_root, exec_rel, appid) {
                    for overlay in &overlays {
                        let dst = overlay.join(exec_rel);
                        if let Some(parent) = dst.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        match std::fs::copy(&stripped, &dst) {
                            Ok(_) => println!(
                                "[splitux] goldberg.steamclient: shadowing DRM exe with Steamless-stripped {} -> {}",
                                stripped.display(),
                                dst.display()
                            ),
                            Err(e) => println!(
                                "[splitux] goldberg.steamclient: failed to shadow stripped exe: {e}"
                            ),
                        }
                    }
                }
            }

        Ok(overlays)
    }
}
