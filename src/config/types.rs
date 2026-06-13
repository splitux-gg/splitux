use crate::audio::AudioSystemPreference;
use crate::wm::presets::LayoutPresets;
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
#[derive(Clone, Serialize, Deserialize)]
pub struct LayoutState {
    #[serde(default)]
    pub games_panel: PanelState,
    #[serde(default = "default_devices_panel")]
    pub devices_panel: PanelState,
}

fn default_devices_panel() -> PanelState {
    PanelState {
        collapsed: true, // Devices panel collapsed by default
        custom_width: None,
    }
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            games_panel: PanelState::default(),
            devices_panel: default_devices_panel(),
        }
    }
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
    /// Legacy field - migrated to layout_presets on load
    #[serde(default, skip_serializing)]
    pub vertical_two_player: bool,
    /// Layout presets for each player count
    #[serde(default)]
    pub layout_presets: LayoutPresets,
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
    /// Delay before each instance spawn for input initialization (seconds)
    /// Allows previous instance's SDL/libinput to complete before spawning next
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_init_delay: Option<f64>,
    /// splitux-together: stream launched instances to remote players over WebRTC.
    #[serde(default)]
    pub together: TogetherConfig,
}

/// splitux-together configuration. When `enabled`, every launched instance also
/// gets a `seat-streamer` sidecar: its screen is captured from gamescope and
/// streamed to a remote browser, and the browser's input drives the instance's
/// virtual gamepad/keyboard/mouse. splitux pops up one invite URL per seat.
#[derive(Serialize, Deserialize, Clone)]
pub struct TogetherConfig {
    /// Producer signalling websocket the seat-streamers dial out to. Point this
    /// at a local orchestrator (`ws://127.0.0.1:8080/ws/producer`) or the public
    /// service (`wss://together.gabeforge.com/ws/producer`).
    #[serde(default = "default_signalling_uri")]
    pub signalling_uri: String,
    /// Public base URL the invite links are built from (`{base}/j/{token}`).
    /// e.g. `https://together.gabeforge.com` or `http://127.0.0.1:8080`.
    #[serde(default = "default_public_base_url")]
    pub public_base_url: String,
    /// When true, splitux spawns its own local orchestrator (serving the bundled
    /// web client) before the seats. When false, it assumes `signalling_uri`
    /// already points at a running service.
    #[serde(default = "default_true")]
    pub spawn_local_orchestrator: bool,
    /// GStreamer encoder for the seats: "va" (AMD VCN, production), "vulkan",
    /// "x264" (CPU fallback). See seat-streamer --encoder.
    #[serde(default = "default_encoder")]
    pub encoder: String,
    /// Per-seat target bitrate (kbps).
    #[serde(default = "default_bitrate")]
    pub bitrate: u32,
    /// Per-seat target fps tier (200/144/100/72). Drives both the gamescope
    /// capture refresh (`-r`) and the seat-streamer (`--fps`). 0 resolves to the
    /// default tier via `resolved_fps()`.
    #[serde(default = "default_fps")]
    pub fps: u32,
    /// STUN server URI for WebRTC.
    #[serde(default = "default_stun")]
    pub stun: String,
    /// Optional TURN relay (`turn://user:pass@host:3478`) for WAN paths where
    /// the host is behind NAT. Required for remote friends in practice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<String>,
}

fn default_signalling_uri() -> String {
    "ws://127.0.0.1:8080/ws/producer".to_string()
}
fn default_public_base_url() -> String {
    "http://127.0.0.1:8080".to_string()
}
fn default_true() -> bool {
    true
}
fn default_encoder() -> String {
    "va".to_string()
}
fn default_bitrate() -> u32 {
    20000
}
/// Default per-seat fps tier. The architecture targets 200/144/100/72; an unset
/// `together.fps` resolves to the top tier.
pub fn default_fps() -> u32 {
    200
}

impl TogetherConfig {
    /// Single source of truth for the seat fps, so the gamescope capture refresh
    /// (`-r`) and the seat-streamer (`--fps`) always agree. A 0 (explicit
    /// "unset") resolves to the default tier.
    pub fn resolved_fps(&self) -> u32 {
        if self.fps > 0 {
            self.fps
        } else {
            default_fps()
        }
    }
}
fn default_stun() -> String {
    "stun://stun.l.google.com:19302".to_string()
}

impl Default for TogetherConfig {
    fn default() -> Self {
        TogetherConfig {
            signalling_uri: default_signalling_uri(),
            public_base_url: default_public_base_url(),
            spawn_local_orchestrator: true,
            encoder: default_encoder(),
            bitrate: default_bitrate(),
            fps: default_fps(),
            stun: default_stun(),
            turn: None,
        }
    }
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
            layout_presets: LayoutPresets::default(),
            pad_filter_type: PadFilterType::NoSteamInput,
            allow_multiple_instances_on_same_device: false,
            disable_mount_gamedirs: false,
            photon_app_ids: PhotonAppIds::default(),
            audio: AudioConfig::default(),
            master_profile: None,
            layout: LayoutState::default(),
            device_aliases: HashMap::new(),
            input_init_delay: None,
            together: TogetherConfig::default(),
        }
    }
}

impl SplituxConfig {
    /// Migrate legacy settings to current format
    /// Call this after loading config from disk
    pub fn migrate(&mut self) {
        // Migrate vertical_two_player bool to layout_presets
        if self.vertical_two_player && self.layout_presets.two_player == "2p_horizontal" {
            self.layout_presets.two_player = "2p_vertical".to_string();
        }

        // Migrate deprecated 3p presets to new equal splits
        match self.layout_presets.three_player.as_str() {
            "3p_t_shape" | "3p_left_main" => {
                self.layout_presets.three_player = "3p_vertical".to_string();
            }
            "3p_inverted_t" | "3p_right_main" => {
                self.layout_presets.three_player = "3p_horizontal".to_string();
            }
            _ => {}
        }

        // Migrate deprecated 4p preset
        if self.layout_presets.four_player == "4p_main_plus_3" {
            self.layout_presets.four_player = "4p_grid".to_string();
        }
    }
}

