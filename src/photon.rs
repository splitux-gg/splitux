//! Photon/BepInEx integration for Unity games using Photon networking
//!
//! This module handles:
//! - Installing BepInEx into game overlay
//! - Generating per-instance config files for LocalMultiplayer mod
//! - Auto-detecting Unity backend type (Mono vs IL2CPP)
//!
//! Note: The LocalMultiplayer mod itself should be placed in the handler's
//! overlay/BepInEx/plugins/ directory by the user.

use std::fs;
use std::path::{Path, PathBuf};

use crate::app::load_photon_ids;
use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::{PATH_PARTY, PATH_RES};

/// Unity scripting backend type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnityBackend {
    /// Mono backend (older games, has GAME_Data/Managed/*.dll)
    Mono,
    /// IL2CPP backend (newer games, has GameAssembly.dll)
    Il2Cpp,
}

/// Configuration for a Photon instance
#[derive(Debug, Clone)]
pub struct PhotonConfig {
    /// Instance index (0-based)
    #[allow(dead_code)] // Reserved for future per-instance config
    pub instance_idx: usize,
    /// Player name for this instance
    pub player_name: String,
    /// Listen port for local networking
    pub listen_port: u16,
    /// Ports of other instances for discovery
    #[allow(dead_code)] // Reserved for broadcast discovery
    pub broadcast_ports: Vec<u16>,
}

/// Detect Unity backend type from game directory
///
/// - IL2CPP games have `GameAssembly.dll` in the root
/// - Mono games have `GAME_Data/Managed/` directory with .dll files
pub fn detect_unity_backend(game_dir: &Path) -> UnityBackend {
    // Check for IL2CPP indicator
    if game_dir.join("GameAssembly.dll").exists() {
        println!("[splitux] Detected Unity IL2CPP backend");
        return UnityBackend::Il2Cpp;
    }

    // Check for Mono indicator - look for *_Data/Managed/ directory
    if let Ok(entries) = fs::read_dir(game_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.ends_with("_Data") {
                    let managed_dir = path.join("Managed");
                    if managed_dir.exists() && managed_dir.is_dir() {
                        println!("[splitux] Detected Unity Mono backend (found {}/Managed/)", name);
                        return UnityBackend::Mono;
                    }
                }
            }
        }
    }

    // Default to Mono (more common for indie games)
    println!("[splitux] Could not detect Unity backend, defaulting to Mono");
    UnityBackend::Mono
}

/// Check if BepInEx resources are available (either Mono or IL2CPP)
#[allow(dead_code)] // API for UI to check availability
pub fn bepinex_available() -> bool {
    let bepinex_path = PATH_RES.join("bepinex");
    let mono_exists = bepinex_path.join("mono").join("core").exists();
    let il2cpp_exists = bepinex_path.join("il2cpp").join("core").exists();
    mono_exists || il2cpp_exists
}

/// Check if specific BepInEx backend is available
pub fn bepinex_backend_available(backend: UnityBackend) -> bool {
    let bepinex_path = PATH_RES.join("bepinex");
    match backend {
        UnityBackend::Mono => bepinex_path.join("mono").join("core").exists(),
        UnityBackend::Il2Cpp => bepinex_path.join("il2cpp").join("core").exists(),
    }
}

/// Get the path to bundled BepInEx resources for a specific backend
fn get_bepinex_res_path(backend: UnityBackend) -> PathBuf {
    let subdir = match backend {
        UnityBackend::Mono => "mono",
        UnityBackend::Il2Cpp => "il2cpp",
    };
    PATH_RES.join("bepinex").join(subdir)
}

/// Create BepInEx overlay for a single instance
///
/// This creates an overlay with:
/// 1. BepInEx core files (from bundled resources, Mono or IL2CPP)
/// 2. Doorstop loader (winhttp.dll for Windows)
/// 3. BepInEx configuration
///
/// The LocalMultiplayer mod should be in the handler's overlay/BepInEx/plugins/
fn create_instance_overlay(
    instance_idx: usize,
    _handler: &Handler, // Reserved for handler-specific overlay customization
    config: &PhotonConfig,
    is_windows: bool,
    backend: UnityBackend,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let overlay_dir = PATH_PARTY
        .join("tmp")
        .join(format!("photon-overlay-{}", instance_idx));

    // Clean previous overlay
    if overlay_dir.exists() {
        fs::remove_dir_all(&overlay_dir)?;
    }
    fs::create_dir_all(&overlay_dir)?;

    let bepinex_res = get_bepinex_res_path(backend);

    if !bepinex_res.exists() {
        let backend_name = match backend {
            UnityBackend::Mono => "mono",
            UnityBackend::Il2Cpp => "il2cpp",
        };
        return Err(format!(
            "BepInEx resources not found for {} backend. Please run ./splitux.sh build",
            backend_name
        ).into());
    }

    // 1. Copy BepInEx core
    let bepinex_core_src = bepinex_res.join("core");
    let bepinex_core_dest = overlay_dir.join("BepInEx").join("core");
    if bepinex_core_src.exists() {
        copy_dir_recursive(&bepinex_core_src, &bepinex_core_dest)?;
    }

    // 2. Copy doorstop loader
    if is_windows {
        let winhttp_src = bepinex_res.join("winhttp.dll");
        if winhttp_src.exists() {
            fs::copy(&winhttp_src, overlay_dir.join("winhttp.dll"))?;
        }
    }

    // 3. Write doorstop config
    write_doorstop_config(&overlay_dir, is_windows, backend)?;

    // 4. Create BepInEx config directory
    let bepinex_config_dir = overlay_dir.join("BepInEx").join("config");
    fs::create_dir_all(&bepinex_config_dir)?;

    let backend_name = match backend {
        UnityBackend::Mono => "Mono",
        UnityBackend::Il2Cpp => "IL2CPP",
    };
    println!(
        "[splitux] Photon overlay {} created: Player {}, Port {}, Backend: {}",
        instance_idx, config.player_name, config.listen_port, backend_name
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
    // BepInEx 5 (Mono) uses BepInEx.Preloader.dll
    // BepInEx 6 (IL2CPP) uses BepInEx.Preloader.dll but in Unity.IL2CPP subfolder structure
    let preloader_dll = match backend {
        UnityBackend::Mono => "BepInEx.Preloader.dll",
        UnityBackend::Il2Cpp => "BepInEx.Preloader.dll", // Same name, different content
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

/// Create Photon overlays for all instances
pub fn create_all_overlays(
    handler: &Handler,
    instances: &[Instance],
    is_windows: bool,
    game_dir: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    // Check if Photon App IDs are configured
    let photon_ids = load_photon_ids();
    if photon_ids.pun_app_id.is_empty() {
        return Err(
            "Photon PUN App ID not configured. Go to Settings > Photon to set it up.".into(),
        );
    }

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

    // Generate ports for each instance (different range from Goldberg)
    const BASE_PORT: u16 = 47684;
    let instance_ports: Vec<u16> = (0..instances.len())
        .map(|i| BASE_PORT + i as u16)
        .collect();

    let mut overlays = Vec::new();

    for (i, instance) in instances.iter().enumerate() {
        let broadcast_ports: Vec<u16> = instance_ports
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, &port)| port)
            .collect();

        let config = PhotonConfig {
            instance_idx: i,
            player_name: instance.profname.clone(),
            listen_port: instance_ports[i],
            broadcast_ports,
        };

        let overlay = create_instance_overlay(i, handler, &config, is_windows, backend)?;
        overlays.push(overlay);
    }

    Ok(overlays)
}

/// Generate the LocalMultiplayer config for an instance
///
/// This writes the config file to the profile's windata directory at the path
/// specified in handler.photon.config_path
pub fn generate_instance_config(
    profile_path: &Path,
    handler: &Handler,
    instance_idx: usize,
    total_instances: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let photon_ids = load_photon_ids();

    // Get Photon settings from new optional field (Phase 7)
    let photon_settings = handler.photon_ref()
        .ok_or("Photon backend not enabled")?;

    // Get config path from handler settings
    let config_path_pattern = &photon_settings.config_path;
    if config_path_pattern.is_empty() {
        return Err(
            "Handler must specify photon.config_path for Photon backend".into(),
        );
    }

    // Build the full path within the profile's windata directory
    let config_path = profile_path.join("windata").join(config_path_pattern);

    // Create parent directories
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Generate config content
    let config = format!(
        r#"[Photon]
AppId={}
VoiceAppId={}

[LocalMultiplayer]
PlayerIndex={}
TotalPlayers={}
ListenPort={}
"#,
        photon_ids.pun_app_id,
        photon_ids.voice_app_id,
        instance_idx,
        total_instances,
        47684 + instance_idx as u16,
    );

    fs::write(&config_path, config)?;
    println!(
        "[splitux] Photon config written: {}",
        config_path.display()
    );

    Ok(())
}

/// Generate Photon configs for all instances at launch time
pub fn generate_all_configs(
    handler: &Handler,
    instances: &[Instance],
) -> Result<(), Box<dyn std::error::Error>> {
    // Get Photon settings from new optional field (Phase 7)
    let photon_settings = handler.photon_ref()
        .ok_or("Photon backend not enabled")?;

    // Set up shared files first (before instance configs)
    if !photon_settings.shared_files.is_empty() {
        setup_shared_files(handler, instances)?;
    }

    for (i, instance) in instances.iter().enumerate() {
        let profile_path = PATH_PARTY.join("profiles").join(&instance.profname);
        generate_instance_config(&profile_path, handler, i, instances.len())?;
    }
    Ok(())
}

/// Set up shared files between instances
///
/// For mods like LocalMultiplayer that need to share data (e.g., lobby IDs),
/// this creates symlinks from each instance's expected file location to a
/// shared file location.
fn setup_shared_files(
    handler: &Handler,
    instances: &[Instance],
) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    // Get Photon settings from new optional field (Phase 7)
    let photon_settings = handler.photon_ref()
        .ok_or("Photon backend not enabled")?;

    // Create shared directory for this game session
    let shared_dir = PATH_PARTY.join("tmp").join("photon-shared");
    fs::create_dir_all(&shared_dir)?;

    for shared_path_pattern in &photon_settings.shared_files {
        // Get the filename for the shared file
        let file_name = Path::new(shared_path_pattern)
            .file_name()
            .ok_or_else(|| format!("Invalid shared file path: {}", shared_path_pattern))?
            .to_string_lossy();

        let shared_file = shared_dir.join(file_name.as_ref());

        // Create initial shared file if it doesn't exist
        // For GlobalSave, create a minimal valid JSON structure
        if !shared_file.exists() {
            let initial_content = if file_name == "GlobalSave" {
                // LocalMultiplayer mod expects this JSON structure
                r#"{
  "SpoofSteamAccounts": [],
  "SpoofSteamAccountsInUse": []
}"#
            } else {
                // Empty file for unknown shared files
                ""
            };
            fs::write(&shared_file, initial_content)?;
            println!(
                "[splitux] Created shared file: {}",
                shared_file.display()
            );
        }

        // Symlink from each instance's expected path to the shared file
        for instance in instances {
            let profile_path = PATH_PARTY.join("profiles").join(&instance.profname);
            let instance_file = profile_path.join("windata").join(shared_path_pattern);

            // Create parent directories
            if let Some(parent) = instance_file.parent() {
                fs::create_dir_all(parent)?;
            }

            // Remove existing file/symlink
            if instance_file.exists() || instance_file.is_symlink() {
                fs::remove_file(&instance_file)?;
            }

            // Create symlink to shared file
            symlink(&shared_file, &instance_file)?;
            println!(
                "[splitux] {} -> {}",
                instance_file.display(),
                shared_file.display()
            );
        }
    }

    Ok(())
}
