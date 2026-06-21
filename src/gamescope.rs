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

    // SDL backend (cfg.gamescope_sdl_backend, default on). gamescope owns an SDL
    // window and takes input through SDL/XWayland rather than running as a
    // headless wayland client. This is REQUIRED for reliable mouse/keyboard under
    // remote streaming (Sunshine/Moonlight) on Wayland/niri: the default wayland
    // backend doesn't cleanly take the host's injected input, so menu clicks die.
    // It matches what Lutris uses. Previously gated to X11 only ("WM handles
    // positioning"), which silently disabled the flag on Wayland.
    if cfg.gamescope_sdl_backend {
        cmd.arg("--backend=sdl");
        if is_wayland_session() {
            // SDL needs a real video driver; use XWayland (x11). setup_env removed
            // SDL_VIDEODRIVER for the wayland backend, and runs before add_args, so
            // this re-set wins. The niri WM still positions the SDL toplevel.
            cmd.env("SDL_VIDEODRIVER", "x11");
        } else {
            cmd.arg(format!("--display-index={}", instance.monitor));
        }
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

/// Drive a together instance's nested compositor at the fps tier.
///
/// Set ONCE per together instance (independent of how many seats it carries):
/// the PipeWire capture is clocked at the compositor's refresh, and the headless
/// backend otherwise defaults to 60Hz — capping the producer at one frame per
/// vblank. `resolved_fps()` is the same source the seat-streamer `--fps` uses,
/// so capture and encode rates always agree. Local splitscreen (no together
/// seats) keeps the monitor's native refresh.
pub fn add_together_refresh_rate(cmd: &mut Command, cfg: &SplituxConfig) {
    cmd.args(["-r", &cfg.together.resolved_fps().to_string()]);
}

/// Hold a splitux-together remote seat's virtual keyboard + mouse.
///
/// These are ALWAYS held (independent of the seat's Gamepad/Kb+Mouse type) so
/// that input the remote browser sends over those devices reaches the game's
/// gamescope instead of leaking to the host desktop. Requires gamescope-splitux
/// (input holding), which is also the PipeWire capture source the seat streams.
/// Call once per seat — gamescope accepts repeated `--libinput-hold-dev`, so a
/// local-split instance can hold every one of its seats' kbd/mice.
pub fn add_seat_hold_args(
    cmd: &mut Command,
    seat: &crate::together::TogetherSeatDevices,
    cfg: &SplituxConfig,
) {
    if !cfg.input_holding {
        return;
    }
    for dev in [seat.kbd.as_ref(), seat.mouse.as_ref(), seat.ptr.as_ref()].into_iter().flatten() {
        cmd.arg(format!("--libinput-hold-dev={}", dev.display()));
    }
}

/// Keep parent-compositor input flowing even while seat devices are held.
///
/// Holding a seat's virtual kbd/mouse (`--libinput-hold-dev`) switches
/// gamescope-splitux to libinput-only input and blocks ALL parent-compositor
/// (host desktop) input — correct for a pure-remote seat, but it locks out a
/// LOCAL host sharing the same collapsed local-split instance. This flag tells
/// gamescope to keep the held devices AND still accept the focused window's
/// kb/m, so the host plays alongside the remote seat(s). No-op without
/// input_holding (the flag only exists in gamescope-splitux).
pub fn add_libinput_allow_parent(cmd: &mut Command, cfg: &SplituxConfig) {
    if !cfg.input_holding {
        return;
    }
    cmd.arg("--libinput-allow-parent");
}

/// Add the separator between gamescope args and the inner command
pub fn add_separator(cmd: &mut Command) {
    cmd.arg("--");
}
