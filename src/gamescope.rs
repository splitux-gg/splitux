//! Gamescope nested compositor setup
//!
//! This module handles building gamescope commands with the correct arguments
//! for resolution, display, and input handling.

use std::path::Path;
use std::process::Command;

use crate::app::SplituxConfig;
use crate::instance::Instance;
use crate::monitor::Monitor;
use crate::paths::BIN_GSC_SPLITUX;
use crate::util::is_wayland_session;

/// Create the base gamescope command
///
/// Returns a Command for either gamescope or gamescope-splitux based on config
pub fn create_command(cfg: &SplituxConfig) -> Command {
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

    // Session-aware SDL backend selection for gamescope
    // Parent process sets SDL_VIDEODRIVER=x11 for its own use, so we must
    // explicitly override for child processes
    if is_wayland_session() {
        // Wayland: use native Wayland SDL (remove inherited x11 setting)
        cmd.env_remove("SDL_VIDEODRIVER");
    } else {
        // X11: use X11 SDL
        cmd.env("SDL_VIDEODRIVER", "x11");
    }

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
pub fn add_args(cmd: &mut Command, instance: &Instance, _monitors: &[Monitor], cfg: &SplituxConfig) {
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

    // X11 session: use SDL backend with display-index (legacy path)
    // Wayland session: skip, WM handles positioning
    if !is_wayland_session() && cfg.gamescope_sdl_backend {
        cmd.arg("--backend=sdl");
        cmd.arg(format!("--display-index={}", instance.monitor));
    }
}

/// Add input device holding arguments for gamescope-splitux
///
/// When a virtual device path is provided (from gptokeyb), gamescope will
/// read exclusively from that device for keyboard/mouse input.
pub fn add_input_holding_args(
    cmd: &mut Command,
    virtual_device: Option<&Path>,
    cfg: &SplituxConfig,
) {
    if !cfg.input_holding {
        return;
    }

    if let Some(vdev) = virtual_device {
        cmd.arg(format!("--libinput-hold-dev={}", vdev.display()));
    }
}

/// Add the separator between gamescope args and the inner command
pub fn add_separator(cmd: &mut Command) {
    cmd.arg("--");
}
