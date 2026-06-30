//! PipeWire native operations (via wpctl/pw-cli)
//!
//! Uses native PipeWire tools instead of PulseAudio compatibility layer.

use std::process::Command;

use crate::audio::pure::{
    classify_device, generate_virtual_sink_description, generate_virtual_sink_name,
    is_splitux_sink, parse_sink_owner_pid,
};
use crate::audio::types::{AudioResult, AudioSink, VirtualSink};

/// Scan available audio sinks using wpctl
pub fn scan_sinks() -> AudioResult<Vec<AudioSink>> {
    // Get wpctl status output
    let output = Command::new("wpctl").args(["status"]).output()?;

    if !output.status.success() {
        return Err("wpctl status failed".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sinks = parse_wpctl_sinks(&stdout);

    // Filter out our own virtual sinks
    let sinks: Vec<_> = sinks
        .into_iter()
        .filter(|s| !is_splitux_sink(&s.name))
        .collect();

    Ok(sinks)
}

/// Parse wpctl status output for audio sinks
///
/// wpctl status shows sinks under "Audio" -> "Sinks:" section
fn parse_wpctl_sinks(output: &str) -> Vec<AudioSink> {
    let mut sinks = Vec::new();
    let mut in_sinks_section = false;

    for line in output.lines() {
        // Look for Sinks section header
        if line.contains("Sinks:") {
            in_sinks_section = true;
            continue;
        }

        // Exit sinks section on next section header
        if in_sinks_section && (line.contains("Sources:") || line.contains("Streams:")) {
            break;
        }

        if in_sinks_section {
            // Parse sink line format: "  │  * 46. node_name [vol: 1.00]"
            // or "  │    46. node_name [vol: 1.00]"
            if let Some(sink) = parse_wpctl_sink_line(line) {
                sinks.push(sink);
            }
        }
    }

    sinks
}

/// Parse a single wpctl sink line
fn parse_wpctl_sink_line(line: &str) -> Option<AudioSink> {
    // Strip tree drawing characters and whitespace
    let cleaned = line
        .replace(['│', '├', '└', '─'], "")
        .trim()
        .to_string();

    if cleaned.is_empty() {
        return None;
    }

    // Check if this is the default sink (marked with *)
    let is_default = cleaned.starts_with('*');
    let cleaned = cleaned.trim_start_matches('*').trim();

    // Parse format: "46. Description [vol: 1.00]"
    let parts: Vec<&str> = cleaned.splitn(2, ". ").collect();
    if parts.len() < 2 {
        return None;
    }

    let node_id = parts[0].trim();
    let rest = parts[1];

    // Extract description (before [vol:])
    let description = rest
        .split('[')
        .next()
        .unwrap_or(rest)
        .trim()
        .to_string();

    if description.is_empty() {
        return None;
    }

    // Use node ID as the name for PipeWire
    let name = format!("pw_node_{}", node_id);
    let device_type = classify_device(&name, &description);

    Some(AudioSink {
        name,
        description,
        device_type,
        is_default,
    })
}

/// Create a mute sink for an instance (null sink with no output)
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

/// Create a capture sink for an instance (null sink with no output).
///
/// Structurally a mute sink, but its monitor is captured by a splitux-together
/// seat-streamer so the instance's audio reaches only that seat. See the
/// PulseAudio variant for the full rationale.
pub fn create_capture_sink(ns: &str, instance_idx: usize) -> AudioResult<VirtualSink> {
    create_null_sink(
        ns,
        instance_idx,
        &format!("Splitux Instance {} (Remote Capture)", instance_idx + 1),
        "capture sink",
    )
}

/// Create a bare null sink (no output) via pactl's compat layer. Shared by the
/// mute and capture paths, which differ only in description/logging.
fn create_null_sink(
    ns: &str,
    instance_idx: usize,
    description: &str,
    kind: &str,
) -> AudioResult<VirtualSink> {
    let sink_name = generate_virtual_sink_name(ns, instance_idx);

    println!(
        "[splitux] audio - Creating PipeWire {} '{}' (no output)",
        kind, sink_name
    );

    // Use pactl for compatibility
    // Specify rate/channels to match other sinks.
    // node.latency pins a small FIXED quantum so the monitor delivers steady
    // fine-grained audio the seat-streamer's pulsesrc can keep up with; without it
    // the free-running null sink starves the capture and the stream cuts. See the
    // PulseAudio variant for the full rationale.
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

    let module_id = crate::audio::pure::parse_module_id(&String::from_utf8_lossy(
        &null_sink_output.stdout,
    ))
    .ok_or("Failed to parse null-sink module ID")?;

    println!(
        "[splitux] audio - Created PipeWire {} {} (module {})",
        kind, sink_name, module_id
    );

    Ok(VirtualSink {
        sink_name,
        cleanup_ids: vec![module_id],
    })
}

/// Create a virtual sink for an instance using pw-cli
///
/// Note: PipeWire virtual sink creation is more complex than PulseAudio.
/// This uses the pipewire-pulse compat layer's module-null-sink internally.
pub fn create_virtual_sink(
    ns: &str,
    instance_idx: usize,
    target_sink: &str,
) -> AudioResult<VirtualSink> {
    let sink_name = generate_virtual_sink_name(ns, instance_idx);
    let description = generate_virtual_sink_description(instance_idx);

    println!(
        "[splitux] audio - Creating PipeWire virtual sink '{}' -> '{}'",
        sink_name, target_sink
    );

    // For now, use pactl through PipeWire's compatibility layer
    // Native pw-cli create-node is more complex and less portable
    //
    // Future: Could use pw-cli directly:
    // pw-cli create-node adapter { factory.name=support.null-audio-sink ... }
    //
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
            "Failed to create virtual sink: {}",
            String::from_utf8_lossy(&null_sink_output.stderr)
        )
        .into());
    }

    let module_id = crate::audio::pure::parse_module_id(&String::from_utf8_lossy(
        &null_sink_output.stdout,
    ))
    .ok_or("Failed to parse module ID")?;

    // Create link to target sink using pw-link
    // First, need to get the node IDs
    let link_result = create_pipewire_link(&sink_name, target_sink);

    let cleanup_ids = match link_result {
        Ok(link_id) => vec![link_id, module_id],
        Err(e) => {
            println!(
                "[splitux] audio - Warning: pw-link failed ({}), falling back to module-loopback",
                e
            );

            // Fallback to loopback module
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
                // Cleanup null sink
                let _ = Command::new("pactl")
                    .args(["unload-module", &module_id])
                    .output();
                return Err("Failed to create loopback".into());
            }

            let loopback_id = crate::audio::pure::parse_module_id(&String::from_utf8_lossy(
                &loopback_output.stdout,
            ))
            .ok_or("Failed to parse loopback module ID")?;

            vec![loopback_id, module_id]
        }
    };

    println!(
        "[splitux] audio - Created PipeWire virtual sink {} -> {}",
        sink_name, target_sink
    );

    Ok(VirtualSink {
        sink_name,
        cleanup_ids,
    })
}

/// Create a PipeWire link between nodes
fn create_pipewire_link(source_sink: &str, target_sink: &str) -> AudioResult<String> {
    // pw-link needs the actual port names
    // Format: source:output_FL, source:output_FR -> target:playback_FL, target:playback_FR

    let source_monitor = format!("{}.monitor:capture_FL", source_sink);
    let target_input = format!("{}:playback_FL", target_sink);

    let output = Command::new("pw-link")
        .args([&source_monitor, &target_input])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "pw-link failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    // pw-link doesn't return a link ID, we'll use a marker
    Ok(format!("pw-link:{}->{}", source_sink, target_sink))
}

/// Cleanup virtual sinks
pub fn cleanup_sinks(sinks: &[VirtualSink]) -> AudioResult<()> {
    let mut errors = Vec::new();

    for sink in sinks {
        println!(
            "[splitux] audio - Cleaning up PipeWire virtual sink {}",
            sink.sink_name
        );

        for cleanup_id in &sink.cleanup_ids {
            if cleanup_id.starts_with("pw-link:") {
                // This was a pw-link, need to destroy it
                // pw-link -d source:port target:port
                // For now, just skip - links are destroyed when nodes are destroyed
                continue;
            }

            // Module ID - use pactl to unload
            let output = Command::new("pactl")
                .args(["unload-module", cleanup_id])
                .output();

            if let Err(e) = output {
                errors.push(format!("module {}: {}", cleanup_id, e));
            } else if let Ok(o) = output
                && !o.status.success() {
                    errors.push(format!(
                        "module {}: {}",
                        cleanup_id,
                        String::from_utf8_lossy(&o.stderr)
                    ));
                }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("Some cleanup operations failed: {}", errors.join(", ")).into())
    }
}

/// Reap splitux sink modules left behind by DEAD launches (crash recovery),
/// without touching LIVE concurrent sessions' sinks. Mirrors the PulseAudio
/// variant — sinks/loopbacks are created via pactl's compat layer either way, so
/// we reap through pactl and skip any sink whose owning launch pid is still alive.
pub fn cleanup_orphan_sinks() -> AudioResult<()> {
    let output = Command::new("pactl")
        .args(["list", "modules", "short"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
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
            continue;
        }
        if let Some(module_id) = line.split_whitespace().next() {
            println!(
                "[splitux] audio - Reaping orphan sink module {} ({}, dead pid {})",
                module_id, name, pid
            );
            let _ = Command::new("pactl")
                .args(["unload-module", module_id])
                .output();
        }
    }

    Ok(())
}
