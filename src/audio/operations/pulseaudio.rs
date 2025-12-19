//! PulseAudio operations (via pactl)
//!
//! Works with both native PulseAudio and PipeWire's pipewire-pulse compatibility layer.

use std::process::Command;

use crate::audio::pure::{
    classify_device, generate_virtual_sink_description, generate_virtual_sink_name,
    is_splitux_sink, parse_module_id,
};
use crate::audio::types::{AudioResult, AudioSink, VirtualSink};

/// Scan available audio sinks using pactl
pub fn scan_sinks() -> AudioResult<Vec<AudioSink>> {
    // Get default sink first
    let default_output = Command::new("pactl").args(["get-default-sink"]).output()?;
    let default_sink = String::from_utf8_lossy(&default_output.stdout)
        .trim()
        .to_string();

    // Get detailed sink list
    let output = Command::new("pactl").args(["list", "sinks"]).output()?;

    if !output.status.success() {
        return Err("pactl list sinks failed".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sinks = parse_pactl_sinks(&stdout, &default_sink);

    // Filter out our own virtual sinks
    let sinks: Vec<_> = sinks
        .into_iter()
        .filter(|s| !is_splitux_sink(&s.name))
        .collect();

    Ok(sinks)
}

/// Parse pactl list sinks output into AudioSink structs
fn parse_pactl_sinks(output: &str, default_sink: &str) -> Vec<AudioSink> {
    let mut sinks = Vec::new();
    let mut current_name = String::new();

    for line in output.lines() {
        let line = line.trim();

        if line.starts_with("Name:") {
            current_name = line.strip_prefix("Name:").unwrap_or("").trim().to_string();
        } else if line.starts_with("Description:") {
            let description = line
                .strip_prefix("Description:")
                .unwrap_or("")
                .trim()
                .to_string();

            // We have both name and description, create the sink
            if !current_name.is_empty() {
                let device_type = classify_device(&current_name, &description);
                let is_default = current_name == default_sink;

                sinks.push(AudioSink {
                    name: current_name.clone(),
                    description,
                    device_type,
                    is_default,
                });
            }
        }
    }

    sinks
}

/// Create a mute sink for an instance (null sink with no loopback)
///
/// Audio sent to this sink goes nowhere - used for explicit muting
pub fn create_mute_sink(instance_idx: usize) -> AudioResult<VirtualSink> {
    let sink_name = generate_virtual_sink_name(instance_idx);
    let description = format!("Splitux Instance {} (Muted)", instance_idx);

    println!(
        "[splitux] audio - Creating mute sink '{}' (no output)",
        sink_name
    );

    // Create null sink only (no loopback = audio goes nowhere)
    let null_sink_output = Command::new("pactl")
        .args([
            "load-module",
            "module-null-sink",
            &format!("sink_name={}", sink_name),
            &format!(
                "sink_properties=device.description=\"{}\"",
                description.replace(' ', "\\ ")
            ),
        ])
        .output()?;

    if !null_sink_output.status.success() {
        return Err(format!(
            "Failed to create mute sink: {}",
            String::from_utf8_lossy(&null_sink_output.stderr)
        )
        .into());
    }

    let module_id = parse_module_id(&String::from_utf8_lossy(&null_sink_output.stdout))
        .ok_or("Failed to parse mute sink module ID")?;

    println!(
        "[splitux] audio - Created mute sink {} (module {})",
        sink_name, module_id
    );

    Ok(VirtualSink {
        sink_name,
        cleanup_ids: vec![module_id],
    })
}

/// Create a virtual sink for an instance, routed to the target physical sink
pub fn create_virtual_sink(instance_idx: usize, target_sink: &str) -> AudioResult<VirtualSink> {
    let sink_name = generate_virtual_sink_name(instance_idx);
    let description = generate_virtual_sink_description(instance_idx);

    println!(
        "[splitux] audio - Creating virtual sink '{}' -> '{}'",
        sink_name, target_sink
    );

    // Create null sink (virtual output that captures audio)
    let null_sink_output = Command::new("pactl")
        .args([
            "load-module",
            "module-null-sink",
            &format!("sink_name={}", sink_name),
            &format!(
                "sink_properties=device.description=\"{}\"",
                description.replace(' ', "\\ ")
            ),
        ])
        .output()?;

    if !null_sink_output.status.success() {
        return Err(format!(
            "Failed to create null sink: {}",
            String::from_utf8_lossy(&null_sink_output.stderr)
        )
        .into());
    }

    let module_id = parse_module_id(&String::from_utf8_lossy(&null_sink_output.stdout))
        .ok_or("Failed to parse null-sink module ID")?;

    // Create loopback to route null sink's monitor to the target physical sink
    let loopback_output = Command::new("pactl")
        .args([
            "load-module",
            "module-loopback",
            &format!("source={}.monitor", sink_name),
            &format!("sink={}", target_sink),
            "latency_msec=1", // Low latency for gaming
        ])
        .output()?;

    if !loopback_output.status.success() {
        // Cleanup the null sink we just created
        let _ = Command::new("pactl")
            .args(["unload-module", &module_id])
            .output();

        return Err(format!(
            "Failed to create loopback: {}",
            String::from_utf8_lossy(&loopback_output.stderr)
        )
        .into());
    }

    let loopback_id = parse_module_id(&String::from_utf8_lossy(&loopback_output.stdout))
        .ok_or("Failed to parse loopback module ID")?;

    println!(
        "[splitux] audio - Created virtual sink {} (module {}) -> {} (loopback {})",
        sink_name, module_id, target_sink, loopback_id
    );

    Ok(VirtualSink {
        sink_name,
        cleanup_ids: vec![loopback_id, module_id], // Loopback first, then null sink
    })
}

/// Cleanup virtual sinks by unloading their modules
pub fn cleanup_sinks(sinks: &[VirtualSink]) -> AudioResult<()> {
    let mut errors = Vec::new();

    for sink in sinks {
        println!(
            "[splitux] audio - Cleaning up virtual sink {} (modules: {:?})",
            sink.sink_name, sink.cleanup_ids
        );

        // Unload in order (loopback first, then null sink)
        for module_id in &sink.cleanup_ids {
            if let Err(e) = unload_module(module_id) {
                errors.push(format!("module {}: {}", module_id, e));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("Some modules failed to unload: {}", errors.join(", ")).into())
    }
}

/// Unload a single PulseAudio module
fn unload_module(module_id: &str) -> AudioResult<()> {
    let output = Command::new("pactl")
        .args(["unload-module", module_id])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Failed to unload module {}: {}",
            module_id,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    Ok(())
}

/// Emergency cleanup: unload all splitux-related modules
///
/// Used when we don't have the module IDs (e.g., crash recovery)
pub fn cleanup_all_splitux_sinks() -> AudioResult<()> {
    let output = Command::new("pactl")
        .args(["list", "modules", "short"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if line.contains("splitux_instance_") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(module_id) = parts.first() {
                println!(
                    "[splitux] audio - Emergency cleanup: unloading module {}",
                    module_id
                );
                let _ = unload_module(module_id);
            }
        }
    }

    Ok(())
}
