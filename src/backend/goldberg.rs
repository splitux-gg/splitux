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
}

impl Default for GoldbergSettings {
    fn default() -> Self {
        Self {
            disable_networking: false,
            networking_sockets: false,
            settings: std::collections::HashMap::new(),
            bridged_lan: false,
            p2p_bridge: false,
            plugin: None,
            generate_interfaces: true,
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
        is_windows: bool,
        game_root: &Path,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        // Find Steam API DLLs in the game directory
        let mut dlls = find_steam_api_dlls(&game_root.to_path_buf())?;

        // Filter out NetworkingSockets unless explicitly enabled
        if !self.settings.networking_sockets {
            dlls.retain(|dll| dll.dll_type != SteamDllType::NetworkingSockets);
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
        let configs: Vec<GoldbergConfig> = instances
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

                GoldbergConfig {
                    app_id: handler.get_steam_appid().unwrap_or(480),
                    steam_id: generate_steam_id(&instance.profname),
                    account_name: instance.profname.clone(),
                    listen_port: instance_ports[i],
                    broadcast_ports,
                }
            })
            .collect();

        pipeline_create_all_overlays(
            &dlls,
            &configs,
            is_windows,
            &self.settings.settings,
            self.settings.disable_networking,
            &self.settings.plugin,
            game_root,
            self.settings.generate_interfaces,
        )
    }
}
