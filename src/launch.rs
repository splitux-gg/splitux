//! Launch module - game launching and orchestration
//!
//! This module provides:
//! - Profile setup for game instances
//! - Overlay mounting for game files
//! - Command building for gamescope + bwrap
//! - Game execution with window manager integration
//!
//! ## Module Structure
//! - `types.rs`: Constants and type definitions
//! - `pure/`: Pure functions (validation)
//! - `operations/`: Atomic side effects (profiles, overlays)
//! - `pipelines/`: High-level orchestration (build_cmds, execute)

mod operations;
mod pipelines;
mod pure;
mod types;

// Re-export public API
pub use operations::{fuse_overlayfs_mount_gamedirs, setup_profiles};
pub use pipelines::{launch_cmds, launch_game, print_launch_cmds};
pub use pure::{validate_executable, validate_runtime};
pub use types::SDL_GAMECONTROLLER_IGNORE_DEVICES;

// Re-export from launch_legacy for backward compatibility during migration
pub use crate::launch_legacy::*;
