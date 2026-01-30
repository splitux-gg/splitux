// Base bwrap container setup operations

use std::process::Command;

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

/// Set up BepInEx environment variables for Linux native games
///
/// These are passed via --setenv so they apply inside the container
pub fn setup_bepinex_env(cmd: &mut Command, env_vars: &std::collections::HashMap<String, String>) {
    for (key, value) in env_vars {
        cmd.args(["--setenv", key, value]);
    }
}
