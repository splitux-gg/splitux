//! Steam API DLL discovery
//!
//! Finds Steam API DLLs in game directories for Goldberg replacement.

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::super::pure::detect_bitness;
use super::super::types::{SteamApiDll, SteamDllType};

/// Find all Steam API DLLs in the game directory
///
/// Searches recursively for steam_api.dll, steam_api64.dll, libsteam_api.so,
/// and GameNetworkingSockets.dll.
/// Returns relative paths from the game root along with bitness information.
pub fn find_steam_api_dlls(game_dir: &Path) -> Result<Vec<SteamApiDll>, Box<dyn std::error::Error>> {
    let mut dlls = Vec::new();
    // Track directories containing 64-bit steam_api DLLs (for inferring GameNetworkingSockets bitness)
    let mut dirs_with_64bit_steam_api: Vec<PathBuf> = Vec::new();
    // Defer GameNetworkingSockets detection until we know steam_api bitness
    let mut networking_sockets_paths: Vec<(PathBuf, PathBuf)> = Vec::new(); // (full_path, rel_path)

    for entry in WalkDir::new(game_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
            let filename_lower = filename.to_lowercase();

            // Standard Steam API DLLs
            if filename_lower == "steam_api.dll"
                || filename_lower == "steam_api64.dll"
                || filename_lower == "libsteam_api.so"
            {
                if let Ok(rel_path) = path.strip_prefix(game_dir) {
                    let is_64bit = detect_bitness(path, &filename_lower);
                    if is_64bit {
                        if let Some(dir) = rel_path.parent() {
                            dirs_with_64bit_steam_api.push(dir.to_path_buf());
                        }
                    }
                    dlls.push(SteamApiDll {
                        rel_path: rel_path.to_path_buf(),
                        is_64bit,
                        dll_type: SteamDllType::SteamApi,
                    });
                    println!(
                        "[splitux] Found Steam API: {} ({})",
                        rel_path.display(),
                        if is_64bit { "64-bit" } else { "32-bit" }
                    );
                }
            }
            // GameNetworkingSockets.dll - defer bitness detection
            else if filename_lower == "gamenetworkingsockets.dll" {
                if let Ok(rel_path) = path.strip_prefix(game_dir) {
                    networking_sockets_paths.push((path.to_path_buf(), rel_path.to_path_buf()));
                }
            }
        }
    }

    // Now process GameNetworkingSockets.dll with knowledge of steam_api locations
    for (full_path, rel_path) in networking_sockets_paths {
        let dll_dir = rel_path.parent().map(|p| p.to_path_buf());

        // Infer bitness: if steam_api64.dll exists in same directory, it's 64-bit
        let is_64bit = if let Some(ref dir) = dll_dir {
            dirs_with_64bit_steam_api.iter().any(|d| d == dir)
        } else {
            // Fallback to path-based detection
            detect_bitness(&full_path, "gamenetworkingsockets.dll")
        };

        dlls.push(SteamApiDll {
            rel_path: rel_path.clone(),
            is_64bit,
            dll_type: SteamDllType::NetworkingSockets,
        });
        println!(
            "[splitux] Found GameNetworkingSockets: {} ({})",
            rel_path.display(),
            if is_64bit { "64-bit" } else { "32-bit" }
        );
    }

    Ok(dlls)
}
