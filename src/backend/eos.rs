//! EOS (Epic Online Services) Emulator backend
//!
//! Provides LAN multiplayer via EOS SDK DLL replacement using Nemirtingas emulator.
//!
//! ## Module Structure
//! - `types.rs`: Internal types (EosDll, EosConfig)
//! - `operations/`: Atomic I/O operations (find DLLs, write settings, create overlay)
//! - `pipelines.rs`: High-level orchestration (create_all_overlays)

use super::Backend;
use std::error::Error;
use std::path::{Path, PathBuf};

use crate::handler::Handler;
use crate::instance::Instance;

mod operations;
mod pipelines;
mod types;

use pipelines::create_all_overlays as pipeline_create_all_overlays;

/// EOS emulator settings from handler YAML (dot-notation: eos.*)
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct EosSettings {
    /// Epic Games application ID (eos.appid)
    #[serde(default)]
    pub appid: String,

    /// Enable LAN discovery (eos.enable_lan)
    #[serde(default = "default_true")]
    pub enable_lan: bool,

    /// Disable online networking, LAN only (eos.disable_online_networking)
    #[serde(default = "default_true")]
    pub disable_online_networking: bool,
}

fn default_true() -> bool {
    true
}

/// EOS backend implementation
pub struct Eos {
    pub settings: EosSettings,
}

impl Eos {
    pub fn new(settings: EosSettings) -> Self {
        Self { settings }
    }
}

impl Backend for Eos {
    fn name(&self) -> &str {
        "eos"
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
        // Use appid from settings, fallback to handler's steam_appid if not set
        let appid = if self.settings.appid.is_empty() {
            handler
                .steam_appid
                .map(|id| id.to_string())
                .unwrap_or_else(|| "InvalidAppid".to_string())
        } else {
            self.settings.appid.clone()
        };

        pipeline_create_all_overlays(
            instances,
            is_windows,
            game_root,
            &appid,
            self.settings.enable_lan,
            self.settings.disable_online_networking,
        )
    }
}
