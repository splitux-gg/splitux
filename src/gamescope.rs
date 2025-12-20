//! Gamescope nested compositor setup
//!
//! This module handles building gamescope commands with the correct arguments
//! for resolution, display, and input handling.

use std::process::Command;

use crate::app::PartyConfig;
use crate::input::DeviceInfo;
use crate::input::DeviceType;
use crate::instance::Instance;
use crate::paths::BIN_GSC_SPLITUX;

/// Create the base gamescope command
///
/// Returns a Command for either gamescope or gamescope-splitux based on config
pub fn create_command(cfg: &PartyConfig) -> Command {
    let gamescope = match cfg.input_holding {
        true => BIN_GSC_SPLITUX.as_path(),
        false => std::path::Path::new("gamescope"),
    };
    Command::new(gamescope)
}

/// Set up gamescope environment variables
///
/// These are env vars that affect gamescope itself, not the game inside it
pub fn setup_env(cmd: &mut Command) {
    // Disable gamescope WSI layer
    cmd.env("ENABLE_GAMESCOPE_WSI", "0");

    // CRITICAL: Tell gamescope's SDL to NOT use any joysticks!
    // By pointing to /dev/null, SDL won't find any joysticks to enumerate.
    // This prevents gamescope from capturing gamepad input for window focus.
    // The actual gamepad device is passed to Wine via bwrap --setenv below.
    cmd.env("SDL_JOYSTICK_DEVICE", "/dev/null");

    // Disable SDL HiDPI scaling - we want pixel-exact window sizes
    // Without this, SDL on HiDPI displays (like 4K TVs with scale > 1.0)
    // will create windows at logical size (divided by scale) instead of
    // the requested pixel size.
    cmd.env("SDL_VIDEO_WAYLAND_SCALE", "1");
}

/// Add gamescope command-line arguments
pub fn add_args(cmd: &mut Command, instance: &Instance, cfg: &PartyConfig) {
    // Resolution
    cmd.args([
        "-W",
        &instance.width.to_string(),
        "-H",
        &instance.height.to_string(),
    ]);

    // Cursor hiding
    cmd.args(["--hide-cursor-delay", "1000"]);

    // Force grab cursor if enabled
    if cfg.gamescope_force_grab_cursor {
        cmd.arg("--force-grab-cursor");
    }

    // SDL backend with display index
    if cfg.gamescope_sdl_backend {
        cmd.arg("--backend=sdl");
        cmd.arg(format!("--display-index={}", instance.monitor));
    }
}

/// Add input device holding arguments for gamescope-splitux
///
/// This configures gamescope to hold specific input devices and disable backend input
/// for device types that this instance should use directly.
pub fn add_input_holding_args(
    cmd: &mut Command,
    instance: &Instance,
    input_devices: &[DeviceInfo],
    cfg: &PartyConfig,
) {
    if !cfg.input_holding {
        return;
    }

    let mut has_keyboard = false;
    let mut has_mouse = false;
    let mut held_devices = String::new();

    for &d in &instance.devices {
        let dev = &input_devices[d];
        match dev.device_type {
            DeviceType::Keyboard => {
                has_keyboard = true;
                held_devices.push_str(&format!("{},", &dev.path));
            }
            DeviceType::Mouse => {
                has_mouse = true;
                held_devices.push_str(&format!("{},", &dev.path));
            }
            _ => {}
        }
    }

    // When we have keyboard/mouse assigned, disable backend's default handling
    // so our libinput-held devices get exclusive input
    if has_keyboard {
        cmd.arg("--backend-disable-keyboard");
    }
    if has_mouse {
        cmd.arg("--backend-disable-mouse");
    }

    // Pass specific device paths to hold for libinput processing
    if !held_devices.is_empty() {
        cmd.arg(format!(
            "--libinput-hold-dev={}",
            held_devices.trim_end_matches(',')
        ));
    }
}

/// Add the separator between gamescope args and the inner command
pub fn add_separator(cmd: &mut Command) {
    cmd.arg("--");
}
