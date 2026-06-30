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

    // Cursor auto-hide (opt-in). Forcing this hid the cursor in any window that
    // never sees mouse motion (a second instance, or a pad-driven seat) and it
    // never came back — clicks still landed, but the pointer wasn't drawn.
    if cfg.gamescope_autohide_cursor {
        cmd.args(["--hide-cursor-delay", "1000"]);
    }

    // Pin the pointer inside the game (gamescope relative-mouse grab). ON BY
    // DEFAULT for every instance: `-f` fullscreen confines the WINDOW to the
    // output but does NOT lock the POINTER, so during mouse-look the cursor
    // drifts to the edge and slips onto another monitor. `--force-grab-cursor`
    // makes gamescope always grab the cursor so it can't escape; you switch
    // instances via WM bindings, not by the cursor leaving. Toggle off only for
    // point-and-click / touch games that need a free host cursor.
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

/// Fullscreen the hosted game (`-f`).
///
/// Makes gamescope own the whole output: the game fills it at the output
/// resolution (instead of a default ~720p floating window) AND the cursor is
/// confined to that output — moving the mouse to a screen edge can't escape onto
/// the host desktop or another monitor. This is the correct cursor-confinement
/// primitive (own the output), not an exclusive device grab. Gate it to single /
/// online-co-op games via the handler `fullscreen` flag — NOT local split-screen,
/// where each instance is a sub-region of one output.
pub fn add_fullscreen(cmd: &mut Command) {
    cmd.arg("-f");
}

/// Add input device holding arguments for gamescope-splitux
///
/// When a virtual device path is provided (from gptokeyb), gamescope will
/// read exclusively from that device for keyboard/mouse input.
///
/// NOTE: this deliberately does NOT hold the instance's REAL keyboard/mouse.
/// An exclusive grab of the real mouse (to "confine the cursor") took the device
/// away from the host without gamescope drawing a usable confined cursor — the
/// mouse simply locked up. A kb/mouse seat must keep a usable mouse, so only
/// gptokeyb's virtual output device is held.
pub fn add_input_holding_args(
    cmd: &mut Command,
    virtual_device: Option<&Path>,
    cfg: &SplituxConfig,
) {
    if !cfg.input_holding {
        return;
    }

    // gptokeyb's virtual kbd/mouse (controller→kb/m translation output).
    if let Some(vdev) = virtual_device {
        cmd.arg(format!("--libinput-hold-dev={}", vdev.display()));
    }
}

/// Cap the nested compositor's refresh (`-r`) at the configured fps tier — for
/// EVERY instance, local or together.
///
/// Applied to TOGETHER instances ONLY (gated at the call site on a non-empty seat
/// list). Their PipeWire capture is clocked at the compositor's refresh and the
/// headless backend otherwise defaults to 60Hz, so `-r` must match the stream tier
/// for capture/encode pacing. `resolved_fps()` is the same source the seat-streamer
/// `--fps` uses, so the capture and encode rates always agree.
///
/// LOCAL instances are deliberately NOT capped here: gamescope frame-limiting a
/// local seat below the panel's native refresh (e.g. 60 on a 200Hz display) strobes
/// black frames on motion — a gamescope-only present-pacing artifact (native
/// presentation is clean). Local seats render at the display's native refresh.
pub fn add_refresh_rate(cmd: &mut Command, cfg: &SplituxConfig) {
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
