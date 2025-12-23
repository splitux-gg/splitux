//! BepInEx installation operations
//!
//! Thin wrappers around the shared bepinex module for backward compatibility.
//! Photon games are Windows-only, so we default is_windows=true.

use std::path::Path;

use super::super::types::UnityBackend;

/// Check if specific BepInEx backend is available
pub fn bepinex_backend_available(backend: UnityBackend) -> bool {
    // Photon is Windows-only
    crate::bepinex::bepinex_backend_available(true, backend)
}

/// Copy BepInEx core files to overlay directory
pub fn install_bepinex_core(
    overlay_dir: &Path,
    backend: UnityBackend,
) -> Result<(), Box<dyn std::error::Error>> {
    // Photon is Windows-only
    crate::bepinex::install_bepinex_core(overlay_dir, true, backend)
}

/// Install doorstop loader (winhttp.dll for Windows)
pub fn install_doorstop(
    overlay_dir: &Path,
    backend: UnityBackend,
    is_windows: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    crate::bepinex::install_doorstop(overlay_dir, is_windows, backend)
}

/// Write BepInEx doorstop configuration
pub fn write_doorstop_config(
    overlay_dir: &Path,
    is_windows: bool,
    backend: UnityBackend,
) -> Result<(), Box<dyn std::error::Error>> {
    crate::bepinex::write_doorstop_config(overlay_dir, is_windows, backend)
}
