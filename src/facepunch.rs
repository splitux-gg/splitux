//! Facepunch.Steamworks integration for Unity games using Facepunch Steam API
//!
//! This module handles:
//! - Installing BepInEx with SplituxFacepunch plugin
//! - Generating per-instance splitux.cfg files
//! - Creating overlay directories for each instance
//!
//! The SplituxFacepunch plugin reads the config and applies:
//! - Facepunch library patches (spoof_identity, force_valid, photon_bypass)
//! - Runtime patches for game-specific classes

use std::fs;
use std::path::{Path, PathBuf};

use crate::handler::{FacepunchSettings, Handler, RuntimePatch};
use crate::instance::Instance;
use crate::paths::{PATH_PARTY, PATH_RES};
use crate::photon::{detect_unity_backend, bepinex_backend_available, UnityBackend};
use crate::profiles::generate_steam_id;

/// Configuration for a Facepunch instance
#[derive(Debug, Clone)]
pub struct FacepunchConfig {
    /// Instance index (0-based)
    pub player_index: usize,
    /// Player name for this instance
    pub account_name: String,
    /// Spoofed Steam ID
    pub steam_id: u64,
    /// Facepunch settings from handler
    pub settings: FacepunchSettings,
    /// Runtime patches from handler
    pub runtime_patches: Vec<RuntimePatch>,
}

/// Generate the splitux.cfg content for an instance
/// Uses flat format (no sections) to match the working bepinex-test config
fn generate_config_content(config: &FacepunchConfig) -> String {
    let mut content = String::new();

    // Flat format matching bepinex-test
    content.push_str(&format!("player_index={}\n", config.player_index));
    content.push_str(&format!("account_name={}\n", config.account_name));
    content.push_str(&format!("steam_id={}\n", config.steam_id));

    content
}

/// Create a Facepunch overlay for a single instance
///
/// This creates an overlay with:
/// 1. BepInEx core files (Mono backend)
/// 2. Doorstop loader (winhttp.dll for Windows, libdoorstop.so for Linux)
/// 3. BepInEx/config/splitux.cfg with instance-specific settings
///
/// The SplituxFacepunch.dll should be in handler's overlay/BepInEx/plugins/
fn create_instance_overlay(
    instance_idx: usize,
    config: &FacepunchConfig,
    is_windows: bool,
    backend: UnityBackend,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let overlay_dir = PATH_PARTY
        .join("tmp")
        .join(format!("facepunch-overlay-{}", instance_idx));

    // Clean previous overlay
    if overlay_dir.exists() {
        fs::remove_dir_all(&overlay_dir)?;
    }
    fs::create_dir_all(&overlay_dir)?;

    // Choose BepInEx variant based on platform and Unity backend
    let bepinex_subdir = match (is_windows, backend) {
        (true, UnityBackend::Mono) => "mono",
        (false, UnityBackend::Mono) => "mono-linux",
        (_, UnityBackend::Il2Cpp) => "il2cpp",
    };
    let bepinex_res = PATH_RES.join("bepinex").join(bepinex_subdir);

    if !bepinex_res.exists() {
        return Err(format!(
            "BepInEx resources not found for {} backend. Please run ./splitux.sh build",
            bepinex_subdir
        ).into());
    }

    // 1. Copy BepInEx core
    let bepinex_core_src = bepinex_res.join("core");
    let bepinex_core_dest = overlay_dir.join("BepInEx").join("core");
    if bepinex_core_src.exists() {
        copy_dir_recursive(&bepinex_core_src, &bepinex_core_dest)?;
    }

    // 2. Copy doorstop loader (platform-specific)
    if is_windows {
        let winhttp_src = bepinex_res.join("winhttp.dll");
        if winhttp_src.exists() {
            fs::copy(&winhttp_src, overlay_dir.join("winhttp.dll"))?;
        }
    } else {
        // Linux native: copy libdoorstop.so
        let libdoorstop_src = bepinex_res.join("libdoorstop.so");
        if libdoorstop_src.exists() {
            fs::copy(&libdoorstop_src, overlay_dir.join("libdoorstop.so"))?;
        }
    }

    // 3. Write doorstop config
    write_doorstop_config(&overlay_dir, is_windows, backend)?;

    // 4. Create BepInEx config directory and write configs
    let bepinex_config_dir = overlay_dir.join("BepInEx").join("config");
    fs::create_dir_all(&bepinex_config_dir)?;

    // Write splitux.cfg with player identity
    let config_content = generate_config_content(config);
    fs::write(bepinex_config_dir.join("splitux.cfg"), &config_content)?;

    // Write BepInEx.cfg to disable console logging (prevents CStreamWriter crash on Linux)
    // This is critical for native Linux games - BepInEx's LinuxConsoleDriver crashes
    // with CStreamWriter if console logging is enabled
    let bepinex_cfg = r#"[Logging.Console]
Enabled = false

[Logging.Disk]
Enabled = true
LogLevels = All

[Chainloader]
HideManagerGameObject = true
"#;
    fs::write(bepinex_config_dir.join("BepInEx.cfg"), bepinex_cfg)?;

    // 5. Copy SplituxFacepunch plugin DLL
    let plugins_dir = overlay_dir.join("BepInEx").join("plugins");
    fs::create_dir_all(&plugins_dir)?;

    let plugin_src = PATH_RES.join("facepunch").join("SplituxFacepunch.dll");
    if plugin_src.exists() {
        fs::copy(&plugin_src, plugins_dir.join("SplituxFacepunch.dll"))?;
    } else {
        println!(
            "[splitux] Warning: SplituxFacepunch.dll not found at {}",
            plugin_src.display()
        );
        println!("[splitux] Run ./splitux.sh build to download it");
    }

    let backend_name = match backend {
        UnityBackend::Mono => "Mono",
        UnityBackend::Il2Cpp => "IL2CPP",
    };
    println!(
        "[splitux] Facepunch overlay {} created: Player {}, SteamID {}, Backend: {}",
        instance_idx, config.account_name, config.steam_id, backend_name
    );

    Ok(overlay_dir)
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

/// Write BepInEx doorstop configuration
fn write_doorstop_config(overlay_dir: &Path, is_windows: bool, backend: UnityBackend) -> Result<(), Box<dyn std::error::Error>> {
    let preloader_dll = match backend {
        UnityBackend::Mono => "BepInEx.Preloader.dll",
        UnityBackend::Il2Cpp => "BepInEx.Preloader.dll",
    };

    let config = if is_windows {
        format!(
            "[General]\nenabled=true\ntarget_assembly=BepInEx\\core\\{}\n",
            preloader_dll
        )
    } else {
        format!(
            "[General]\nenabled=true\ntarget_assembly=BepInEx/core/{}\n",
            preloader_dll
        )
    };

    fs::write(overlay_dir.join("doorstop_config.ini"), config)?;
    Ok(())
}

/// Create Facepunch overlays for all instances
pub fn create_all_overlays(
    handler: &Handler,
    instances: &[Instance],
    is_windows: bool,
    game_dir: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    // Detect Unity backend type
    let backend = detect_unity_backend(game_dir);

    // Check if BepInEx resources exist for this backend
    if !bepinex_backend_available(backend) {
        let backend_name = match backend {
            UnityBackend::Mono => "Mono",
            UnityBackend::Il2Cpp => "IL2CPP",
        };
        return Err(format!(
            "BepInEx resources not found for {} backend. Run ./splitux.sh build",
            backend_name
        ).into());
    }

    // Get Facepunch settings from new optional field (Phase 7)
    let facepunch_settings = handler.facepunch_ref()
        .ok_or("Facepunch backend not enabled")?;

    let mut overlays = Vec::new();

    for (i, instance) in instances.iter().enumerate() {
        let config = FacepunchConfig {
            player_index: i,
            account_name: instance.profname.clone(),
            steam_id: generate_steam_id(&instance.profname),
            settings: FacepunchSettings {
                spoof_identity: facepunch_settings.spoof_identity,
                force_valid: facepunch_settings.force_valid,
                photon_bypass: facepunch_settings.photon_bypass,
            },
            runtime_patches: handler.runtime_patches.clone(),
        };

        let overlay = create_instance_overlay(i, &config, is_windows, backend)?;
        overlays.push(overlay);
    }

    Ok(overlays)
}

/// Check if handler uses Facepunch backend
/// Uses new optional field (Phase 7)
pub fn uses_facepunch(handler: &Handler) -> bool {
    handler.has_facepunch()
}

/// Get environment variables needed for BepInEx on Linux native games
/// Returns empty HashMap for Windows games (doorstop uses winhttp.dll injection)
pub fn get_linux_bepinex_env(game_dir: &Path) -> std::collections::HashMap<String, String> {
    let mut env = std::collections::HashMap::new();

    let libdoorstop = game_dir.join("libdoorstop.so");
    let preloader = game_dir.join("BepInEx/core/BepInEx.Preloader.dll");

    if libdoorstop.exists() {
        // BepInEx doorstop environment variables (from run_bepinex.sh)
        env.insert("DOORSTOP_ENABLED".to_string(), "1".to_string());
        env.insert("DOORSTOP_TARGET_ASSEMBLY".to_string(), preloader.to_string_lossy().to_string());

        // LD_LIBRARY_PATH must include the game directory for libdoorstop.so to be found
        env.insert("LD_LIBRARY_PATH".to_string(), game_dir.to_string_lossy().to_string());

        // LD_PRELOAD with full path to libdoorstop.so
        env.insert("LD_PRELOAD".to_string(), libdoorstop.to_string_lossy().to_string());
    }

    env
}
