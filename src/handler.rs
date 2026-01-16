mod io;
mod operations;
pub mod pure;
#[cfg(test)]
mod tests;
mod types;

// Re-export types from submodule
pub use types::{FacepunchSettings, PhotonSettings, RequiredMod, RuntimePatch, SDL2Override, is_default_sdl2};
// Re-export I/O functions from submodule
pub use io::{import_handler, scan_handlers};

use crate::backend::{
    EosSettings as BackendEosSettings, FacepunchSettings as BackendFacepunchSettings,
    GoldbergSettings as BackendGoldbergSettings, MultiplayerBackend,
    PhotonSettings as BackendPhotonSettings,
};
use crate::gptokeyb::GptokeybSettings;
use crate::util::SanitizePath;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub const HANDLER_SPEC_CURRENT_VERSION: u16 = 3;

fn is_default_backend(b: &MultiplayerBackend) -> bool {
    *b == MultiplayerBackend::None
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Handler {
    // Members that are determined by context (not serialized)
    #[serde(skip)]
    pub path_handler: PathBuf,
    #[serde(skip)]
    pub img_paths: Vec<PathBuf>,

    // Required fields
    pub name: String,
    pub exec: String,

    // Optional metadata
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub info: String,
    #[serde(default, skip_serializing_if = "is_default_spec_ver")]
    pub spec_ver: u16,

    // Game location (one of these should be set)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_appid: Option<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path_gameroot: String,

    /// Platform configuration (Phase 10: unified platform abstraction)
    /// When set, takes precedence over steam_appid/path_gameroot
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<crate::platform::PlatformConfig>,

    // Launch configuration
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub runtime: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub args: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub env: String,
    #[serde(default, skip_serializing_if = "is_default_sdl2")]
    pub sdl2_override: SDL2Override,
    /// Path to Proton installation. If set, uses direct Proton instead of umu-run.
    /// Example: "Proton - Experimental" or full path like "/path/to/proton"
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub proton_path: String,

    // Multiplayer settings
    /// Multiplayer backend to use (none, goldberg, photon)
    #[serde(default, skip_serializing_if = "is_default_backend")]
    pub backend: MultiplayerBackend,
    /// DEPRECATED: Use `backend: goldberg` instead. Kept for backwards compatibility.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub use_goldberg: bool,
    /// Game-specific Goldberg settings files.
    /// Keys are filenames (e.g., "force_lobby_type.txt", "invite_all.txt")
    /// Values are file contents (use empty string for empty files)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub goldberg_settings: HashMap<String, String>,
    /// Disable Steam Networking Sockets in Goldberg.
    /// Set to true for games that have lobby discovery issues with SNS.
    /// This forces the game to use legacy networking for LAN discovery.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub goldberg_disable_networking: bool,
    /// Also replace GameNetworkingSockets.dll with Goldberg's version.
    /// Most games work better with disable_steam config patch instead.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub goldberg_networking_sockets: bool,
    /// Photon-specific settings (only used when backend = photon)
    #[serde(default, skip_serializing_if = "PhotonSettings::is_empty")]
    pub photon_settings: PhotonSettings,

    /// Facepunch.Steamworks patch settings for SplituxFacepunch plugin
    /// Presence enables Facepunch backend (can coexist with Goldberg)
    #[serde(default, skip_serializing_if = "FacepunchSettings::is_default")]
    pub facepunch_settings: FacepunchSettings,

    /// Runtime patches for game-specific classes (used by SplituxFacepunch)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_patches: Vec<RuntimePatch>,

    // ============= NEW BACKEND SETTINGS (Phase 7) =============
    // These optional fields enable backends by presence.
    // Multiple backends can be enabled simultaneously.

    /// Goldberg Steam Emulator settings (enables Goldberg if Some)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goldberg: Option<BackendGoldbergSettings>,

    /// Photon/BepInEx settings (enables Photon if Some)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photon: Option<BackendPhotonSettings>,

    /// Facepunch.Steamworks settings (enables Facepunch if Some)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facepunch: Option<BackendFacepunchSettings>,

    /// EOS (Epic Online Services) emulator settings (enables EOS if Some)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eos: Option<BackendEosSettings>,

    /// Required mods/files that must be installed by the user
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_mods: Vec<RequiredMod>,

    // Advanced
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub game_null_paths: Vec<String>,
    /// Disable bwrap container (may be needed for games with networking issues)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_bwrap: bool,
    /// Disable input device isolation (for games where mods handle input internally)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_input_isolation: bool,

    /// gptokeyb settings for controller→keyboard/mouse translation
    /// Enable for games without native controller support
    #[serde(default, skip_serializing_if = "GptokeybSettings::is_default")]
    pub gptokeyb: GptokeybSettings,

    /// Game config file patches - modify game config files with key-value replacements
    /// Outer key: file path relative to game root (e.g., "conf/settings.cfg")
    /// Inner key-value: config key to find and new value to set
    /// Supports set-style (set key "value"), ini-style (key=value), space-style (key value)
    /// Falls back to line search/replace if format not detected
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub game_patches: HashMap<String, HashMap<String, String>>,

    // Save game integration
    /// Path to original save data location. Supports:
    /// - ~ or $HOME for home directory
    /// - For Windows games: relative paths like "AppData/LocalLow/Company/Game" are relative to windata
    /// - Absolute paths are used as-is
    /// When set, original saves are copied to each profile before launch.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub original_save_path: String,
    /// Sync saves back to original location after game session ends.
    /// Uses the first named (non-guest) profile's saves.
    /// Original saves are always backed up before overwriting.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub save_sync_back: bool,
    /// Remap Steam IDs in save file names when copying to/from profiles.
    /// Some games (like Deep Rock Galactic) tie saves to Steam IDs by embedding
    /// the ID in the filename (e.g., "76561198035859048_Player.sav").
    /// When enabled, save files are renamed to use each profile's Goldberg Steam ID.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub save_steam_id_remap: bool,
}

fn is_default_spec_ver(v: &u16) -> bool {
    *v == HANDLER_SPEC_CURRENT_VERSION || *v == 0
}

// Import YAML parsing functions from pure module
use pure::yaml_parser::expand_dot_notation;

impl Default for Handler {
    fn default() -> Self {
        Self {
            path_handler: PathBuf::new(),
            img_paths: Vec::new(),
            path_gameroot: String::new(),

            name: String::new(),
            author: String::new(),
            version: String::new(),
            info: String::new(),
            spec_ver: HANDLER_SPEC_CURRENT_VERSION,

            runtime: String::new(),
            exec: String::new(),
            args: String::new(),
            env: String::new(),
            sdl2_override: SDL2Override::No,
            proton_path: String::new(),

            backend: MultiplayerBackend::None,
            use_goldberg: false,
            steam_appid: None,
            platform: None,
            goldberg_settings: std::collections::HashMap::new(),
            goldberg_disable_networking: false,
            goldberg_networking_sockets: false,
            photon_settings: PhotonSettings::default(),
            facepunch_settings: FacepunchSettings::default(),
            runtime_patches: Vec::new(),

            // New optional backend fields (Phase 7)
            goldberg: None,
            photon: None,
            facepunch: None,
            eos: None,

            required_mods: Vec::new(),

            game_null_paths: Vec::new(),
            disable_bwrap: false,
            disable_input_isolation: false,
            gptokeyb: GptokeybSettings::default(),
            game_patches: HashMap::new(),

            original_save_path: String::new(),
            save_sync_back: false,
            save_steam_id_remap: false,
        }
    }
}

impl Handler {
    pub fn from_yaml(yaml_path: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let file = File::open(yaml_path)?;

        // Phase 1: Read raw YAML to support dot notation
        let raw: serde_yaml::Value = serde_yaml::from_reader(BufReader::new(file))?;

        // Phase 2: Expand dot notation keys (e.g., "goldberg.disable_networking" -> nested)
        let expanded = expand_dot_notation(raw);

        // Phase 3: Deserialize the expanded structure
        let mut handler: Handler = serde_yaml::from_value(expanded)?;

        handler.path_handler = yaml_path
            .parent()
            .ok_or_else(|| "Invalid path")?
            .to_path_buf();
        handler.img_paths = handler.get_imgs();

        // Clean up whitespace from all fields
        handler.trim_fields();

        // Sanitize paths
        for path in &mut handler.game_null_paths {
            *path = path.sanitize_path();
        }

        // Phase 4: Migrate old format to new optional backend fields
        handler.migrate_legacy_backends();

        // Validate required fields
        handler.validate()?;

        Ok(handler)
    }

    /// Migrate legacy backend fields to new optional backend format
    fn migrate_legacy_backends(&mut self) {
        // Migrate Goldberg: old enum + flat fields -> new Optional<GoldbergSettings>
        if self.goldberg.is_none() {
            let should_enable = self.backend == MultiplayerBackend::Goldberg
                || self.use_goldberg
                || !self.goldberg_settings.is_empty()
                || self.goldberg_disable_networking
                || self.goldberg_networking_sockets;

            if should_enable {
                self.goldberg = Some(BackendGoldbergSettings {
                    disable_networking: self.goldberg_disable_networking,
                    networking_sockets: self.goldberg_networking_sockets,
                    settings: self.goldberg_settings.clone(),
                    plugin: None, // Legacy handlers don't have plugin support
                });
            }
        }

        // Migrate Photon: old enum + struct -> new Optional<PhotonSettings>
        if self.photon.is_none() {
            let should_enable = self.backend == MultiplayerBackend::Photon
                || !self.photon_settings.is_empty();

            if should_enable {
                self.photon = Some(BackendPhotonSettings {
                    config_path: self.photon_settings.config_path.clone(),
                    shared_files: self.photon_settings.shared_files.clone(),
                    plugin: None, // Legacy settings don't have plugin
                });
            }
        }

        // Migrate Facepunch: presence-based -> new Optional<FacepunchSettings>
        if self.facepunch.is_none() {
            let should_enable = !self.facepunch_settings.is_default()
                || !self.runtime_patches.is_empty();

            if should_enable {
                self.facepunch = Some(BackendFacepunchSettings {
                    spoof_identity: self.facepunch_settings.spoof_identity,
                    force_valid: self.facepunch_settings.force_valid,
                    photon_bypass: self.facepunch_settings.photon_bypass,
                });
            }
        }

        // Clear deprecated fields after migration
        self.use_goldberg = false;
    }

    /// Trim whitespace from all string fields
    fn trim_fields(&mut self) {
        self.name = self.name.trim().to_string();
        self.exec = self.exec.trim().to_string();
        self.author = self.author.trim().to_string();
        self.version = self.version.trim().to_string();
        self.info = self.info.trim().to_string();
        self.path_gameroot = self.path_gameroot.trim().to_string();
        self.runtime = self.runtime.trim().to_string();
        self.args = self.args.trim().to_string();
        self.env = self.env.trim().to_string();
        self.proton_path = self.proton_path.trim().to_string();
        self.original_save_path = self.original_save_path.trim().to_string();

        // Trim paths in null_paths list
        for path in &mut self.game_null_paths {
            *path = path.trim().to_string();
        }
        // Remove empty entries
        self.game_null_paths.retain(|p| !p.is_empty());
    }

    /// Validate that required fields are present
    fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.name.is_empty() {
            return Err("Handler 'name' is required".into());
        }
        if self.exec.is_empty() {
            return Err("Handler 'exec' (executable path) is required".into());
        }
        Ok(())
    }

    pub fn from_cli(path_exec: &str, args: &str) -> Self {
        let mut handler = Self::default();

        handler.path_gameroot = Path::new(path_exec)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap();
        handler.exec = Path::new(path_exec)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap();
        handler.args = args.to_string();

        handler
    }

    // Steam asset methods moved to operations/steam_assets.rs

    pub fn display(&self) -> &str {
        self.name.as_str()
    }

    pub fn display_clamp(&self) -> String {
        if self.name.len() > 25 {
            let out = format!("{}...", &self.name[..22]);
            out.clone()
        } else {
            self.name.clone()
        }
    }

    pub fn win(&self) -> bool {
        self.exec.ends_with(".exe") || self.exec.ends_with(".bat")
    }

    pub fn is_saved_handler(&self) -> bool {
        !self.path_handler.as_os_str().is_empty()
    }

    pub fn handler_dir_name(&self) -> &str {
        self.path_handler
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    }

    // Persistence methods moved to operations/persistence.rs

    // ============= NEW BACKEND HELPER METHODS (Phase 7) =============

    /// Check if Goldberg backend is enabled
    pub fn has_goldberg(&self) -> bool {
        self.goldberg.is_some()
    }

    /// Check if Goldberg backend has a plugin configured (needs BepInEx)
    pub fn has_goldberg_plugin(&self) -> bool {
        self.goldberg
            .as_ref()
            .and_then(|g| g.plugin.as_ref())
            .map(|p| !p.is_empty())
            .unwrap_or(false)
    }

    /// Check if Photon backend is enabled
    pub fn has_photon(&self) -> bool {
        self.photon.is_some()
    }

    /// Check if Facepunch backend is enabled
    pub fn has_facepunch(&self) -> bool {
        self.facepunch.is_some()
    }

    /// Check if EOS backend is enabled
    pub fn has_eos(&self) -> bool {
        self.eos.is_some()
    }

    /// Get display string for enabled backends (e.g., "Goldberg", "Photon, Facepunch")
    pub fn backend_display(&self) -> String {
        let mut backends = Vec::new();
        if self.has_goldberg() {
            backends.push("Goldberg");
        }
        if self.has_eos() {
            backends.push("EOS");
        }
        if self.has_photon() {
            backends.push("Photon");
        }
        if self.has_facepunch() {
            backends.push("Facepunch");
        }
        if backends.is_empty() {
            "None".to_string()
        } else {
            backends.join(", ")
        }
    }

    /// Get Goldberg settings reference (if enabled)
    pub fn goldberg_ref(&self) -> Option<&BackendGoldbergSettings> {
        self.goldberg.as_ref()
    }

    /// Get Photon settings reference (if enabled)
    pub fn photon_ref(&self) -> Option<&BackendPhotonSettings> {
        self.photon.as_ref()
    }

    /// Get Facepunch settings reference (if enabled)
    pub fn facepunch_ref(&self) -> Option<&BackendFacepunchSettings> {
        self.facepunch.as_ref()
    }

    /// Get EOS settings reference (if enabled)
    pub fn eos_ref(&self) -> Option<&BackendEosSettings> {
        self.eos.as_ref()
    }

    /// Enable Goldberg backend with default settings
    pub fn enable_goldberg(&mut self) {
        if self.goldberg.is_none() {
            self.goldberg = Some(BackendGoldbergSettings::default());
        }
    }

    /// Enable Photon backend with default settings
    pub fn enable_photon(&mut self) {
        if self.photon.is_none() {
            self.photon = Some(BackendPhotonSettings::default());
        }
    }

    /// Enable Facepunch backend with default settings
    pub fn enable_facepunch(&mut self) {
        if self.facepunch.is_none() {
            self.facepunch = Some(BackendFacepunchSettings::default());
        }
    }

    /// Disable Goldberg backend
    pub fn disable_goldberg(&mut self) {
        self.goldberg = None;
    }

    /// Disable Photon backend
    pub fn disable_photon(&mut self) {
        self.photon = None;
    }

    /// Disable Facepunch backend
    pub fn disable_facepunch(&mut self) {
        self.facepunch = None;
    }

    /// Enable EOS backend with default settings
    pub fn enable_eos(&mut self) {
        if self.eos.is_none() {
            self.eos = Some(BackendEosSettings::default());
        }
    }

    /// Disable EOS backend
    pub fn disable_eos(&mut self) {
        self.eos = None;
    }

    // ============= PLATFORM HELPER METHODS (Phase 10) =============

    /// Get Platform trait object from config or legacy fields
    ///
    /// Resolution order:
    /// 1. platform field (if set)
    /// 2. steam_appid (creates SteamPlatform)
    /// 3. path_gameroot (creates ManualPlatform)
    /// 4. Empty ManualPlatform as fallback
    pub fn get_platform(&self) -> Box<dyn crate::platform::Platform> {
        use crate::platform::{ManualPlatform, SteamPlatform};

        if let Some(ref config) = self.platform {
            config.as_platform()
        } else if let Some(appid) = self.steam_appid {
            Box::new(SteamPlatform::new(appid))
        } else if !self.path_gameroot.is_empty() {
            Box::new(ManualPlatform::new(self.path_gameroot.clone()))
        } else {
            Box::new(ManualPlatform::new(String::new()))
        }
    }

    /// Get Steam app ID from platform config or legacy field
    pub fn get_steam_appid(&self) -> Option<u32> {
        self.platform
            .as_ref()
            .and_then(|p| p.steam_appid())
            .or(self.steam_appid)
    }

    /// Set Steam platform with given app ID
    pub fn set_platform_steam(&mut self, appid: u32) {
        self.platform = Some(crate::platform::PlatformConfig::Steam { steam_appid: appid });
    }

    /// Set Manual platform with given path
    pub fn set_platform_manual(&mut self, path: String) {
        self.platform = Some(crate::platform::PlatformConfig::Manual { path_gameroot: path });
    }

    /// Clear platform config
    pub fn clear_platform(&mut self) {
        self.platform = None;
    }

    /// Get platform name (e.g., "steam", "manual")
    pub fn platform_name(&self) -> String {
        self.get_platform().name().to_string()
    }

    /// Get platform-specific app identifier (e.g., Steam appid as string)
    pub fn platform_app_id(&self) -> Option<String> {
        self.get_platform().app_identifier()
    }

    // ============= GPTOKEYB HELPER METHODS =============

    /// Check if gptokeyb is enabled for this handler
    pub fn has_gptokeyb(&self) -> bool {
        self.gptokeyb.is_enabled()
    }

    /// Get gptokeyb settings reference (for UI)
    #[allow(dead_code)]
    pub fn gptokeyb_ref(&self) -> &GptokeybSettings {
        &self.gptokeyb
    }
}

// Tests moved to handler/tests.rs
