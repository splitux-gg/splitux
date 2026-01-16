//! EOS SDK DLL discovery
//!
//! Finds EOS SDK DLLs in game directories for emulator replacement.

use std::path::Path;
use walkdir::WalkDir;

use super::super::types::EosDll;

/// Find all EOS SDK DLLs in the game directory
///
/// Searches recursively for EOSSDK-Win64-Shipping.dll, EOSSDK-Win32-Shipping.dll,
/// and libEOSSDK-Linux-Shipping.so.
/// Returns relative paths from the game root along with bitness information.
pub fn find_eos_dlls(game_dir: &Path) -> Result<Vec<EosDll>, Box<dyn std::error::Error>> {
    let mut dlls = Vec::new();

    for entry in WalkDir::new(game_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
            let filename_lower = filename.to_lowercase();

            // EOS SDK DLLs
            let is_eos_dll = filename_lower == "eossdk-win64-shipping.dll"
                || filename_lower == "eossdk-win32-shipping.dll"
                || filename_lower == "libeossdk-linux-shipping.so";

            if is_eos_dll {
                if let Ok(rel_path) = path.strip_prefix(game_dir) {
                    let is_64bit = filename_lower.contains("64")
                        || filename_lower.contains("linux");

                    dlls.push(EosDll {
                        rel_path: rel_path.to_path_buf(),
                        is_64bit,
                    });
                    println!(
                        "[splitux] Found EOS SDK: {} ({})",
                        rel_path.display(),
                        if is_64bit { "64-bit" } else { "32-bit" }
                    );
                }
            }
        }
    }

    Ok(dlls)
}
