//! Audio session setup pipeline

use crate::audio::operations::{cleanup_all_splitux_sinks, create_mute_sink, create_virtual_sink};
use crate::audio::types::{AudioContext, AudioResult, VirtualSink, AUDIO_MUTED_SENTINEL};

/// Set up audio routing for a game session
///
/// Creates virtual sinks for each instance that has an audio assignment.
/// Returns the created virtual sinks (for cleanup) and the sink names to use
/// for PULSE_SINK environment variable per instance.
pub fn setup_audio_session(ctx: &AudioContext) -> AudioResult<(Vec<VirtualSink>, Vec<String>)> {
    // First, clean up any leftover sinks from previous sessions
    let _ = cleanup_all_splitux_sinks(ctx.system);

    let mut virtual_sinks = Vec::new();
    let mut sink_env_vars = Vec::new();

    for (instance_idx, maybe_target) in ctx.assignments.iter().enumerate() {
        if let Some(target_sink) = maybe_target {
            // Check if this is an explicit mute request
            if target_sink == AUDIO_MUTED_SENTINEL {
                // Create mute sink (null sink with no loopback - audio goes nowhere)
                match create_mute_sink(ctx.system, instance_idx) {
                    Ok(virtual_sink) => {
                        sink_env_vars.push(virtual_sink.sink_name.clone());
                        virtual_sinks.push(virtual_sink);
                    }
                    Err(e) => {
                        println!(
                            "[splitux] audio - Warning: Failed to create mute sink for instance {}: {}",
                            instance_idx, e
                        );
                        sink_env_vars.push(String::new());
                    }
                }
            } else {
                // Create virtual sink routed to the target physical device
                match create_virtual_sink(ctx.system, instance_idx, target_sink) {
                    Ok(virtual_sink) => {
                        sink_env_vars.push(virtual_sink.sink_name.clone());
                        virtual_sinks.push(virtual_sink);
                    }
                    Err(e) => {
                        // Log error but continue - audio failure shouldn't block game launch
                        println!(
                            "[splitux] audio - Warning: Failed to create virtual sink for instance {}: {}",
                            instance_idx, e
                        );
                        sink_env_vars.push(String::new());
                    }
                }
            }
        } else {
            // No assignment: use default sink (empty string means no override)
            sink_env_vars.push(String::new());
        }
    }

    println!(
        "[splitux] audio - Session setup complete: {} virtual sinks created",
        virtual_sinks.len()
    );

    Ok((virtual_sinks, sink_env_vars))
}
