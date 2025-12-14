//! Handler module - game configuration and metadata
//!
//! This module provides:
//! - `Handler` struct for game configuration
//! - Type definitions for mods, settings, and patches
//! - I/O operations for loading, saving, and scanning handlers
//!
//! ## Module Structure
//! - `types.rs`: RequiredMod, SDL2Override, etc. (internal, for future migration)
//! - `io.rs`: scan_handlers, import_handler, handlers_dir

mod io;
mod types;

// Re-export I/O functions from io.rs
// These are new implementations that work with the legacy Handler
pub use io::{handlers_dir, import_handler, scan_handlers};

// Re-export everything from handler_legacy for backward compatibility
// All existing code using crate::handler::* continues to work unchanged
pub use crate::handler_legacy::*;
