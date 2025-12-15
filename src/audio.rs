//! Audio routing for splitux - per-instance audio output assignment
//!
//! This module provides per-instance audio routing, allowing each game
//! instance to output to a different audio device (speakers, headphones, etc.).
//!
//! ## Supported Audio Systems
//! - **PulseAudio** (via `pactl`) - Works on native PA and PipeWire via pipewire-pulse
//! - **PipeWire Native** (via `wpctl`/`pw-cli`) - For users who prefer native PW tools
//!
//! ## Module Structure
//! - `types.rs`: AudioSystem, AudioSink, VirtualSink, AudioContext
//! - `pure/`: Pure functions (device classification, sink name helpers)
//! - `operations/`: Atomic I/O operations (scan, create, cleanup)
//! - `pipelines/`: High-level orchestration (setup_session, teardown_session)

mod operations;
mod pipelines;
mod pure;
mod types;

use std::process::Command;

// Re-export types
pub use types::{
    AudioContext, AudioDeviceType, AudioResult, AudioSink, AudioSystem, AudioSystemPreference,
    VirtualSink,
};

// Re-export operations
pub use operations::{cleanup_all_splitux_sinks, cleanup_sinks, create_virtual_sink, scan_sinks};

// Re-export pipelines
pub use pipelines::{setup_audio_session, teardown_audio_session};

// Re-export pure functions
pub use pure::{
    classify_device, generate_virtual_sink_description, generate_virtual_sink_name,
    is_splitux_sink, parse_module_id,
};

/// Detect available audio system
///
/// Checks for available audio tools and returns the detected system.
/// Prefers PulseAudio (pactl) as it works universally on both PA and PipeWire.
pub fn detect_audio_system() -> AudioSystem {
    // Check for pactl (PulseAudio or pipewire-pulse)
    let has_pactl = Command::new("pactl")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Check for wpctl (native PipeWire)
    let has_wpctl = Command::new("wpctl")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Prefer pactl (universal), wpctl as fallback
    if has_pactl {
        println!("[splitux] audio - Detected PulseAudio/PipeWire-pulse (pactl available)");
        AudioSystem::PulseAudio
    } else if has_wpctl {
        println!("[splitux] audio - Detected PipeWire native (wpctl available)");
        AudioSystem::PipeWireNative
    } else {
        println!("[splitux] audio - No audio system detected");
        AudioSystem::None
    }
}

/// Resolve user preference to actual audio system
///
/// Takes the user's preference and the detected system, returns the system to use.
pub fn resolve_audio_system(preference: AudioSystemPreference) -> AudioSystem {
    match preference {
        AudioSystemPreference::Auto => detect_audio_system(),
        AudioSystemPreference::PulseAudio => {
            // Check if pactl is available
            let has_pactl = Command::new("pactl")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if has_pactl {
                AudioSystem::PulseAudio
            } else {
                println!("[splitux] audio - Warning: PulseAudio requested but pactl not found");
                AudioSystem::None
            }
        }
        AudioSystemPreference::PipeWireNative => {
            // Check if wpctl is available
            let has_wpctl = Command::new("wpctl")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if has_wpctl {
                AudioSystem::PipeWireNative
            } else {
                println!(
                    "[splitux] audio - Warning: PipeWire native requested but wpctl not found"
                );
                AudioSystem::None
            }
        }
    }
}
