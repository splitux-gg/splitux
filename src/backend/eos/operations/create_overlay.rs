//! EOS overlay creation
//!
//! Creates per-instance overlay directories with EOS emulator DLLs and configuration.

use std::fs;
use std::path::{Path, PathBuf};

use crate::backend::operations::prepare_overlay_dir;
use crate::paths::PATH_ASSETS;

use super::super::types::{EosConfig, EosDll};
use super::write_settings::write_eos_settings;

/// Create an EOS emulator overlay for a single game instance
///
/// This creates a directory structure that mirrors the game's EOS SDK DLL locations,
/// replacing them with the Nemirtingas emulator DLLs and adding configuration.
/// The overlay is meant to be used as a lowerdir in fuse-overlayfs.
pub fn create_instance_overlay(
    instance_idx: usize,
    dlls: &[EosDll],
    config: &EosConfig,
    is_windows: bool,
    enable_lan: bool,
    disable_online_networking: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let overlay_dir = prepare_overlay_dir("eos", instance_idx)?;

    for dll in dlls {
        let dll_dir = dll.rel_path.parent().unwrap_or(Path::new(""));
        let target_dir = overlay_dir.join(dll_dir);
        fs::create_dir_all(&target_dir)?;

        // Original DLL name (what the game expects)
        let dll_name = dll.rel_path.file_name().unwrap();

        // Determine source path based on platform and bitness
        let src_path = if is_windows {
            if dll.is_64bit {
                PATH_ASSETS.join("eos/win64/EOSSDK-Win64-Shipping.dll")
            } else {
                PATH_ASSETS.join("eos/win32/EOSSDK-Win32-Shipping.dll")
            }
        } else {
            // Linux - only 64-bit supported for now
            PATH_ASSETS.join("eos/linux64/libEOSSDK-Linux-Shipping.so")
        };

        let dest_path = target_dir.join(dll_name);

        if src_path.exists() {
            fs::copy(&src_path, &dest_path)?;
            println!(
                "[splitux] EOS overlay {}: {} -> {}",
                instance_idx,
                src_path.display(),
                dest_path.display()
            );
        } else {
            println!(
                "[splitux] Warning: EOS emulator DLL not found: {}",
                src_path.display()
            );
        }

        // Create nepice_settings next to DLL (Nemirtingas config directory)
        let settings_dir = target_dir.join("nepice_settings");
        write_eos_settings(&settings_dir, config, enable_lan, disable_online_networking)?;
    }

    // For native Linux games, also create nepice_settings at overlay root
    // The emulator looks for config next to the executable
    if !is_windows {
        let root_settings_dir = overlay_dir.join("nepice_settings");
        if !root_settings_dir.exists() {
            write_eos_settings(
                &root_settings_dir,
                config,
                enable_lan,
                disable_online_networking,
            )?;
            println!(
                "[splitux] EOS overlay {}: Also created nepice_settings at game root",
                instance_idx
            );
        }
    }

    println!(
        "[splitux] EOS overlay {} created: User {}, Port {}, enable_lan: {}, disable_online: {}",
        instance_idx, config.username, config.listen_port, enable_lan, disable_online_networking
    );

    Ok(overlay_dir)
}
