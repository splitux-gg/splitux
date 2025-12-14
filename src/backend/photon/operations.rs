//! Photon operations - atomic side effects
//!
//! Each function performs a single I/O operation.

mod bepinex;
mod config;
mod overlay;
mod symlinks;

pub use bepinex::{
    bepinex_available, bepinex_backend_available, get_bepinex_res_path,
};
pub use config::{generate_instance_config, PhotonAppIds};
pub use overlay::create_instance_overlay;
pub use symlinks::setup_shared_files;
