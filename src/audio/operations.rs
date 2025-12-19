//! Audio operations - atomic side effects
//!
//! Each function performs I/O operations with the audio system.
//! Operations dispatch to the appropriate implementation based on AudioSystem.

pub mod pipewire;
pub mod pulseaudio;

use crate::audio::types::{AudioResult, AudioSink, AudioSystem, VirtualSink};

/// Scan available audio sinks
pub fn scan_sinks(system: AudioSystem) -> AudioResult<Vec<AudioSink>> {
    match system {
        AudioSystem::PulseAudio => pulseaudio::scan_sinks(),
        AudioSystem::PipeWireNative => pipewire::scan_sinks(),
        AudioSystem::None => Err("No audio system available".into()),
    }
}

/// Create a virtual sink for an instance, routed to the target physical sink
pub fn create_virtual_sink(
    system: AudioSystem,
    instance_idx: usize,
    target_sink: &str,
) -> AudioResult<VirtualSink> {
    match system {
        AudioSystem::PulseAudio => pulseaudio::create_virtual_sink(instance_idx, target_sink),
        AudioSystem::PipeWireNative => pipewire::create_virtual_sink(instance_idx, target_sink),
        AudioSystem::None => Err("No audio system available".into()),
    }
}

/// Create a mute sink for an instance (null sink with no output)
///
/// Audio sent to this sink goes nowhere - used for explicit muting
pub fn create_mute_sink(system: AudioSystem, instance_idx: usize) -> AudioResult<VirtualSink> {
    match system {
        AudioSystem::PulseAudio => pulseaudio::create_mute_sink(instance_idx),
        AudioSystem::PipeWireNative => pipewire::create_mute_sink(instance_idx),
        AudioSystem::None => Err("No audio system available".into()),
    }
}

/// Cleanup virtual sinks
pub fn cleanup_sinks(system: AudioSystem, sinks: &[VirtualSink]) -> AudioResult<()> {
    match system {
        AudioSystem::PulseAudio => pulseaudio::cleanup_sinks(sinks),
        AudioSystem::PipeWireNative => pipewire::cleanup_sinks(sinks),
        AudioSystem::None => Ok(()), // Nothing to clean up
    }
}

/// Emergency cleanup: remove all splitux-related audio modules/nodes
pub fn cleanup_all_splitux_sinks(system: AudioSystem) -> AudioResult<()> {
    match system {
        AudioSystem::PulseAudio => pulseaudio::cleanup_all_splitux_sinks(),
        AudioSystem::PipeWireNative => pipewire::cleanup_all_splitux_sinks(),
        AudioSystem::None => Ok(()),
    }
}
