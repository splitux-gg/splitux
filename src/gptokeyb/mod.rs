//! gptokeyb integration module
//!
//! Provides controller→keyboard/mouse translation for games without native
//! controller support. Uses gptokeyb (fork) to read gamepad input and emit
//! virtual keyboard/mouse events via Linux uinput.
//!
//! Architecture:
//! - gptokeyb runs as a daemon alongside each game instance
//! - Reads the real controller device (evdev)
//! - Creates virtual keyboard/mouse via uinput
//! - Game inside bwrap sees only the virtual devices
//!
//! Usage in handler.yaml:
//! ```yaml
//! gptokeyb:
//!   profile: fps         # Built-in: fps, mouse_only, racing
//!   mouse_scale: 512     # Optional: cursor speed
//! ```

mod operations;
mod types;

pub use operations::{is_available, spawn_all_daemons, terminate_all};
pub use types::GptokeybSettings;
