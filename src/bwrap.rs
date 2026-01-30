//! Bubblewrap container setup
//!
//! This module handles configuring bubblewrap (bwrap) for process isolation,
//! including input device blocking and SDL environment setup.

use std::process::Command;

use crate::input::{DeviceInfo, DeviceType};

/// Add base bwrap arguments to command
///
/// Sets up the container with full filesystem access but isolated /tmp
pub fn add_base_args(cmd: &mut Command) {
    cmd.arg("bwrap");
    cmd.arg("--die-with-parent");
    cmd.args(["--dev-bind", "/", "/"]);
    cmd.args(["--tmpfs", "/tmp"]);
    // Bind-mount the X11 socket directory so games can connect to gamescope's display
    // Without this, --tmpfs /tmp hides the socket and games fail to launch
    cmd.args(["--bind", "/tmp/.X11-unix", "/tmp/.X11-unix"]);
}

/// Set up SDL environment variables inside the bwrap container
///
/// These are passed via --setenv so they apply inside the container, not to gamescope
pub fn setup_sdl_env(cmd: &mut Command, gamepad_paths: &[String]) {
    // SDL joystick configuration
    cmd.args(["--setenv", "SDL_JOYSTICK_HIDAPI", "0"]);
    cmd.args(["--setenv", "SDL_JOYSTICK_LINUX_EVDEV", "1"]);
    cmd.args(["--setenv", "SDL_JOYSTICK_LINUX_CLASSIC", "1"]);
    cmd.args(["--setenv", "SDL_GAMECONTROLLER_USE_BUTTON_LABELS", "1"]);
    cmd.args(["--setenv", "SDL_VIDEODRIVER", "x11"]);

    // Debug logging for SDL joystick (can be helpful for troubleshooting)
    cmd.args(["--setenv", "SDL_JOYSTICK_DEBUG", "1"]);
    cmd.args(["--setenv", "SDL_LOGGING", "debug"]);

    // Set the specific gamepad device(s) for this instance
    if !gamepad_paths.is_empty() {
        cmd.args(["--setenv", "SDL_JOYSTICK_DEVICE", &gamepad_paths.join(",")]);
    }
}

/// Set up audio routing environment variables inside the bwrap container
///
/// Sets PULSE_SINK to route audio to a specific sink (works for both
/// PulseAudio and PipeWire via pipewire-pulse compatibility layer)
pub fn setup_audio_env(cmd: &mut Command, sink_name: &str) {
    if sink_name.is_empty() {
        return;
    }
    // PULSE_SINK works for both PulseAudio and PipeWire (via pipewire-pulse)
    cmd.args(["--setenv", "PULSE_SINK", sink_name]);
}

/// Get all /dev/input/js* device paths (legacy joystick interface)
pub fn glob_js_devices() -> Vec<String> {
    let mut devices = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("js") {
                devices.push(format!("/dev/input/{}", name_str));
            }
        }
    }
    devices
}

/// Get evdev paths for all gamepads NOT assigned to this instance
pub fn get_unassigned_gamepad_evdev(
    input_devices: &[DeviceInfo],
    assigned_indices: &[usize],
) -> Vec<String> {
    let mut to_block = Vec::new();
    for (i, dev) in input_devices.iter().enumerate() {
        if dev.device_type == DeviceType::Gamepad && !assigned_indices.contains(&i) {
            to_block.push(dev.path.clone());
        }
    }
    to_block
}

/// Get hidraw devices for gamepads NOT assigned to this instance
///
/// This blocks HIDAPI access to other players' controllers.
/// Works by:
/// 1. Finding hidraw devices that share a parent with known gamepad evdev devices
/// 2. Matching Bluetooth controllers by HID_UNIQ (MAC address)
pub fn get_gamepad_hidraw_devices(
    input_devices: &[DeviceInfo],
    assigned_indices: &[usize],
) -> Vec<String> {
    let mut to_block = Vec::new();

    // Build maps for gamepad paths and UNIQs
    let mut all_gamepad_evdev: Vec<(&str, bool)> = Vec::new();
    let mut all_gamepad_uniq: Vec<(&str, bool)> = Vec::new();
    for (i, dev) in input_devices.iter().enumerate() {
        if dev.device_type == DeviceType::Gamepad {
            let is_assigned = assigned_indices.contains(&i);
            all_gamepad_evdev.push((dev.path.as_str(), is_assigned));
            if !dev.uniq.is_empty() {
                all_gamepad_uniq.push((dev.uniq.as_str(), is_assigned));
            }
        }
    }

    // For each hidraw device, check if it belongs to a gamepad
    if let Ok(entries) = std::fs::read_dir("/sys/class/hidraw") {
        for entry in entries.flatten() {
            let hidraw_name = entry.file_name();
            let hidraw_name_str = hidraw_name.to_string_lossy();
            let device_path = entry.path().join("device");

            let mut found_gamepad = false;
            let mut is_assigned = false;

            // Method 1: Check HID_UNIQ from uevent (works for Bluetooth controllers)
            let uevent_path = device_path.join("uevent");
            if let Ok(uevent) = std::fs::read_to_string(&uevent_path) {
                for line in uevent.lines() {
                    if let Some(hid_uniq) = line.strip_prefix("HID_UNIQ=") {
                        // Check if this UNIQ matches any of our known gamepads
                        for (gamepad_uniq, gamepad_assigned) in &all_gamepad_uniq {
                            if *gamepad_uniq == hid_uniq {
                                found_gamepad = true;
                                is_assigned = *gamepad_assigned;
                                break;
                            }
                        }
                        break;
                    }
                }
            }

            // Method 2: Look for input/event* nodes under device (works for USB controllers)
            if !found_gamepad {
                if let Ok(device_entries) = std::fs::read_dir(&device_path) {
                    for dev_entry in device_entries.flatten() {
                        let dev_name = dev_entry.file_name();
                        let dev_str = dev_name.to_string_lossy();
                        if dev_str.starts_with("input") {
                            if let Ok(input_entries) = std::fs::read_dir(dev_entry.path()) {
                                for input_entry in input_entries.flatten() {
                                    let input_name = input_entry.file_name();
                                    let input_str = input_name.to_string_lossy();
                                    if input_str.starts_with("event") {
                                        let evdev_path = format!("/dev/input/{}", input_str);
                                        for (gamepad_path, gamepad_assigned) in &all_gamepad_evdev {
                                            if *gamepad_path == evdev_path {
                                                found_gamepad = true;
                                                is_assigned = *gamepad_assigned;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Block this hidraw if it's a gamepad that's NOT assigned to this instance
            if found_gamepad && !is_assigned {
                to_block.push(format!("/dev/{}", hidraw_name_str));
            }
        }
    }

    to_block
}

/// Build blocking args for js devices. Returns bwrap --bind args as a flat Vec.
///
/// Call this RIGHT BEFORE spawning so we check which devices are currently
/// accessible. Gamescope may recreate js devices with different ownership.
pub fn get_js_blocking_args(initial_js_devices: &[String], instance_idx: usize) -> Vec<String> {
    let js_to_block: Vec<_> = initial_js_devices
        .iter()
        .filter(|p| {
            let path = std::path::Path::new(p);
            path.exists() && std::fs::OpenOptions::new().write(true).open(path).is_ok()
        })
        .collect();
    println!(
        "[splitux] Instance {}: Blocking {} js devices: {:?}",
        instance_idx,
        js_to_block.len(),
        js_to_block
    );
    let mut args = Vec::new();
    for js_path in &js_to_block {
        args.extend(["--bind".to_string(), "/dev/null".to_string(), js_path.to_string()]);
    }
    args
}

/// Build blocking args for evdev and hidraw devices. Returns bwrap --bind args as a flat Vec.
///
/// Call this RIGHT BEFORE spawning so we check which devices are currently
/// accessible. Gamescope may recreate devices with different ownership.
pub fn get_evdev_hidraw_blocking_args(
    input_devices: &[DeviceInfo],
    assigned_indices: &[usize],
    instance_idx: usize,
) -> Vec<String> {
    let mut args = Vec::new();

    // Block evdev devices with permission check
    let unassigned_evdev = get_unassigned_gamepad_evdev(input_devices, assigned_indices);
    let evdev_to_block: Vec<_> = unassigned_evdev
        .iter()
        .filter(|p| {
            let path = std::path::Path::new(p);
            path.exists() && std::fs::OpenOptions::new().write(true).open(path).is_ok()
        })
        .collect();

    println!(
        "[splitux] Instance {}: Blocking {} evdev devices: {:?}",
        instance_idx,
        evdev_to_block.len(),
        evdev_to_block
    );

    for path in &evdev_to_block {
        args.extend(["--bind".to_string(), "/dev/null".to_string(), path.to_string()]);
    }

    // Block hidraw devices with permission check
    let unassigned_hidraw = get_gamepad_hidraw_devices(input_devices, assigned_indices);
    let hidraw_to_block: Vec<_> = unassigned_hidraw
        .iter()
        .filter(|p| {
            let path = std::path::Path::new(p);
            path.exists() && std::fs::OpenOptions::new().write(true).open(path).is_ok()
        })
        .collect();

    println!(
        "[splitux] Instance {}: Blocking {} hidraw devices: {:?}",
        instance_idx,
        hidraw_to_block.len(),
        hidraw_to_block
    );

    for path in &hidraw_to_block {
        args.extend(["--bind".to_string(), "/dev/null".to_string(), path.to_string()]);
    }

    args
}

/// Log assigned device indices for this instance.
///
/// Note: Actual device blocking (js, evdev, hidraw) is now handled at spawn time
/// by block_js_devices() and block_evdev_hidraw_devices() to avoid race conditions
/// when gamescope recreates device nodes with different ownership.
pub fn log_assigned_devices(
    _cmd: &mut Command,
    _input_devices: &[DeviceInfo],
    assigned_indices: &[usize],
    instance_idx: usize,
) {
    println!(
        "[splitux] Instance {}: Assigned device indices: {:?}",
        instance_idx, assigned_indices
    );
}

/// Get gamepad evdev paths for assigned devices
pub fn get_assigned_gamepad_paths(
    input_devices: &[DeviceInfo],
    assigned_indices: &[usize],
) -> Vec<String> {
    assigned_indices
        .iter()
        .filter_map(|&d| {
            let dev = input_devices.get(d)?;
            if dev.device_type == DeviceType::Gamepad {
                Some(dev.path.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Set up BepInEx environment variables for Linux native games
///
/// These are passed via --setenv so they apply inside the container
pub fn setup_bepinex_env(cmd: &mut Command, env_vars: &std::collections::HashMap<String, String>) {
    for (key, value) in env_vars {
        cmd.args(["--setenv", key, value]);
    }
}
