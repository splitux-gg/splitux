//! Audio session teardown pipeline

use crate::audio::operations::cleanup_sinks;
use crate::audio::types::{AudioResult, AudioSystem, VirtualSink};

/// Tear down audio routing after a game session
pub fn teardown_audio_session(system: AudioSystem, virtual_sinks: &[VirtualSink]) -> AudioResult<()> {
    if virtual_sinks.is_empty() {
        println!("[splitux] audio - No virtual sinks to clean up");
        return Ok(());
    }

    println!(
        "[splitux] audio - Tearing down {} virtual sinks",
        virtual_sinks.len()
    );

    cleanup_sinks(system, virtual_sinks)?;

    println!("[splitux] audio - Session teardown complete");
    Ok(())
}
