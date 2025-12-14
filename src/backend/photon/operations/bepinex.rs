//! BepInEx installation operations
//!
//! Functions for installing and configuring BepInEx in game overlays.

use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::PATH_RES;

use super::super::types::UnityBackend;

/// Check if BepInEx resources are available (either Mono or IL2CPP)
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
pub fn get_bepinex_res_path(backend: UnityBackend) -> PathBuf {
    PATH_RES.join("bepinex").join(backend.bepinex_subdir())
}

/// Copy BepInEx core files to overlay directory
pub fn install_bepinex_core(
    overlay_dir: &Path,
    backend: UnityBackend,
) -> Result<(), Box<dyn std::error::Error>> {
    let bepinex_res = get_bepinex_res_path(backend);

    if !bepinex_res.exists() {
        return Err(format!(
            "BepInEx resources not found for {} backend. Please run ./splitux.sh build",
            backend.display_name()
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

/// Install doorstop loader (winhttp.dll for Windows)
pub fn install_doorstop(
    overlay_dir: &Path,
    backend: UnityBackend,
    is_windows: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if is_windows {
        let bepinex_res = get_bepinex_res_path(backend);
        let winhttp_src = bepinex_res.join("winhttp.dll");
        if winhttp_src.exists() {
            fs::copy(&winhttp_src, overlay_dir.join("winhttp.dll"))?;
        }
    }
    Ok(())
}

/// Write BepInEx doorstop configuration
pub fn write_doorstop_config(
    overlay_dir: &Path,
    is_windows: bool,
    _backend: UnityBackend,
) -> Result<(), Box<dyn std::error::Error>> {
    // BepInEx 5 (Mono) and BepInEx 6 (IL2CPP) both use BepInEx.Preloader.dll
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
