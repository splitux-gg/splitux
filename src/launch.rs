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
pub use operations::setup_profiles;
pub use pipelines::launch_game;
