// Profile preferences module
// Stores per-profile settings like preferred controller and audio device

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::paths::PATH_PARTY;

/// Preferences stored per profile for automatic device assignment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfilePreferences {
    /// Preferred controller identifier (uniq field - Bluetooth MAC or USB serial)
    /// Used to auto-assign the controller when this profile is selected
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_controller: Option<String>,

    /// Human-readable name of the preferred controller (for display purposes)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_controller_name: Option<String>,

    /// Preferred audio sink name (e.g., "alsa_output.usb-headphones")
    /// Used to route audio to the correct output when this profile is selected
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_audio: Option<String>,

    /// Human-readable name of the preferred audio device (for display purposes)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_audio_name: Option<String>,
}

impl ProfilePreferences {
    /// Get the path to the preferences file for a profile
    pub fn path(profile_name: &str) -> PathBuf {
        PATH_PARTY
            .join("profiles")
            .join(profile_name)
            .join("preferences.json")
    }

    /// Load preferences for a profile, returns default if file doesn't exist or is invalid
    pub fn load(profile_name: &str) -> Self {
        let path = Self::path(profile_name);
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save preferences for a profile
    pub fn save(&self, profile_name: &str) -> std::io::Result<()> {
        let path = Self::path(profile_name);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }

    /// Check if this profile has a preferred controller set
    pub fn has_controller(&self) -> bool {
        self.preferred_controller.is_some()
    }

    /// Check if this profile has a preferred audio device set
    pub fn has_audio(&self) -> bool {
        self.preferred_audio.is_some()
    }

    /// Set the preferred controller
    pub fn set_controller(&mut self, uniq: &str, name: &str) {
        self.preferred_controller = Some(uniq.to_string());
        self.preferred_controller_name = Some(name.to_string());
    }

    /// Clear the preferred controller
    pub fn clear_controller(&mut self) {
        self.preferred_controller = None;
        self.preferred_controller_name = None;
    }

    /// Set the preferred audio device
    pub fn set_audio(&mut self, sink_name: &str, display_name: &str) {
        self.preferred_audio = Some(sink_name.to_string());
        self.preferred_audio_name = Some(display_name.to_string());
    }

    /// Clear the preferred audio device
    pub fn clear_audio(&mut self) {
        self.preferred_audio = None;
        self.preferred_audio_name = None;
    }
}
