//! PulseAudio operations (via pactl)
//!
//! Works with both native PulseAudio and PipeWire's pipewire-pulse compatibility layer.

use std::process::Command;

use crate::audio::pure::{
    classify_device, generate_virtual_sink_description, generate_virtual_sink_name,
    is_splitux_sink, parse_module_id, parse_sink_owner_pid,
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
pub fn create_mute_sink(ns: &str, instance_idx: usize) -> AudioResult<VirtualSink> {
    create_null_sink(
        ns,
        instance_idx,
        &format!("Splitux Instance {} (Muted)", instance_idx),
        "mute sink",
    )
}

/// Create a capture sink for an instance (null sink with no loopback).
///
/// Structurally identical to a mute sink, but its monitor is meant to be
/// captured by a splitux-together seat-streamer (`<sink>.monitor`) so the
/// instance's audio reaches ONLY that seat — giving each together instance its
/// own audio stream instead of every seat sharing the default sink's monitor.
pub fn create_capture_sink(ns: &str, instance_idx: usize) -> AudioResult<VirtualSink> {
    create_null_sink(
        ns,
        instance_idx,
        &format!("Splitux Instance {} (Remote Capture)", instance_idx + 1),
        "capture sink",
    )
}

/// Create a bare null sink (no loopback) for an instance. Shared by the mute and
/// capture paths, which differ only in description/logging.
fn create_null_sink(
    ns: &str,
    instance_idx: usize,
    description: &str,
    kind: &str,
) -> AudioResult<VirtualSink> {
    let sink_name = generate_virtual_sink_name(ns, instance_idx);

    println!("[splitux] audio - Creating {} '{}' (no output)", kind, sink_name);

    // Create null sink only (no loopback = audio goes nowhere on the host).
    // rate/channels pinned so a downstream monitor capture sees a stable format.
    //
    // node.latency pins a small FIXED quantum on the PipeWire node. Without it the
    // null sink free-runs on PipeWire's floating quantum and its monitor delivers
    // audio in coarse, irregular bursts; the seat-streamer's pulsesrc (25ms
    // latency-time) then can't read fast enough (acap ~30 vs the 50 frames/s the
    // Opus encoder wants) → the stream stutters/cuts. A ~10ms quantum (512/48000)
    // gives the monitor a steady, fine-grained real-time cadence the capture keeps
    // up with. Harmless on plain PulseAudio (unknown sink_properties are ignored).
    let null_sink_output = Command::new("pactl")
        .args([
            "load-module",
            "module-null-sink",
            &format!("sink_name={}", sink_name),
            "rate=48000",
            "channels=2",
            &format!(
                "sink_properties=device.description=\"{}\" node.latency=512/48000",
                description.replace(' ', "\\ ")
            ),
        ])
        .output()?;

    if !null_sink_output.status.success() {
        return Err(format!(
            "Failed to create {}: {}",
            kind,
            String::from_utf8_lossy(&null_sink_output.stderr)
        )
        .into());
    }

    let module_id = parse_module_id(&String::from_utf8_lossy(&null_sink_output.stdout))
        .ok_or("Failed to parse null-sink module ID")?;

    println!(
        "[splitux] audio - Created {} {} (module {})",
        kind, sink_name, module_id
    );

    Ok(VirtualSink {
        sink_name,
        cleanup_ids: vec![module_id],
    })
}

/// Create a virtual sink for an instance, routed to the target physical sink
pub fn create_virtual_sink(
    ns: &str,
    instance_idx: usize,
    target_sink: &str,
) -> AudioResult<VirtualSink> {
    let sink_name = generate_virtual_sink_name(ns, instance_idx);
    let description = generate_virtual_sink_description(instance_idx);

    println!(
        "[splitux] audio - Creating virtual sink '{}' -> '{}'",
        sink_name, target_sink
    );

    // Create null sink (virtual output that captures audio)
    // Specify rate/channels to match loopback and avoid resampling
    let null_sink_output = Command::new("pactl")
        .args([
            "load-module",
            "module-null-sink",
            &format!("sink_name={}", sink_name),
            "rate=48000",
            "channels=2",
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
    // Optimized settings to prevent crackling/grain:
    // - 30ms latency: imperceptible but stable (1ms caused underruns)
    // - 48kHz rate: matches null sink to avoid resampling
    // - adjust_time=3: less frequent rate corrections
    // - max_latency=60ms: prevents latency drift
    let loopback_output = Command::new("pactl")
        .args([
            "load-module",
            "module-loopback",
            &format!("source={}.monitor", sink_name),
            &format!("sink={}", target_sink),
            "source_dont_move=true",
            "sink_dont_move=true",
            "latency_msec=30",
            "max_latency_msec=60",
            "adjust_time=3",
            "rate=48000",
            "channels=2",
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

/// Reap splitux sink modules left behind by DEAD launches (crash recovery),
/// without touching LIVE concurrent sessions' sinks.
///
/// Each sink name carries its owning launch's pid (`splitux_instance_<pid>_…`),
/// so a module whose pid is no longer running is a crashed-launch orphan and is
/// unloaded; sinks owned by a live pid (another concurrent splitux process) are
/// left alone. Safe to call at session start even while other sessions run.
pub fn cleanup_orphan_sinks() -> AudioResult<()> {
    let output = Command::new("pactl")
        .args(["list", "modules", "short"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        // A splitux sink module references its sink by name in its arguments:
        //   null-sink:  `sink_name=splitux_instance_<pid>_<counter>_<idx>`
        //   loopback:   `source=splitux_instance_<pid>_<counter>_<idx>.monitor`
        // Find whichever `key=value` token names a splitux sink (strip a trailing
        // `.monitor`), then recover the owning launch pid from it.
        let Some(name) = line.split_whitespace().find_map(|tok| {
            let val = tok.split_once('=')?.1;
            let val = val.strip_suffix(".monitor").unwrap_or(val);
            is_splitux_sink(val).then_some(val)
        }) else {
            continue;
        };
        let Some(pid) = parse_sink_owner_pid(name) else {
            continue;
        };
        if crate::util::pid_alive(pid) {
            continue; // a LIVE concurrent session — leave its sink alone
        }
        if let Some(module_id) = line.split_whitespace().next() {
            println!(
                "[splitux] audio - Reaping orphan sink module {} ({}, dead pid {})",
                module_id, name, pid
            );
            let _ = unload_module(module_id);
        }
    }

    Ok(())
}
