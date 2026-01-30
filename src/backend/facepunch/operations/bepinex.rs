//! BepInEx installation operations for Facepunch backend
//!
//! Functions for installing and configuring BepInEx with SplituxFacepunch plugin.

use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::PATH_ASSETS;

// Re-use UnityBackend from photon module
use crate::backend::photon::UnityBackend;

/// Get the BepInEx resource path for Facepunch
/// Uses mono-linux for Linux native games (different from Photon)
pub fn get_bepinex_res_path(is_windows: bool, backend: UnityBackend) -> PathBuf {
    let subdir = match (is_windows, backend) {
        (true, UnityBackend::Mono) => "mono",
        (false, UnityBackend::Mono) => "mono-linux",
        (_, UnityBackend::Il2Cpp) => "il2cpp",
    };
    PATH_ASSETS.join("bepinex").join(subdir)
}

/// Install BepInEx core files to overlay directory
pub fn install_bepinex_core(
    overlay_dir: &Path,
    is_windows: bool,
    backend: UnityBackend,
) -> Result<(), Box<dyn std::error::Error>> {
    let bepinex_res = get_bepinex_res_path(is_windows, backend);

    if !bepinex_res.exists() {
        let subdir = match (is_windows, backend) {
            (true, UnityBackend::Mono) => "mono",
            (false, UnityBackend::Mono) => "mono-linux",
            (_, UnityBackend::Il2Cpp) => "il2cpp",
        };
        return Err(format!(
            "BepInEx resources not found for {} backend. Please run ./splitux.sh build",
            subdir
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
        // Linux native: copy libdoorstop.so
        let libdoorstop_src = bepinex_res.join("libdoorstop.so");
        if libdoorstop_src.exists() {
            fs::copy(&libdoorstop_src, overlay_dir.join("libdoorstop.so"))?;
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

/// Install SplituxFacepunch plugin DLL
pub fn install_splitux_plugin(overlay_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let plugins_dir = overlay_dir.join("BepInEx").join("plugins");
    fs::create_dir_all(&plugins_dir)?;

    let plugin_src = PATH_ASSETS.join("facepunch").join("SplituxFacepunch.dll");
    if plugin_src.exists() {
        fs::copy(&plugin_src, plugins_dir.join("SplituxFacepunch.dll"))?;
    } else {
        println!(
            "[splitux] Warning: SplituxFacepunch.dll not found at {}",
            plugin_src.display()
        );
        println!("[splitux] Run ./splitux.sh build to download it");
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
