//! Audio module type definitions

use serde::{Deserialize, Serialize};

/// Sentinel value used in audio assignments to indicate explicit mute
/// When this value is in the assignments, a null sink is created (no loopback)
/// so audio goes nowhere instead of to the default output
pub const AUDIO_MUTED_SENTINEL: &str = "__muted__";

/// Sentinel value used in audio assignments to request a per-instance CAPTURE
/// sink. Like the mute sink it's a bare null sink (no loopback to hardware), but
/// its purpose is the opposite: a splitux-together seat-streamer captures this
/// sink's `.monitor` so the instance's audio reaches ONLY that seat's browser.
/// This is what gives each together instance its own audio stream instead of
/// every seat capturing the one shared default-sink monitor (audio bleed).
pub const AUDIO_CAPTURE_SENTINEL: &str = "__capture__";

/// Which audio system to use for virtual sink management
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioSystem {
    /// PulseAudio (pactl) - works on both native PA and PipeWire via pipewire-pulse
    PulseAudio,
    /// Native PipeWire (wpctl/pw-cli)
    PipeWireNative,
    /// No audio system available
    None,
}

impl AudioSystem {
    pub fn name(&self) -> &'static str {
        match self {
            AudioSystem::PulseAudio => "PulseAudio",
            AudioSystem::PipeWireNative => "PipeWire",
            AudioSystem::None => "None",
        }
    }
}

/// User preference for audio system (saved in config)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AudioSystemPreference {
    /// Auto-detect (defaults to PulseAudio if available)
    #[default]
    Auto,
    /// Force PulseAudio/pactl
    PulseAudio,
    /// Force PipeWire native tools
    PipeWireNative,
}

/// Classification of audio output device
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioDeviceType {
    Speaker,
    Headphone,
    Hdmi,
    Bluetooth,
    Virtual,
    Unknown,
}

/// Audio output device (sink)
#[derive(Clone, Debug)]
pub struct AudioSink {
    /// System sink name/ID (e.g., "alsa_output.pci-0000_00_1f.3.analog-stereo")
    pub name: String,
    /// Human-readable description (e.g., "Built-in Audio Analog Stereo")
    pub description: String,
    /// Device type classification
    pub device_type: AudioDeviceType,
    /// Whether this is the system default sink
    pub is_default: bool,
}

/// Virtual sink created for an instance (for cleanup tracking)
#[derive(Clone, Debug)]
pub struct VirtualSink {
    /// Virtual sink name (e.g., "splitux_instance_0")
    pub sink_name: String,
    /// IDs needed for cleanup (module IDs for PA, node IDs for PW)
    pub cleanup_ids: Vec<String>,
}

/// Context for audio session setup
#[derive(Clone, Debug)]
pub struct AudioContext {
    /// Which audio system to use
    pub system: AudioSystem,
    /// Target sink for each instance (None = use default)
    pub assignments: Vec<Option<String>>,
    /// Per-launch namespace (the launch_id, `<pid>_<counter>`). Embedded in every
    /// virtual sink name so CONCURRENT splitux processes never collide on
    /// `splitux_instance_0` and the orphan reaper can tell a crashed launch's
    /// leftover sinks (dead pid) from a live concurrent session's.
    pub ns: String,
}

/// Result type for audio operations
pub type AudioResult<T> = Result<T, Box<dyn std::error::Error>>;
