//! BepInEx installation operations
//!
//! Platform-aware functions for installing and configuring BepInEx in game overlays.

use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::PATH_RES;

use super::types::UnityBackend;

/// Get BepInEx resource path, with platform-aware subdirectory
///
/// - Windows Mono: `mono`
/// - Linux Mono: `mono-linux`
/// - IL2CPP (both): `il2cpp`
pub fn get_bepinex_res_path(is_windows: bool, backend: UnityBackend) -> PathBuf {
    let subdir = match (is_windows, backend) {
        (true, UnityBackend::Mono) => "mono",
        (false, UnityBackend::Mono) => "mono-linux",
        (_, UnityBackend::Il2Cpp) => "il2cpp",
    };
    PATH_RES.join("bepinex").join(subdir)
}

/// Check if specific BepInEx backend is available
pub fn bepinex_backend_available(is_windows: bool, backend: UnityBackend) -> bool {
    get_bepinex_res_path(is_windows, backend)
        .join("core")
        .exists()
}

/// Copy BepInEx core files to overlay directory
pub fn install_bepinex_core(
    overlay_dir: &Path,
    is_windows: bool,
    backend: UnityBackend,
) -> Result<(), Box<dyn std::error::Error>> {
    let bepinex_res = get_bepinex_res_path(is_windows, backend);

    if !bepinex_res.exists() {
        return Err(format!(
            "BepInEx resources not found for {} backend ({}). Please run ./splitux.sh build",
            backend.display_name(),
            if is_windows { "Windows" } else { "Linux" }
        )
        .into());
    }

    // Copy BepInEx core
    let bepinex_core_src = bepinex_res.join("core");
    let bepinex_core_dest = overlay_dir.join("BepInEx").join("core");
    if bepinex_core_src.exists() {
        copy_dir_recursive(&bepinex_core_src, &bepinex_core_dest)?;
    }

    // Create BepInEx config directory
    let bepinex_config_dir = overlay_dir.join("BepInEx").join("config");
    fs::create_dir_all(&bepinex_config_dir)?;

    Ok(())
}

/// Install doorstop loader (platform-specific)
///
/// - Windows: Copies `winhttp.dll`
/// - Linux: Copies `libdoorstop.so` (if available)
pub fn install_doorstop(
    overlay_dir: &Path,
    is_windows: bool,
    backend: UnityBackend,
) -> Result<(), Box<dyn std::error::Error>> {
    let bepinex_res = get_bepinex_res_path(is_windows, backend);

    if is_windows {
        let winhttp_src = bepinex_res.join("winhttp.dll");
        if winhttp_src.exists() {
            fs::copy(&winhttp_src, overlay_dir.join("winhttp.dll"))?;
        }
    } else {
        // Linux doorstop
        let doorstop_src = bepinex_res.join("libdoorstop.so");
        if doorstop_src.exists() {
            fs::copy(&doorstop_src, overlay_dir.join("libdoorstop.so"))?;
        }
    }
    Ok(())
}

/// Write BepInEx doorstop configuration
pub fn write_doorstop_config(
    overlay_dir: &Path,
    is_windows: bool,
    backend: UnityBackend,
) -> Result<(), Box<dyn std::error::Error>> {
    // Copy the bundled doorstop_config.ini instead of generating minimal one
    let bepinex_res = get_bepinex_res_path(is_windows, backend);
    let config_src = bepinex_res.join("doorstop_config.ini");

    if config_src.exists() {
        fs::copy(&config_src, overlay_dir.join("doorstop_config.ini"))?;
    } else {
        // Fallback to minimal config if bundled one doesn't exist
        let preloader_dll = "BepInEx.Preloader.dll";
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
    }
    Ok(())
}

/// Install plugin DLLs to overlay's BepInEx/plugins/ directory
pub fn install_plugin_dlls(
    overlay_dir: &Path,
    dll_files: &[PathBuf],
) -> Result<(), Box<dyn std::error::Error>> {
    if dll_files.is_empty() {
        return Ok(());
    }

    let plugins_dir = overlay_dir.join("BepInEx").join("plugins");
    fs::create_dir_all(&plugins_dir)?;

    for dll_path in dll_files {
        let filename = dll_path.file_name().ok_or("Invalid DLL path")?;
        let dest = plugins_dir.join(filename);
        fs::copy(dll_path, &dest)?;
        eprintln!("[bepinex] Installed plugin: {:?}", filename);
    }

    Ok(())
}

/// Copy directory recursively
pub fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
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
