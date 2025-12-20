//! Mod/plugin source types

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Source specification for a plugin/mod
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct PluginSource {
    /// Source type: "thunderstore", "github", "url"
    #[serde(default)]
    pub source: String,

    /// Community/game identifier (for Thunderstore)
    #[serde(default)]
    pub community: String,

    /// Package identifier: "Owner/PackageName"
    #[serde(default)]
    pub package: String,

    /// Version string (e.g., "1.4.0")
    #[serde(default)]
    pub version: String,
}

impl PluginSource {
    /// Check if this source is empty/unconfigured
    pub fn is_empty(&self) -> bool {
        self.source.is_empty() && self.package.is_empty()
    }

    /// Get the cache directory path for this plugin
    pub fn cache_path(&self, base_cache: &Path) -> PathBuf {
        base_cache
            .join(&self.source)
            .join(&self.community)
            .join(self.package.replace('/', "_"))
            .join(&self.version)
    }

    /// Build the download URL for Thunderstore packages
    pub fn thunderstore_url(&self) -> String {
        format!(
            "https://thunderstore.io/package/download/{}/{}/",
            self.package, self.version
        )
    }

    /// Human-readable display name
    pub fn display_name(&self) -> String {
        format!("{}@{}", self.package, self.version)
    }
}

/// Filter a list of paths to only DLL files
pub fn filter_dll_files(files: &[PathBuf]) -> Vec<&PathBuf> {
    files
        .iter()
        .filter(|f| {
            f.extension()
                .map(|e| e.eq_ignore_ascii_case("dll"))
                .unwrap_or(false)
        })
        .collect()
}
