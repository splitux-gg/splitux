//! Standalone backend - BepInEx + plugins without network emulation
//!
//! For games where mods handle their own multiplayer networking.
//! Examples: DSP with Nebula, games with custom networking mods.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use super::Backend;
use crate::bepinex::install_plugin_dlls;
use crate::handler::Handler;
use crate::instance::Instance;
use crate::mods::{self, filter_plugin_files, PluginSource};
use crate::paths::PATH_PARTY;

/// Standalone backend settings
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct StandaloneSettings {
    /// Thunderstore community for BepInEx download
    #[serde(default)]
    pub community: String,

    /// BepInEx package to use (e.g., "xiaoye97/BepInEx" for DSP)
    /// Defaults to "bbepis/BepInExPack" if not specified
    #[serde(default)]
    pub bepinex_package: String,

    /// Plugins to install
    #[serde(default)]
    pub plugins: Vec<PluginSource>,
}

impl StandaloneSettings {
    /// Check if this settings is empty/unconfigured
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.community.is_empty() && self.bepinex_package.is_empty() && self.plugins.is_empty()
    }

    /// Get the BepInEx package to use, defaulting to bbepis/BepInExPack
    pub fn get_bepinex_package(&self) -> &str {
        if self.bepinex_package.is_empty() {
            "bbepis/BepInExPack"
        } else {
            &self.bepinex_package
        }
    }
}

/// Standalone backend implementation
pub struct Standalone {
    settings: StandaloneSettings,
}

impl Standalone {
    pub fn new(settings: StandaloneSettings) -> Self {
        Self { settings }
    }
}

impl Backend for Standalone {
    fn name(&self) -> &str {
        "standalone"
    }

    fn requires_overlay(&self) -> bool {
        true
    }

    fn create_all_overlays(
        &self,
        _handler: &Handler,
        instances: &[Instance],
        _is_windows: bool,
        _game_dir: &Path,
    ) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        // Fetch BepInEx pack from Thunderstore
        let bepinex_dir = fetch_bepinex_pack(
            &self.settings.community,
            self.settings.get_bepinex_package(),
        )?;

        // Fetch all plugins and collect DLL paths
        let plugin_dlls = fetch_all_plugins(&self.settings.plugins, &self.settings.community)?;

        let mut overlays = Vec::new();

        for i in 0..instances.len() {
            let overlay_dir = PATH_PARTY
                .join("tmp")
                .join(format!("standalone-{}", i));

            // Clean previous overlay
            if overlay_dir.exists() {
                fs::remove_dir_all(&overlay_dir)?;
            }
            fs::create_dir_all(&overlay_dir)?;

            // Install BepInEx from Thunderstore package
            // This includes winhttp.dll and doorstop_config.ini from the package
            install_bepinex_from_package(&overlay_dir, &bepinex_dir)?;

            // Install plugin DLLs
            install_plugin_dlls(&overlay_dir, &plugin_dlls)?;

            println!(
                "[splitux] Standalone overlay {} created: {} plugins",
                i,
                plugin_dlls.len()
            );

            overlays.push(overlay_dir);
        }

        Ok(overlays)
    }
}

/// Fetch BepInEx pack from Thunderstore for a community
fn fetch_bepinex_pack(community: &str, package: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cache_base = mods::cache_base();
    mods::fetch_bepinex_pack(community, package, &cache_base)
}

/// Fetch all plugins and return their file paths (DLLs + asset bundles)
fn fetch_all_plugins(
    plugins: &[PluginSource],
    default_community: &str,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let cache_base = mods::cache_base();
    let mut all_files = Vec::new();

    for plugin in plugins {
        if plugin.is_empty() {
            continue;
        }
        // Resolve plugin with defaults (community inheritance, latest version, etc.)
        let resolved = plugin.resolve(default_community, &cache_base)?;
        let files = mods::fetch_plugin(&resolved, &cache_base)?;
        let plugin_files: Vec<PathBuf> = filter_plugin_files(&files)
            .into_iter()
            .cloned()
            .collect();
        all_files.extend(plugin_files);
    }

    Ok(all_files)
}

/// Install BepInEx from a Thunderstore package to overlay
fn install_bepinex_from_package(
    overlay_dir: &Path,
    bepinex_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Different BepInEx packages have different structures:
    //
    // bbepis/BepInExPack:
    // - BepInExPack/
    //   - BepInEx/
    //   - winhttp.dll
    //   - doorstop_config.ini
    //
    // xiaoye97/BepInEx (DSP-specific, older doorstop):
    // - BepInEx/
    // - winhttp.dll
    // - doorstop_config.ini
    //
    // We need to find the root that contains winhttp.dll and BepInEx folder.

    // Try to find the package root (directory containing winhttp.dll and BepInEx)
    let pack_root = find_bepinex_root(bepinex_dir)?;

    let bepinex_src = pack_root.join("BepInEx");
    if !bepinex_src.exists() {
        return Err(format!(
            "BepInEx folder not found in Thunderstore package: {:?}",
            bepinex_dir
        )
        .into());
    }

    // Copy BepInEx folder
    let bepinex_dest = overlay_dir.join("BepInEx");
    copy_dir_recursive(&bepinex_src, &bepinex_dest)?;

    // Copy winhttp.dll (doorstop loader for Windows/Proton)
    let winhttp_src = pack_root.join("winhttp.dll");
    if winhttp_src.exists() {
        fs::copy(&winhttp_src, overlay_dir.join("winhttp.dll"))?;
        eprintln!("[standalone] Installed winhttp.dll (doorstop loader)");
    }

    // Copy doorstop_config.ini
    let config_src = pack_root.join("doorstop_config.ini");
    if config_src.exists() {
        fs::copy(&config_src, overlay_dir.join("doorstop_config.ini"))?;
        eprintln!("[standalone] Installed doorstop_config.ini");
    }

    eprintln!(
        "[standalone] Installed BepInEx from Thunderstore package: {:?}",
        bepinex_src
    );

    Ok(())
}

/// Find the root directory of a BepInEx package (the one containing winhttp.dll and BepInEx/)
fn find_bepinex_root(base_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Check if base_dir itself is the root
    if base_dir.join("winhttp.dll").exists() && base_dir.join("BepInEx").exists() {
        return Ok(base_dir.to_path_buf());
    }

    // Search one level deep for common wrapper directories
    if let Ok(entries) = fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.join("winhttp.dll").exists() && path.join("BepInEx").exists() {
                    return Ok(path);
                }
            }
        }
    }

    // Fallback: check for BepInEx folder even without winhttp.dll
    // (some packages might have doorstop files elsewhere)
    if base_dir.join("BepInEx").exists() {
        return Ok(base_dir.to_path_buf());
    }

    // Search one level deep for BepInEx folder
    if let Ok(entries) = fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("BepInEx").exists() {
                return Ok(path);
            }
        }
    }

    Err(format!(
        "Could not find BepInEx package root in: {:?}",
        base_dir
    )
    .into())
}

/// Copy directory recursively
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }

    Ok(())
}
