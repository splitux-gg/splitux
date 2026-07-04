// Base bwrap container setup operations

use std::process::Command;

/// Add base bwrap arguments to command
///
/// Sets up the container with full filesystem access but isolated /tmp.
///
/// `as_program` selects how the command is rooted. In the normal path the
/// command's program is gamescope and bwrap is its child after `--`, so we emit
/// a literal `bwrap` argument (as_program = false). When gamescope is bypassed
/// (single local seat, `disable_gamescope`) the command is rooted directly at
/// bwrap — the program is already `bwrap`, so the leading argument is skipped
/// (as_program = true) to avoid `bwrap bwrap …`.
pub fn add_base_args(cmd: &mut Command, as_program: bool) {
    if !as_program {
        cmd.arg("bwrap");
    }
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

    // Set the specific gamepad device(s) for this instance. SDL2 parses
    // SDL_JOYSTICK_DEVICE as a COLON-separated list (PATH-style; see SDL's
    // LINUX_JoystickInit) — a comma-joined list is read as one bogus path.
    // SDL games masked this (they also enumerate udev on their own), but
    // Brotato's custom Godot treats this var as the authoritative pad list:
    // with the comma list it open()s the literal joined string, gets ENOENT,
    // and ends up with ZERO pads (2-seat local-split Brotato regression).
    if !gamepad_paths.is_empty() {
        cmd.args(["--setenv", "SDL_JOYSTICK_DEVICE", &gamepad_paths.join(":")]);
    }
}

/// Set up audio routing environment variables inside the bwrap container
///
/// Sets PULSE_SINK to route audio to a specific sink (works for both
/// PulseAudio and PipeWire via pipewire-pulse compatibility layer).
///
/// PULSE_SINK alone is NOT enough: `module-stream-restore` remembers per-stream
/// device AND volume keyed by the stream's identity (media.role, else app-name),
/// and a matching saved entry OVERRIDES PULSE_SINK. On a Sunshine host the saved
/// "role:game → sink-sunshine-stereo @ 0%" rule hijacks any game that declares
/// `media.role=game` (e.g. Enter the Gungeon): it lands on the wrong sink AND is
/// muted to 0%, so its together stream is silent. We sidestep stream-restore by
/// forcing a per-instance-unique `media.role` (PULSE_PROP_OVERRIDE wins even over
/// the app's own proplist): a fresh role has no saved entry, so PULSE_SINK and
/// the default full volume both apply. The role is unique per launch (the sink
/// name carries `<pid>_<n>`), so no stale entry can ever match it either.
pub fn setup_audio_env(cmd: &mut Command, sink_name: &str) {
    if sink_name.is_empty() {
        return;
    }
    // PULSE_SINK works for both PulseAudio and PipeWire (via pipewire-pulse)
    cmd.args(["--setenv", "PULSE_SINK", sink_name]);
    // Force a unique stream-restore identity so no pre-existing device/volume
    // rule can override the routing above. `media.<key>=<val>` proplist syntax.
    cmd.args(["--setenv", "PULSE_PROP_OVERRIDE", &format!("media.role={sink_name}")]);
}

/// Set up BepInEx environment variables for Linux native games
///
/// These are passed via --setenv so they apply inside the container
pub fn setup_bepinex_env(cmd: &mut Command, env_vars: &std::collections::HashMap<String, String>) {
    for (key, value) in env_vars {
        cmd.args(["--setenv", key, value]);
    }
}
