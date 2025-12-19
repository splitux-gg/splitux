//! Photon operations - atomic side effects
//!
//! Each function performs a single I/O operation.

mod bepinex;
mod config;
mod overlay;
mod symlinks;

pub use bepinex::bepinex_backend_available;
pub use config::generate_instance_config;
pub use overlay::create_instance_overlay;
pub use symlinks::setup_shared_files;
