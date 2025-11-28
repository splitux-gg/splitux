//! Gamescope nested compositor setup
//!
//! This module handles building gamescope commands with the correct arguments
//! for resolution, display, and input handling.

use std::process::Command;

use crate::app::PartyConfig;
use crate::input::DeviceInfo;
use crate::input::DeviceType;
use crate::instance::Instance;
use crate::paths::BIN_GSC_KBM;

/// Create the base gamescope command
///
/// Returns a Command for either gamescope or gamescope-kbm based on config
pub fn create_command(cfg: &PartyConfig) -> Command {
    let gamescope = match cfg.kbm_support {
        true => BIN_GSC_KBM.as_path(),
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

/// Add keyboard/mouse device arguments for gamescope-kbm
///
/// This configures gamescope-kbm's libinput-hold-dev to capture specific input devices
pub fn add_kbm_args(
    cmd: &mut Command,
    instance: &Instance,
    input_devices: &[DeviceInfo],
    cfg: &PartyConfig,
) {
    if !cfg.kbm_support {
        return;
    }

    let mut instance_has_keyboard = false;
    let mut instance_has_mouse = false;
    let mut kbm_devices = String::new();

    for &d in &instance.devices {
        let dev = &input_devices[d];
        if dev.device_type == DeviceType::Keyboard {
            instance_has_keyboard = true;
            kbm_devices.push_str(&format!("{},", &dev.path));
        } else if dev.device_type == DeviceType::Mouse {
            instance_has_mouse = true;
            kbm_devices.push_str(&format!("{},", &dev.path));
        }
    }

    if instance_has_keyboard {
        cmd.arg("--backend-disable-keyboard");
    }
    if instance_has_mouse {
        cmd.arg("--backend-disable-mouse");
    }
    if !kbm_devices.is_empty() {
        cmd.arg(format!(
            "--libinput-hold-dev={}",
            kbm_devices.trim_end_matches(',')
        ));
    }
}

/// Add the separator between gamescope args and the inner command
pub fn add_separator(cmd: &mut Command) {
    cmd.arg("--");
}
