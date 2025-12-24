//! gptokeyb operations - binary detection, config resolution, process spawning

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Instant;

use super::types::GptokeybSettings;
use crate::input::DeviceInfo;
use crate::paths::{BIN_GPTOKEYB, PATH_RES};

/// Check if gptokeyb binary is available
pub fn is_available() -> bool {
    BIN_GPTOKEYB.exists()
}

/// Get the config file path for a profile
///
/// Returns:
/// - For built-in profiles: res/gptokeyb/{profile}.gptk
/// - For "custom": handler_dir/gptokeyb.gptk
pub fn get_config_path(settings: &GptokeybSettings, handler_dir: &Path) -> Option<PathBuf> {
    if settings.profile.is_empty() {
        return None;
    }

    if settings.profile == super::types::PROFILE_CUSTOM {
        // Custom profile in handler directory
        let custom_path = handler_dir.join("gptokeyb.gptk");
        if custom_path.exists() {
            Some(custom_path)
        } else {
            None
        }
    } else {
        // Built-in profile in res directory
        let builtin_path = PATH_RES.join("gptokeyb").join(format!("{}.gptk", settings.profile));
        if builtin_path.exists() {
            Some(builtin_path)
        } else {
            None
        }
    }
}

/// Wait for gptokeyb virtual device to appear for a specific instance
///
/// Searches /sys/class/input for a device named "Fake Keyboard Mouse {instance_id}".
/// Returns the /dev/input/eventN path if found within the timeout.
pub fn wait_for_virtual_device(instance_id: usize, timeout_ms: u64) -> Option<PathBuf> {
    let expected_name = format!("Fake Keyboard Mouse {}", instance_id);
    let start = Instant::now();

    while start.elapsed().as_millis() < timeout_ms as u128 {
        if let Ok(entries) = std::fs::read_dir("/sys/class/input") {
            for entry in entries.flatten() {
                // Only look for eventN devices (skip mouseN, jsN, etc.)
                let entry_name = entry.file_name();
                let entry_str = entry_name.to_string_lossy();
                if !entry_str.starts_with("event") {
                    continue;
                }

                let name_path = entry.path().join("device/name");
                if let Ok(name) = std::fs::read_to_string(&name_path) {
                    if name.trim() == expected_name {
                        return Some(PathBuf::from("/dev/input").join(entry_name));
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    None
}

/// Spawn gptokeyb daemon for an instance
///
/// Returns (child_process, virtual_device_path) if successful.
/// The daemon will read from the specified controller device and create
/// a virtual keyboard/mouse via uinput with a unique name per instance.
pub fn spawn_daemon(
    settings: &GptokeybSettings,
    handler_dir: &Path,
    device: &DeviceInfo,
    instance_idx: usize,
) -> Result<(Child, Option<PathBuf>), Box<dyn std::error::Error>> {
    if !is_available() {
        return Err("gptokeyb binary not found".into());
    }

    let config_path = get_config_path(settings, handler_dir)
        .ok_or_else(|| format!("gptokeyb profile '{}' not found", settings.profile))?;

    let mut cmd = Command::new(BIN_GPTOKEYB.as_path());

    // Set LD_LIBRARY_PATH so gptokeyb can find libinterpose.so
    if let Some(bin_dir) = BIN_GPTOKEYB.parent() {
        cmd.env("LD_LIBRARY_PATH", bin_dir);
    }

    // Config file
    cmd.arg("-c").arg(&config_path);

    // Controller isolation: target the first (and only) controller seen by SDL
    cmd.arg("-D").arg("0");

    // Unique device name for this instance
    cmd.arg("-n").arg(instance_idx.to_string());

    // Device path (gptokeyb will read from this specific controller)
    cmd.env("SDL_GAMECONTROLLERCONFIG_FILE", ""); // Clear any system mappings
    cmd.env("SDL_JOYSTICK_DEVICE", &device.path);

    // Override settings if specified
    if let Some(scale) = settings.mouse_scale {
        cmd.env("GPTOKEYB_MOUSE_SCALE", scale.to_string());
    }
    if let Some(delay) = settings.mouse_delay {
        cmd.env("GPTOKEYB_MOUSE_DELAY", delay.to_string());
    }

    println!(
        "[splitux] gptokeyb - Instance {}: profile={}, device={}",
        instance_idx, settings.profile, device.path
    );

    let child = cmd.spawn()?;

    // Wait for virtual device to appear (2 second timeout)
    let virtual_device = wait_for_virtual_device(instance_idx, 2000);
    if let Some(ref vdev) = virtual_device {
        println!(
            "[splitux] gptokeyb - Instance {}: virtual device at {}",
            instance_idx,
            vdev.display()
        );
    } else {
        println!(
            "[splitux] gptokeyb - Instance {}: warning: virtual device not detected",
            instance_idx
        );
    }

    Ok((child, virtual_device))
}

/// Spawn gptokeyb daemons for all instances that need them
///
/// Returns (child_handles, virtual_device_paths).
/// Instances without gptokeyb enabled will have None in their slot.
pub fn spawn_all_daemons(
    settings: &GptokeybSettings,
    handler_dir: &Path,
    input_devices: &[DeviceInfo],
    instance_device_indices: &[Vec<usize>],
) -> (Vec<Option<Child>>, Vec<Option<PathBuf>>) {
    let num_instances = instance_device_indices.len();

    if !settings.is_enabled() {
        return (
            (0..num_instances).map(|_| None).collect(),
            (0..num_instances).map(|_| None).collect(),
        );
    }

    let results: Vec<_> = instance_device_indices
        .iter()
        .enumerate()
        .map(|(i, device_indices)| {
            // Get the first gamepad device for this instance
            let gamepad = device_indices
                .iter()
                .filter_map(|&idx| input_devices.get(idx))
                .find(|d| d.device_type == crate::input::DeviceType::Gamepad);

            match gamepad {
                Some(device) => match spawn_daemon(settings, handler_dir, device, i) {
                    Ok((child, vdev)) => (Some(child), vdev),
                    Err(e) => {
                        println!(
                            "[splitux] gptokeyb - Instance {}: Failed to spawn: {}",
                            i, e
                        );
                        (None, None)
                    }
                },
                None => {
                    println!(
                        "[splitux] gptokeyb - Instance {}: No gamepad assigned, skipping",
                        i
                    );
                    (None, None)
                }
            }
        })
        .collect();

    // Unzip into separate vectors
    results.into_iter().unzip()
}

/// Terminate all gptokeyb daemons
pub fn terminate_all(handles: &mut [Option<Child>]) {
    for (i, handle) in handles.iter_mut().enumerate() {
        if let Some(child) = handle.take() {
            let pid = child.id();
            // Send SIGTERM first
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
            println!("[splitux] gptokeyb - Instance {}: Terminated (pid {})", i, pid);
        }
    }
}
