//! Handler-related type definitions
//!
//! Types used by handlers for configuration and mod management.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A required mod/file that must be installed by the user
#[derive(Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RequiredMod {
    /// Display name of the mod
    pub name: String,
    /// Description of what the mod does
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// URL where the mod can be downloaded
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// Destination path relative to handler directory (e.g., "overlay/BepInEx/plugins")
    pub dest_path: String,
    /// Expected filename or pattern (e.g., "LocalMultiplayer.dll" or "*.dll")
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub file_pattern: String,
}

impl RequiredMod {
    /// Check if the mod is installed at the expected location
    pub fn is_installed(&self, handler_path: &Path) -> bool {
        let dest = handler_path.join(&self.dest_path);
        if !dest.exists() {
            return false;
        }

        // If no pattern specified, just check if dest directory exists
        if self.file_pattern.is_empty() {
            return true;
        }

        // Check if any file matching the pattern exists
        if let Ok(entries) = std::fs::read_dir(&dest) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if self.matches_pattern(&name) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a filename matches the pattern (supports * wildcard)
    fn matches_pattern(&self, filename: &str) -> bool {
        let pattern = &self.file_pattern;
        if pattern.starts_with('*') {
            // *.dll -> check if ends with .dll
            let suffix = &pattern[1..];
            filename.ends_with(suffix)
        } else if pattern.ends_with('*') {
            // prefix* -> check if starts with prefix
            let prefix = &pattern[..pattern.len() - 1];
            filename.starts_with(prefix)
        } else {
            // Exact match or contains
            filename == pattern || filename.contains(pattern)
        }
    }

    /// Get the full destination path
    pub fn dest_full_path(&self, handler_path: &Path) -> PathBuf {
        handler_path.join(&self.dest_path)
    }
}

/// Photon-specific settings for BepInEx/LocalMultiplayer
#[derive(Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PhotonSettings {
    /// Path pattern for LocalMultiplayer config file within profile's windata
    /// Example: "AppData/LocalLow/CompanyName/GameName/LocalMultiplayer/global.cfg"
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub config_path: String,

    /// Files that should be shared between all instances (relative to windata)
    /// These files will be symlinked to a shared location so instances can communicate
    /// Example: "AppData/LocalLow/CompanyName/GameName/LocalMultiplayer/GlobalSave"
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared_files: Vec<String>,
}

impl PhotonSettings {
    pub fn is_empty(&self) -> bool {
        self.config_path.is_empty() && self.shared_files.is_empty()
    }
}

/// Facepunch.Steamworks patch settings for SplituxFacepunch BepInEx plugin
/// Presence of this section enables the Facepunch patches.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct FacepunchSettings {
    /// Spoof SteamClient.SteamId and SteamClient.Name to unique per-instance values
    #[serde(default)]
    pub spoof_identity: bool,

    /// Force SteamClient.IsValid and IsLoggedOn to return true
    #[serde(default)]
    pub force_valid: bool,

    /// Bypass Photon Steam authentication (AuthType=255)
    #[serde(default)]
    pub photon_bypass: bool,
}

impl FacepunchSettings {
    pub fn is_default(&self) -> bool {
        !self.spoof_identity && !self.force_valid && !self.photon_bypass
    }
}

/// A runtime patch specification for game-specific classes
/// Used by SplituxFacepunch to apply Harmony patches at runtime
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimePatch {
    /// Target class name (e.g., "SteamManager", "GameManager")
    pub class: String,

    /// Method name to patch (mutually exclusive with property)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub method: String,

    /// Property name to patch (mutually exclusive with method)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub property: String,

    /// Action to apply from PatchActions library
    /// Available: force_true, force_false, skip, force_steam_loaded, fake_auth_ticket, photon_auth_none, log_call
    pub action: String,
}

/// SDL2 library override options
#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SDL2Override {
    #[default]
    No,
    /// Use Steam Runtime's SDL2
    Srt,
    /// Use system SDL2
    Sys,
}

/// Check if SDL2Override is the default value (for serde skip_serializing_if)
pub fn is_default_sdl2(v: &SDL2Override) -> bool {
    *v == SDL2Override::No
}
