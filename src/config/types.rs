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

/// One entry in the input-ignore list.
///
/// Some hardware exposes several evdev nodes that report the SAME name but differ
/// in kind — e.g. the ZSA Moonlander presents both a Keyboard node and a Mouse
/// (mousekeys) node, both named "ZSA Technology Labs Moonlander Mark I". A
/// name-only match can't separate them, so an entry may pin the device kind too.
/// Serialized untagged: a bare string stays a plain name in the JSON (so existing
/// configs keep working), while a kind-qualified entry is `{name, kind}`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(untagged)]
pub enum IgnoredDevice {
    /// Match every device with this exact name, regardless of kind (legacy form).
    Name(String),
    /// Match only the device with this name AND kind ("keyboard"|"mouse"|"gamepad").
    Typed { name: String, kind: String },
}

impl IgnoredDevice {
    /// The device name this entry targets.
    pub fn name(&self) -> &str {
        match self {
            IgnoredDevice::Name(n) => n,
            IgnoredDevice::Typed { name, .. } => name,
        }
    }

    /// The kind qualifier, if any (`None` = matches any kind).
    pub fn kind(&self) -> Option<&str> {
        match self {
            IgnoredDevice::Name(_) => None,
            IgnoredDevice::Typed { kind, .. } => Some(kind),
        }
    }

    /// Whether this entry drops a device of `(name, kind)`. A bare-name entry
    /// matches any kind; a typed entry must match both name and kind.
    pub fn matches(&self, name: &str, kind: &str) -> bool {
        match self {
            IgnoredDevice::Name(n) => n == name,
            IgnoredDevice::Typed { name: n, kind: k } => n == name && k == kind,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum WindowManagerType {
    #[default]
    Auto,
    KWin,
    Hyprland,
    GamescopeOnly,
}

/// GPU vendor for driver/library alignment. Centralizes the per-vendor graphics
/// env (LIBVA video driver, NVIDIA GLX/GBM selection) so launched games AND the
/// seat-streamer's HW video encoder all resolve the right driver stack, instead
/// of scattered hardcodes (this replaces a hardcoded `LIBVA_DRIVER_NAME=radeonsi`
/// in the together seat spawn). `Auto` detects from the active DRM render node.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum GpuVendor {
    #[default]
    Auto,
    Amd,
    Nvidia,
    Intel,
}

impl GpuVendor {
    /// Resolve `Auto` to a concrete vendor from the PCI vendor id of the first DRM
    /// render node (0x1002 AMD, 0x10de NVIDIA, 0x8086 Intel). Falls back to AMD if
    /// nothing is detectable.
    pub fn resolve(self) -> GpuVendor {
        if self != GpuVendor::Auto {
            return self;
        }
        for n in 128..136 {
            let p = format!("/sys/class/drm/renderD{n}/device/vendor");
            if let Ok(s) = std::fs::read_to_string(&p) {
                match s.trim() {
                    "0x10de" => return GpuVendor::Nvidia,
                    "0x8086" => return GpuVendor::Intel,
                    "0x1002" => return GpuVendor::Amd,
                    _ => continue,
                }
            }
        }
        GpuVendor::Amd
    }

    /// Driver/library env that aligns a launched game (and the seat-streamer's HW
    /// encoder) with the active GPU. Applied to both the game command and the
    /// seat-streamer command so neither inherits a stale/foreign driver name.
    pub fn driver_env(self) -> Vec<(&'static str, &'static str)> {
        match self.resolve() {
            GpuVendor::Amd => vec![("LIBVA_DRIVER_NAME", "radeonsi")],
            GpuVendor::Intel => vec![("LIBVA_DRIVER_NAME", "iHD")],
            GpuVendor::Nvidia => vec![
                ("LIBVA_DRIVER_NAME", "nvidia"),
                ("__GLX_VENDOR_LIBRARY_NAME", "nvidia"),
                ("GBM_BACKEND", "nvidia-drm"),
            ],
            // resolve() never returns Auto
            GpuVendor::Auto => vec![],
        }
    }
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
    /// Auto-hide the mouse cursor after ~1s of no pointer motion (gamescope
    /// `--hide-cursor-delay`). Off by default: a multi-instance or pad-driven
    /// session has windows that never see mouse motion, so the cursor would hide
    /// and never come back (clicks still land — it's just not drawn). Opt in for
    /// couch/TV play where a lingering arrow is the bigger annoyance.
    #[serde(default)]
    pub gamescope_autohide_cursor: bool,
    /// Bypass the nested gamescope compositor for a single LOCAL seat and run
    /// the game directly under the host compositor (like Lutris). Eliminates the
    /// double-compositor scan-line artifact on high-refresh physical panels:
    /// gamescope nested in niri presents to niri, niri then scans that surface
    /// out to the DCN at the panel's native refresh, and the two present paths
    /// desync on motion ("Compositor released us but we were not acquired").
    /// Opt-in. Engages ONLY when the launch is a single, non-split,
    /// non-together instance with bwrap enabled — gamescope is still required
    /// for split-screen geometry and together PipeWire capture, so multi-seat
    /// and together launches always keep it. The un-nested game inherits the
    /// session's X display (Xwayland-satellite) so its wine display driver is
    /// unchanged; only the redundant compositor layer is removed.
    #[serde(default)]
    pub disable_gamescope: bool,
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
    /// Exact evdev device names to drop during input scanning. Used to hide the
    /// phantom extra endpoints some keyboards/mice expose (e.g. a keyboard's
    /// "System Control" / "Consumer Control" node, or a trackball's secondary
    /// nodes) so they never clutter the device strip or get picked as a player
    /// seat. The in-app analog of the `99-splitux-not-joystick` udev rule.
    #[serde(default)]
    pub input_blacklist: Vec<IgnoredDevice>,
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
    /// GPU vendor for driver/library alignment (LIBVA video driver, NVIDIA
    /// GLX/GBM). `auto` (default) detects from the active DRM render node.
    #[serde(default)]
    pub gpu_vendor: GpuVendor,
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
/// Default per-seat fps tier. 120 is the locked Together default: browser WebRTC
/// clients software-decode the stream (no HW WebRTC decode on Linux Chromium; flaky
/// elsewhere), and 1080p200 (H.264 level 5.2) overruns software-decode budgets on
/// every client. 120 stays within them while matching common 120/144Hz displays.
/// 200 is the native/local ideal — raise it only with a hardware-decoding client.
pub fn default_fps() -> u32 {
    120
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
            gamescope_force_grab_cursor: true,
            input_holding: true,
            proton_version: "".to_string(),
            proton_separate_pfxs: true,
            vertical_two_player: false,
            layout_presets: LayoutPresets::default(),
            gamescope_autohide_cursor: false,
            disable_gamescope: false,
            pad_filter_type: PadFilterType::NoSteamInput,
            input_blacklist: Vec::new(),
            allow_multiple_instances_on_same_device: false,
            disable_mount_gamedirs: false,
            photon_app_ids: PhotonAppIds::default(),
            audio: AudioConfig::default(),
            master_profile: None,
            layout: LayoutState::default(),
            device_aliases: HashMap::new(),
            input_init_delay: None,
            gpu_vendor: GpuVendor::Auto,
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

