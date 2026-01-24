//! Mod/plugin source types

use serde::{Deserialize, Deserializer, Serialize};
use std::error::Error;
use std::path::{Path, PathBuf};

/// Source specification for a plugin/mod
///
/// Supports multiple YAML formats:
/// ```yaml
/// # Simple string (package only, uses latest version)
/// plugins:
///   - gabrielgad/NebulaMultiplayerMod
///
/// # Object with optional fields
/// plugins:
///   - package: gabrielgad/NebulaMultiplayerMod
///     version: "0.9.19"  # Optional, fetches latest if omitted
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct PluginSource {
    /// Source type: "thunderstore", "github", "url"
    /// Defaults to "thunderstore" if not specified
    #[serde(default)]
    pub source: String,

    /// Community/game identifier (for Thunderstore)
    /// Inherits from standalone.community if not specified
    #[serde(default)]
    pub community: String,

    /// Package identifier: "Owner/PackageName"
    #[serde(default)]
    pub package: String,

    /// Version string (e.g., "1.4.0", or empty for latest)
    #[serde(default)]
    pub version: String,
}

impl<'de> Deserialize<'de> for PluginSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct PluginSourceVisitor;

        impl<'de> Visitor<'de> for PluginSourceVisitor {
            type Value = PluginSource;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string like 'Owner/Package' or an object with 'package' field")
            }

            // Handle simple string format: "gabrielgad/NebulaMultiplayerMod"
            fn visit_str<E>(self, value: &str) -> Result<PluginSource, E>
            where
                E: de::Error,
            {
                Ok(PluginSource {
                    source: String::new(),
                    community: String::new(),
                    package: value.to_string(),
                    version: String::new(),
                })
            }

            // Handle object format: { package: "...", version: "..." }
            fn visit_map<M>(self, mut map: M) -> Result<PluginSource, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut source = None;
                let mut community = None;
                let mut package = None;
                let mut version = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "source" => source = Some(map.next_value()?),
                        "community" => community = Some(map.next_value()?),
                        "package" => package = Some(map.next_value()?),
                        "version" => version = Some(map.next_value()?),
                        _ => {
                            // Skip unknown fields
                            let _ = map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }

                Ok(PluginSource {
                    source: source.unwrap_or_default(),
                    community: community.unwrap_or_default(),
                    package: package.unwrap_or_default(),
                    version: version.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_any(PluginSourceVisitor)
    }
}

impl PluginSource {
    /// Check if this source is empty/unconfigured
    pub fn is_empty(&self) -> bool {
        self.source.is_empty() && self.package.is_empty()
    }

    /// Resolve a plugin source with defaults and latest version lookup
    ///
    /// - Inherits community from default_community if not specified
    /// - Defaults source to "thunderstore" if not specified
    /// - Fetches latest version from API if version is empty
    pub fn resolve(&self, default_community: &str, _cache_base: &Path) -> Result<Self, Box<dyn Error>> {
        let source = if self.source.is_empty() {
            "thunderstore".to_string()
        } else {
            self.source.clone()
        };

        let community = if self.community.is_empty() {
            default_community.to_string()
        } else {
            self.community.clone()
        };

        let version = if self.version.is_empty() {
            // Fetch latest version from Thunderstore API
            fetch_latest_version(&community, &self.package)?
        } else {
            self.version.clone()
        };

        Ok(Self {
            source,
            community,
            package: self.package.clone(),
            version,
        })
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
        if self.version.is_empty() {
            self.package.clone()
        } else {
            format!("{}@{}", self.package, self.version)
        }
    }
}

/// Fetch the latest version of a package from Thunderstore API
fn fetch_latest_version(_community: &str, package: &str) -> Result<String, Box<dyn Error>> {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct PackageVersion {
        version_number: String,
    }

    #[derive(Debug, Deserialize)]
    struct PackageInfo {
        latest: Option<PackageVersion>,
    }

    let (namespace, name) = package
        .split_once('/')
        .ok_or_else(|| format!("Invalid package format: {}", package))?;

    let url = format!(
        "https://thunderstore.io/api/experimental/package/{}/{}/",
        namespace, name
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client.get(&url).send()?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch package info: HTTP {}", response.status()).into());
    }

    let info: PackageInfo = response.json()?;

    info.latest
        .map(|v| v.version_number)
        .ok_or_else(|| format!("No latest version found for {}", package).into())
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

/// Filter a list of paths to BepInEx plugin files (DLLs + asset bundles + other necessary files)
/// Excludes Thunderstore metadata files (manifest.json, icon.png, README.md, etc.)
pub fn filter_plugin_files(files: &[PathBuf]) -> Vec<&PathBuf> {
    let excluded_files = [
        "manifest.json",
        "icon.png",
        "readme.md",
        "changelog.md",
        "license",
        "license.md",
        "license.txt",
    ];

    files
        .iter()
        .filter(|f| {
            // Get filename for exclusion check
            let filename = f
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();

            // Exclude Thunderstore metadata files
            if excluded_files.iter().any(|&ex| filename == ex) {
                return false;
            }

            // Include DLLs
            if f.extension()
                .map(|e| e.eq_ignore_ascii_case("dll"))
                .unwrap_or(false)
            {
                return true;
            }

            // Include PDB files (debug symbols)
            if f.extension()
                .map(|e| e.eq_ignore_ascii_case("pdb"))
                .unwrap_or(false)
            {
                return true;
            }

            // Include Unity asset bundles (no extension or common bundle extensions)
            // Files like "nebulabundle", "assetbundle", etc.
            if f.extension().is_none() && f.is_file() {
                // Exclude directories and hidden files
                if !filename.starts_with('.') {
                    return true;
                }
            }

            // Include common asset extensions
            if let Some(ext) = f.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if matches!(
                    ext_lower.as_str(),
                    "bundle" | "assets" | "so" | "dylib" | "lib"
                ) {
                    return true;
                }
            }

            false
        })
        .collect()
}
