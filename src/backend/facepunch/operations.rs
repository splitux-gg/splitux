//! Facepunch operations - atomic side effects
//!
//! Each function performs a single I/O operation.

mod bepinex;
mod env;
mod overlay;

// Internal re-exports
pub use overlay::create_instance_overlay;

// External re-exports (Phase 9.5 migration)
pub use env::get_linux_bepinex_env;
