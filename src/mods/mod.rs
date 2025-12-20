//! Mod/plugin management module
//!
//! Provides utilities for fetching and caching plugins from external sources
//! like Thunderstore. Used by backends to fetch their required dependencies.

mod types;
mod thunderstore;

pub use types::{filter_dll_files, PluginSource};
pub use thunderstore::fetch_plugin;

use crate::paths::PATH_PARTY;
use std::path::PathBuf;

/// Get the base cache directory for mods
pub fn cache_base() -> PathBuf {
    PATH_PARTY.join(".cache").join("mods")
}
