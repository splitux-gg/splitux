//! Bubblewrap container setup
//!
//! This module handles configuring bubblewrap (bwrap) for process isolation,
//! including input device blocking and SDL environment setup.
//!
//! Structure:
//! - `pure/matching.rs` — stateless device matching and arg building
//! - `operations/base.rs` — base container args, SDL/audio/BepInEx env setup
//! - `operations/devices.rs` — device discovery (js, hidraw, evdev)
//! - `operations/blocking.rs` — device blocking with permission checks

mod operations;
mod pure;

// Re-export all public functions to maintain the existing API
pub use operations::base::{add_base_args, setup_audio_env, setup_bepinex_env, setup_sdl_env};
pub use operations::blocking::{get_evdev_hidraw_blocking_args, get_js_blocking_args};
pub use operations::devices::{
    get_assigned_gamepad_paths, glob_js_devices, log_assigned_devices,
};
