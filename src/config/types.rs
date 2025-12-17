use crate::audio::AudioSystemPreference;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum PadFilterType {
    All,
    NoSteamInput,
    OnlySteamInput,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum WindowManagerType {
    #[default]
    Auto,
    KWin,
    Hyprland,
    GamescopeOnly,
}

/// Photon App IDs for LocalMultiplayer mod
/// Get free App IDs from https://dashboard.photonengine.com
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct PhotonAppIds {
    /// Photon PUN App ID (required for Photon games)
    #[serde(default)]
    pub pun_app_id: String,
    /// Photon Voice App ID (optional, for voice chat)
    #[serde(default)]
    pub voice_app_id: String,
}

/// State for a collapsible/resizable UI panel
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct PanelState {
    /// Whether the panel is collapsed
    #[serde(default)]
    pub collapsed: bool,
    /// Custom width set by user (None = use default)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_width: Option<f32>,
}

/// UI layout state (panel positions, sizes, collapse state)
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct LayoutState {
    #[serde(default)]
    pub games_panel: PanelState,
    #[serde(default)]
    pub devices_panel: PanelState,
}

/// Audio routing configuration for per-instance audio output
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct AudioConfig {
    /// Enable per-instance audio routing
    #[serde(default)]
    pub enabled: bool,
    /// Which audio system to use (Auto, PulseAudio, PipeWireNative)
    #[serde(default)]
    pub system: AudioSystemPreference,
    /// Default sink assignments by instance index (0-based)
    /// e.g., { 0: "alsa_output.usb-headphones", 1: "alsa_output.pci-speakers" }
    #[serde(default)]
    pub default_assignments: HashMap<usize, String>,
}

/// Main application configuration
/// (renamed from PartyConfig)
#[derive(Serialize, Deserialize, Clone)]
pub struct SplituxConfig {
    #[serde(default)]
    pub window_manager: WindowManagerType,
    // Keep enable_kwin_script for backwards compatibility (will be migrated)
    #[serde(default = "default_enable_kwin_script")]
    pub enable_kwin_script: bool,
    pub gamescope_fix_lowres: bool,
    pub gamescope_sdl_backend: bool,
    pub gamescope_force_grab_cursor: bool,
    #[serde(alias = "kbm_support")] // backwards compatibility
    pub input_holding: bool,
    pub proton_version: String,
    pub proton_separate_pfxs: bool,
    #[serde(default)]
    pub vertical_two_player: bool,
    pub pad_filter_type: PadFilterType,
    #[serde(default)]
    pub allow_multiple_instances_on_same_device: bool,
    pub disable_mount_gamedirs: bool,
    /// Photon App IDs for games using Photon networking
    #[serde(default)]
    pub photon_app_ids: PhotonAppIds,
    /// Audio routing configuration
    #[serde(default)]
    pub audio: AudioConfig,
    /// Master profile name - syncs saves to/from original game location
    /// The machine owner typically sets their profile as master
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_profile: Option<String>,
    /// UI layout preferences (panel collapse state, widths)
    #[serde(default)]
    pub layout: LayoutState,
    /// Custom device names (maps device unique ID -> user-assigned name)
    #[serde(default)]
    pub device_aliases: HashMap<String, String>,
}

fn default_enable_kwin_script() -> bool {
    true
}

impl Default for SplituxConfig {
    fn default() -> Self {
        SplituxConfig {
            window_manager: WindowManagerType::Auto,
            enable_kwin_script: true,
            gamescope_fix_lowres: true,
            gamescope_sdl_backend: true,
            gamescope_force_grab_cursor: false,
            input_holding: true,
            proton_version: "".to_string(),
            proton_separate_pfxs: true,
            vertical_two_player: false,
            pad_filter_type: PadFilterType::NoSteamInput,
            allow_multiple_instances_on_same_device: false,
            disable_mount_gamedirs: false,
            photon_app_ids: PhotonAppIds::default(),
            audio: AudioConfig::default(),
            master_profile: None,
            layout: LayoutState::default(),
            device_aliases: HashMap::new(),
        }
    }
}

/// Type alias for backward compatibility during migration
pub type PartyConfig = SplituxConfig;
