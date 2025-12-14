//! Photon operations - atomic side effects
//!
//! Each function performs a single I/O operation.

mod bepinex;
mod config;
mod overlay;
mod symlinks;

pub use bepinex::{
    bepinex_available, bepinex_backend_available, copy_dir_recursive, get_bepinex_res_path,
    install_bepinex_core, install_doorstop, write_doorstop_config,
};
pub use config::{generate_instance_config, PhotonAppIds};
pub use overlay::create_instance_overlay;
pub use symlinks::setup_shared_files;
