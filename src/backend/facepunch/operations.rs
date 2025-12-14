//! Facepunch operations - atomic side effects
//!
//! Each function performs a single I/O operation.

mod bepinex;
mod env;
mod overlay;

// Note: Re-exports commented out until migration complete (see Phase 9.5)
// pub use env::get_linux_bepinex_env;
// pub use overlay::create_instance_overlay;
