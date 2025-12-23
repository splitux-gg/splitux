//! Shared BepInEx module - backend-agnostic plugin loader infrastructure
//!
//! Provides BepInEx installation and configuration for Unity games.
//! Used by: Photon, Facepunch, Goldberg (with plugins)

mod operations;
mod types;

pub use operations::{
    bepinex_backend_available, install_bepinex_core, install_doorstop, install_plugin_dlls,
    write_doorstop_config,
};
pub use types::UnityBackend;
