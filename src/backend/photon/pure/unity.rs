//! Unity backend detection
//!
//! Pure functions for detecting Unity game backend type.

use std::fs;
use std::path::Path;

use super::super::types::UnityBackend;

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
                        println!(
                            "[splitux] Detected Unity Mono backend (found {}/Managed/)",
                            name
                        );
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
