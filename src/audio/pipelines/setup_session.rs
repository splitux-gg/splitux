//! Audio session setup pipeline

use crate::audio::operations::{
    cleanup_orphan_sinks, create_capture_sink, create_mute_sink, create_virtual_sink,
};
use crate::audio::types::{
    AudioContext, AudioResult, VirtualSink, AUDIO_CAPTURE_SENTINEL, AUDIO_MUTED_SENTINEL,
};

/// Set up audio routing for a game session
///
/// Creates virtual sinks for each instance that has an audio assignment.
/// Returns the created virtual sinks (for cleanup) and the sink names to use
/// for PULSE_SINK environment variable per instance.
pub fn setup_audio_session(ctx: &AudioContext) -> AudioResult<(Vec<VirtualSink>, Vec<String>)> {
    // Reap sinks left by DEAD launches (crash recovery). Liveness-filtered by the
    // owning pid embedded in each sink name, so concurrent sessions are untouched.
    let _ = cleanup_orphan_sinks(ctx.system);

    let mut virtual_sinks = Vec::new();
    let mut sink_env_vars = Vec::new();

    for (instance_idx, maybe_target) in ctx.assignments.iter().enumerate() {
        if let Some(target_sink) = maybe_target {
            // Pick the sink flavor by sentinel: explicit mute, per-instance capture
            // (together audio isolation), or a virtual sink routed to a device.
            let result = if target_sink == AUDIO_MUTED_SENTINEL {
                create_mute_sink(ctx.system, &ctx.ns, instance_idx)
            } else if target_sink == AUDIO_CAPTURE_SENTINEL {
                create_capture_sink(ctx.system, &ctx.ns, instance_idx)
            } else {
                create_virtual_sink(ctx.system, &ctx.ns, instance_idx, target_sink)
            };
            match result {
                Ok(virtual_sink) => {
                    sink_env_vars.push(virtual_sink.sink_name.clone());
                    virtual_sinks.push(virtual_sink);
                }
                Err(e) => {
                    // Log error but continue - audio failure shouldn't block game launch
                    println!(
                        "[splitux] audio - Warning: Failed to create sink for instance {}: {}",
                        instance_idx, e
                    );
                    sink_env_vars.push(String::new());
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
