//! gptokeyb operations - binary detection, config resolution, process spawning

use std::path::{Path, PathBuf};
use std::process::{Child, Command};

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

/// Spawn gptokeyb daemon for an instance
///
/// Returns the child process handle if successful.
/// The daemon will read from the specified controller device and create
/// a virtual keyboard/mouse via uinput.
pub fn spawn_daemon(
    settings: &GptokeybSettings,
    handler_dir: &Path,
    device: &DeviceInfo,
    instance_idx: usize,
) -> Result<Child, Box<dyn std::error::Error>> {
    if !is_available() {
        return Err("gptokeyb binary not found".into());
    }

    let config_path = get_config_path(settings, handler_dir)
        .ok_or_else(|| format!("gptokeyb profile '{}' not found", settings.profile))?;

    let mut cmd = Command::new(BIN_GPTOKEYB.as_path());

    // Config file
    cmd.arg("-c").arg(&config_path);

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
    Ok(child)
}

/// Spawn gptokeyb daemons for all instances that need them
///
/// Returns a vector of child handles (one per instance that has gptokeyb enabled).
/// Instances without gptokeyb enabled will have None in their slot.
pub fn spawn_all_daemons(
    settings: &GptokeybSettings,
    handler_dir: &Path,
    input_devices: &[DeviceInfo],
    instance_device_indices: &[Vec<usize>],
) -> Vec<Option<Child>> {
    if !settings.is_enabled() {
        return (0..instance_device_indices.len()).map(|_| None).collect();
    }

    instance_device_indices
        .iter()
        .enumerate()
        .map(|(i, device_indices)| {
            // Get the first gamepad device for this instance
            let gamepad = device_indices
                .iter()
                .filter_map(|&idx| input_devices.get(idx))
                .find(|d| d.device_type == crate::input::DeviceType::Gamepad);

            match gamepad {
                Some(device) => {
                    match spawn_daemon(settings, handler_dir, device, i) {
                        Ok(child) => Some(child),
                        Err(e) => {
                            println!("[splitux] gptokeyb - Instance {}: Failed to spawn: {}", i, e);
                            None
                        }
                    }
                }
                None => {
                    println!("[splitux] gptokeyb - Instance {}: No gamepad assigned, skipping", i);
                    None
                }
            }
        })
        .collect()
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
