mod io;
mod types;

// Re-export types from submodule
pub use types::{FacepunchSettings, PhotonSettings, RequiredMod, RuntimePatch, SDL2Override, is_default_sdl2};
// Re-export I/O functions from submodule
pub use io::{import_handler, scan_handlers};

use crate::backend::{
    FacepunchSettings as BackendFacepunchSettings, GoldbergSettings as BackendGoldbergSettings,
    MultiplayerBackend, PhotonSettings as BackendPhotonSettings,
};
use crate::paths::*;
use crate::util::*;

use eframe::egui::{self, ImageSource};
use rfd::FileDialog;
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

    /// Path to bind as $HOME/.steam inside the container.
    /// Steam API looks for ~/.steam/root/steam.sh and related files.
    /// Default: "~/.steam" (which has symlinks to .local/share/Steam)
    /// Set to empty string to disable Steam home binding.
    #[serde(default = "default_steam_home_path", skip_serializing_if = "String::is_empty")]
    pub steam_home_path: String,

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_between_starts: Option<f64>,
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

    /// Required mods/files that must be installed by the user
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_mods: Vec<RequiredMod>,

    // Advanced
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub game_null_paths: Vec<String>,
    /// Disable bwrap container (may be needed for games with networking issues)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_bwrap: bool,

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

/// Expand dot-notation keys into nested YAML structure
/// e.g., "goldberg.disable_networking: true" becomes:
/// goldberg:
///   disable_networking: true
///
/// Special handling for settings keys:
/// - "goldberg.settings.force_lobby_type.txt" becomes goldberg.settings["force_lobby_type.txt"]
/// - Only expands keys that start with known backend prefixes (goldberg., photon., facepunch.)
/// - Does NOT recursively expand nested values to avoid mangling game_patches, etc.
fn expand_dot_notation(value: serde_yaml::Value) -> serde_yaml::Value {
    use serde_yaml::Value;

    let Value::Mapping(map) = value else {
        return value;
    };

    let mut result = serde_yaml::Mapping::new();
    let known_backends = ["goldberg.", "photon.", "facepunch."];

    for (key, val) in map {
        let Value::String(key_str) = &key else {
            // Non-string key, keep as-is (no recursive expansion)
            result.insert(key, val);
            continue;
        };

        // Only expand keys that start with known backend prefixes
        let should_expand = known_backends.iter().any(|prefix| key_str.starts_with(prefix));

        if should_expand {
            // Smart split for dot notation
            // Handle "backend.settings.filename.ext" specially - settings values are filenames
            let parts = smart_split_dot_notation(key_str);
            insert_nested(&mut result, &parts, val);
        } else {
            // Not a backend key, keep as-is (no recursive expansion)
            result.insert(key, val);
        }
    }

    Value::Mapping(result)
}

/// Smart split for dot notation that preserves filenames in settings
/// - "goldberg.disable_networking" -> ["goldberg", "disable_networking"]
/// - "goldberg.settings.force_lobby_type.txt" -> ["goldberg", "settings", "force_lobby_type.txt"]
/// - "photon.config_path" -> ["photon", "config_path"]
fn smart_split_dot_notation(key: &str) -> Vec<&str> {
    // Check for the pattern: backend.settings.* where everything after settings. is the key
    let known_backends = ["goldberg", "photon", "facepunch"];

    for backend in known_backends {
        let settings_prefix = format!("{}.", backend);
        if key.starts_with(&settings_prefix) {
            let rest = &key[settings_prefix.len()..];

            // Check if this is a settings.* key
            if rest.starts_with("settings.") {
                let settings_key = &rest["settings.".len()..];
                // Return [backend, "settings", "everything.else.as.one.key"]
                return vec![backend, "settings", settings_key];
            } else {
                // Normal two-level split: backend.field
                if let Some(dot_pos) = rest.find('.') {
                    // More dots, but not under settings - do normal split
                    // This handles cases like "goldberg.something.else" -> split all
                    return key.split('.').collect();
                } else {
                    return vec![backend, rest];
                }
            }
        }
    }

    // Not a known backend prefix, do normal split
    key.split('.').collect()
}

/// Insert a value at a nested path in the mapping
/// e.g., insert_nested(map, ["goldberg", "settings", "force_lobby_type"], "2")
/// creates: goldberg: { settings: { force_lobby_type: "2" } }
fn insert_nested(map: &mut serde_yaml::Mapping, parts: &[&str], value: serde_yaml::Value) {
    use serde_yaml::Value;

    if parts.is_empty() {
        return;
    }

    let key = Value::String(parts[0].to_string());

    if parts.len() == 1 {
        // Base case: insert the value at this key
        map.insert(key, value);
    } else {
        // Recursive case: get or create nested mapping
        let nested = map
            .entry(key.clone())
            .or_insert_with(|| Value::Mapping(serde_yaml::Mapping::new()));

        if let Value::Mapping(nested_map) = nested {
            insert_nested(nested_map, &parts[1..], value);
        } else {
            // Key exists but isn't a mapping - replace with mapping
            let mut new_map = serde_yaml::Mapping::new();
            insert_nested(&mut new_map, &parts[1..], value);
            *nested = Value::Mapping(new_map);
        }
    }
}

fn default_steam_home_path() -> String {
    "~/.steam".to_string()
}

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

            pause_between_starts: None,

            backend: MultiplayerBackend::None,
            use_goldberg: false,
            steam_appid: None,
            steam_home_path: default_steam_home_path(),
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

            required_mods: Vec::new(),

            game_null_paths: Vec::new(),
            disable_bwrap: false,
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

    /// Find a file in Steam's librarycache for this app
    /// Steam stores files in: {STEAM}/appcache/librarycache/{appid}/{hash}/{filename}
    /// or directly as: {STEAM}/appcache/librarycache/{appid}/{filename}
    fn find_steam_cache_file(&self, filename: &str) -> Option<std::path::PathBuf> {
        use crate::paths::PATH_STEAM;

        let appid = self.steam_appid?;
        let app_cache = PATH_STEAM.join("appcache/librarycache").join(appid.to_string());

        if !app_cache.exists() {
            return None;
        }

        // Check directly in app folder
        let direct_path = app_cache.join(filename);
        if direct_path.exists() {
            return Some(direct_path);
        }

        // Search in hash subfolders
        if let Ok(entries) = std::fs::read_dir(&app_cache) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let file_path = entry.path().join(filename);
                    if file_path.exists() {
                        return Some(file_path);
                    }
                }
            }
        }

        None
    }

    /// Find the small icon (32x32 jpg) directly in the librarycache app folder
    /// These are stored as {hash}.jpg directly in the app folder (not in subfolders)
    /// The icon files have hash names (not library_*, header*, logo*, etc.)
    fn find_steam_icon(&self) -> Option<std::path::PathBuf> {
        use crate::paths::PATH_STEAM;

        let appid = self.steam_appid?;
        let app_cache = PATH_STEAM.join("appcache/librarycache").join(appid.to_string());

        if !app_cache.exists() {
            return None;
        }

        // Look for image files directly in the app folder (not in subfolders)
        // Icon files have hash names like "b3a992fd5991bd2f4c956d58e062b0ce2988d6cd.jpg"
        // Skip files named library_*, header*, logo* as those are other artwork
        if let Ok(entries) = std::fs::read_dir(&app_cache) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                        // Skip known non-icon files
                        if filename.starts_with("library_")
                            || filename.starts_with("header")
                            || filename.starts_with("logo") {
                            continue;
                        }

                        if let Some(ext) = path.extension() {
                            if ext == "jpg" || ext == "png" || ext == "ico" {
                                return Some(path);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub fn icon(&self) -> ImageSource<'_> {
        // Check for local icon first (supports .png, .jpg, .ico)
        let local_icon_png = self.path_handler.join("icon.png");
        let local_icon_jpg = self.path_handler.join("icon.jpg");
        let local_icon_ico = self.path_handler.join("icon.ico");
        if local_icon_png.exists() {
            return format!("file://{}", local_icon_png.display()).into();
        }
        if local_icon_jpg.exists() {
            return format!("file://{}", local_icon_jpg.display()).into();
        }
        if local_icon_ico.exists() {
            return format!("file://{}", local_icon_ico.display()).into();
        }

        // Try Steam's local cache - small icon (32x32) in the app folder root
        if let Some(cached) = self.find_steam_icon() {
            return format!("file://{}", cached.display()).into();
        }

        // Fallback to default icon
        egui::include_image!("../res/executable_icon.png")
    }

    /// Returns the box art (library_600x900.jpg) for display when no logo available
    pub fn box_art(&self) -> Option<String> {
        self.find_steam_cache_file("library_600x900.jpg")
            .map(|p| format!("file://{}", p.display()))
    }

    /// Returns the game logo from Steam cache if available
    pub fn logo_image(&self) -> Option<String> {
        self.find_steam_cache_file("logo.png")
            .map(|p| format!("file://{}", p.display()))
    }

    /// Returns the Steam library hero image (1920x620 banner) from local cache
    pub fn hero_image(&self) -> Option<String> {
        self.find_steam_cache_file("library_hero.jpg")
            .map(|p| format!("file://{}", p.display()))
    }

    /// Returns the Steam header image (smaller banner) from local cache
    #[allow(dead_code)]
    pub fn header_image(&self) -> Option<String> {
        // Check for locally cached header in handler directory first
        let local_header = self.path_handler.join("imgs").join("steam_header.jpg");
        if local_header.exists() {
            return Some(format!("file://{}", local_header.display()));
        }

        // Then check Steam's local cache
        self.find_steam_cache_file("library_header.jpg")
            .map(|p| format!("file://{}", p.display()))
    }

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

    fn get_imgs(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let imgs_path = self.path_handler.join("imgs");

        let entries = match std::fs::read_dir(imgs_path) {
            Ok(entries) => entries,
            Err(_) => return out,
        };

        for entry_result in entries {
            if let Ok(entry) = entry_result
                && let Ok(file_type) = entry.file_type()
                && file_type.is_file()
                && let Some(path_str) = entry.path().to_str()
                && (path_str.ends_with(".png") || path_str.ends_with(".jpg"))
            {
                out.push(entry.path());
            }
        }

        out.sort();
        out
    }

    pub fn remove_handler(&self) -> Result<(), Box<dyn Error>> {
        if !self.is_saved_handler() {
            return Err("No handler directory to remove".into());
        }
        // TODO: Also return err if handler path exists but is not inside PATH_PARTY/handlers
        std::fs::remove_dir_all(self.path_handler.clone())?;

        Ok(())
    }

    pub fn get_game_rootpath(&self) -> Result<String, Box<dyn Error>> {
        if let Some(appid) = &self.steam_appid
            && let Some((app, library)) = steamlocate::SteamDir::locate()?
                .find_app(*appid)
                .ok()
                .flatten()
        {
            {
                let path = library.resolve_app_dir(&app);
                if path.exists() {
                    let pathstr = path.to_string_lossy().to_string();
                    return Ok(pathstr);
                }
            }
        }

        if !self.path_gameroot.is_empty() && Path::new(&self.path_gameroot).exists() {
            return Ok(self.path_gameroot.clone());
        }

        Err("Game root path not found".into())
    }

    pub fn save(&mut self) -> Result<(), Box<dyn Error>> {
        // If handler has no path, assume we're saving a newly created handler
        if !self.is_saved_handler() {
            if self.name.is_empty() {
                // If handler is based on a Steam game try to get the game's install dir name
                if let Some(appid) = self.steam_appid
                    && let Ok(dir) = steamlocate::SteamDir::locate()
                    && let Ok(Some((app, _))) = dir.find_app(appid)
                {
                    self.name = app.install_dir;
                } else {
                    return Err("Name cannot be empty".into());
                }
            }
            if !PATH_PARTY.join("handlers").join(&self.name).exists() {
                self.path_handler = PATH_PARTY.join("handlers").join(&self.name);
            } else {
                let mut i = 1;
                while PATH_PARTY
                    .join("handlers")
                    .join(&format!("{}-{}", self.name, i))
                    .exists()
                {
                    i += 1;
                }
                self.path_handler = PATH_PARTY
                    .join("handlers")
                    .join(&format!("{}-{}", self.name, i));
            }
        }

        if !self.path_handler.exists() {
            std::fs::create_dir_all(&self.path_handler)?;
        }

        let yaml = serde_yaml::to_string(self)?;
        std::fs::write(self.path_handler.join("handler.yaml"), yaml)?;

        Ok(())
    }

    pub fn export(&self) -> Result<(), Box<dyn Error>> {
        if self.name.is_empty() {
            return Err("Name cannot be empty".into());
        }

        let mut file = FileDialog::new()
            .set_title("Save file to:")
            .set_directory(&*PATH_HOME)
            .add_filter("Splitux Handler Package", &["spx"])
            .save_file()
            .ok_or_else(|| "File not specified")?;

        if file.extension().is_none() || file.extension() != Some("spx".as_ref()) {
            file.set_extension("spx");
        }

        let tmpdir = PATH_PARTY.join("tmp");
        std::fs::create_dir_all(&tmpdir)?;

        copy_dir_recursive(&self.path_handler, &tmpdir)?;

        // Clear the rootpath before exporting so that users downloading it can set their own
        let mut handlerclone = self.clone();
        handlerclone.path_gameroot = String::new();
        // Overwrite the handler.yaml file with handlerclone
        let yaml = serde_yaml::to_string(&handlerclone)?;
        std::fs::write(tmpdir.join("handler.yaml"), yaml)?;

        if file.is_file() {
            std::fs::remove_file(&file)?;
        }

        zip_dir(&tmpdir, &file)?;
        clear_tmp()?;

        Ok(())
    }

    // ============= NEW BACKEND HELPER METHODS (Phase 7) =============

    /// Check if Goldberg backend is enabled
    pub fn has_goldberg(&self) -> bool {
        self.goldberg.is_some()
    }

    /// Check if Photon backend is enabled
    pub fn has_photon(&self) -> bool {
        self.photon.is_some()
    }

    /// Check if Facepunch backend is enabled
    pub fn has_facepunch(&self) -> bool {
        self.facepunch.is_some()
    }

    /// Check if any backend is enabled
    pub fn has_any_backend(&self) -> bool {
        self.has_goldberg() || self.has_photon() || self.has_facepunch()
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_notation_expansion() {
        let yaml = r#"
name: Test
goldberg.disable_networking: false
goldberg.settings.force_lobby_type.txt: "2"
goldberg.settings.invite_all.txt: ""
"#;
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        println!("Raw YAML: {:#?}", raw);

        let expanded = expand_dot_notation(raw);
        println!("Expanded YAML: {:#?}", expanded);

        // Check the expanded structure
        let map = expanded.as_mapping().unwrap();
        let goldberg = map.get(&serde_yaml::Value::String("goldberg".to_string()))
            .expect("goldberg key should exist");
        let goldberg_map = goldberg.as_mapping()
            .expect("goldberg should be a mapping");

        // Print goldberg structure
        println!("Goldberg map: {:#?}", goldberg_map);

        // Check disable_networking
        let disable_net = goldberg_map.get(&serde_yaml::Value::String("disable_networking".to_string()))
            .expect("disable_networking should exist");
        assert_eq!(disable_net.as_bool(), Some(false));

        // Check settings - now we need to look at what actually exists
        for (k, v) in goldberg_map.iter() {
            println!("Key: {:?}, Value: {:?}", k, v);
        }
    }

    #[test]
    fn test_handler_load_with_dot_notation() {
        let yaml = r#"
name: TestHandler
exec: test.exe
spec_ver: 3
goldberg.disable_networking: false
goldberg.settings.force_lobby_type.txt: "2"
"#;
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let expanded = expand_dot_notation(raw);
        let handler: Handler = serde_yaml::from_value(expanded).unwrap();

        assert!(handler.goldberg.is_some(), "goldberg should be Some");
        let goldberg = handler.goldberg.unwrap();
        assert_eq!(goldberg.disable_networking, false);
        assert_eq!(goldberg.settings.get("force_lobby_type.txt"), Some(&"2".to_string()));

        println!("Handler loaded: {:?}", handler.name);
    }

    #[test]
    fn test_photon_handler_dot_notation() {
        let yaml = r#"
name: TestPhoton
exec: test.exe
spec_ver: 3
photon.config_path: "AppData/LocalLow/Test/Game/config.cfg"
photon.shared_files:
  - "AppData/LocalLow/Test/Game/shared"
"#;
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let expanded = expand_dot_notation(raw);
        let handler: Handler = serde_yaml::from_value(expanded).unwrap();

        assert!(handler.photon.is_some(), "photon should be Some");
        let photon = handler.photon.unwrap();
        assert_eq!(photon.config_path, "AppData/LocalLow/Test/Game/config.cfg");
        assert_eq!(photon.shared_files.len(), 1);

        println!("Photon handler loaded: {:?}", handler.name);
    }

    #[test]
    fn test_facepunch_handler_dot_notation() {
        let yaml = r#"
name: TestFacepunch
exec: test.x86_64
spec_ver: 3
facepunch.spoof_identity: true
facepunch.force_valid: true
facepunch.photon_bypass: true
"#;
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let expanded = expand_dot_notation(raw);
        let handler: Handler = serde_yaml::from_value(expanded).unwrap();

        assert!(handler.facepunch.is_some(), "facepunch should be Some");
        let facepunch = handler.facepunch.unwrap();
        assert!(facepunch.spoof_identity);
        assert!(facepunch.force_valid);
        assert!(facepunch.photon_bypass);

        println!("Facepunch handler loaded: {:?}", handler.name);
    }

    #[test]
    fn test_load_all_installed_handlers() {
        use crate::handler::scan_handlers;

        let handlers = scan_handlers();
        println!("\n=== Loaded {} handlers ===", handlers.len());

        for h in &handlers {
            println!("\n--- {} ---", h.name);
            if let Some(ref goldberg) = h.goldberg {
                println!("  goldberg.disable_networking: {}", goldberg.disable_networking);
                println!("  goldberg.settings: {:?}", goldberg.settings);
            }
            if let Some(ref photon) = h.photon {
                println!("  photon.config_path: {}", photon.config_path);
                println!("  photon.shared_files: {:?}", photon.shared_files);
            }
            if let Some(ref facepunch) = h.facepunch {
                println!("  facepunch.spoof_identity: {}", facepunch.spoof_identity);
                println!("  facepunch.force_valid: {}", facepunch.force_valid);
                println!("  facepunch.photon_bypass: {}", facepunch.photon_bypass);
            }
            if h.goldberg.is_none() && h.photon.is_none() && h.facepunch.is_none() {
                println!("  (no backend configured)");
            }
        }

        // All handlers should load without errors
        assert!(!handlers.is_empty(), "Should have at least one handler");
    }

    #[test]
    fn test_riftbreaker_handler() {
        // Test TheRiftbreaker handler specifically - it only has goldberg.settings.* without goldberg.disable_networking
        let yaml = r#"
name: The Riftbreaker
spec_ver: 3
steam_appid: 780310
exec: bin/riftbreaker_win_release.exe
goldberg.settings.force_lobby_type.txt: "2"
goldberg.settings.invite_all.txt: ""
"#;
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        println!("Raw YAML: {:#?}", raw);

        let expanded = expand_dot_notation(raw);
        println!("Expanded YAML: {:#?}", expanded);

        let handler: Handler = serde_yaml::from_value(expanded).unwrap();

        assert!(handler.goldberg.is_some(), "goldberg should be Some even with only settings");
        let goldberg = handler.goldberg.unwrap();
        println!("goldberg: {:?}", goldberg);
        assert_eq!(goldberg.settings.get("force_lobby_type.txt"), Some(&"2".to_string()));
    }

    #[test]
    fn test_load_actual_riftbreaker_file() {
        use std::path::PathBuf;
        use std::io::BufReader;
        use std::fs::File;

        let yaml_path = PathBuf::from(std::env::var("HOME").unwrap())
            .join(".local/share/splitux/handlers/TheRiftbreaker/handler.yaml");

        if yaml_path.exists() {
            // Debug: read and print the raw and expanded YAML
            let file = File::open(&yaml_path).unwrap();
            let raw: serde_yaml::Value = serde_yaml::from_reader(BufReader::new(file)).unwrap();
            println!("Raw YAML from file: {:#?}", raw);

            let expanded = expand_dot_notation(raw);
            println!("Expanded YAML: {:#?}", expanded);

            match serde_yaml::from_value::<Handler>(expanded) {
                Ok(handler) => {
                    println!("Loaded TheRiftbreaker: {}", handler.name);
                    println!("  goldberg: {:?}", handler.goldberg);
                    assert!(handler.goldberg.is_some());
                }
                Err(e) => {
                    panic!("Failed to deserialize TheRiftbreaker handler: {}", e);
                }
            }
        } else {
            println!("TheRiftbreaker handler not found at {:?}, skipping test", yaml_path);
        }
    }
}
