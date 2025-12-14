//! DLL bitness detection
//!
//! Pure functions for detecting whether Steam API DLLs are 32-bit or 64-bit.

use std::fs;
use std::path::Path;

/// Detect whether a Steam API DLL is 64-bit based on filename, directory hints, or ELF header
///
/// This is a pure function that determines bitness from:
/// 1. Filename (steam_api64.dll is always 64-bit)
/// 2. Directory structure hints (win64, x64, x86_64)
/// 3. ELF header for Linux .so files
pub fn detect_bitness(path: &Path, filename: &str) -> bool {
    // 1. Check filename first (most reliable for Windows)
    if filename == "steam_api64.dll" {
        return true;
    }
    if filename == "steam_api.dll" {
        // 2. Check directory structure for hints
        let path_str = path.to_string_lossy().to_lowercase();
        if path_str.contains("win64")
            || path_str.contains("x64")
            || path_str.contains("x86_64")
            || path_str.contains("/64/")
        {
            return true;
        }
        // Default to 32-bit for steam_api.dll
        return false;
    }
    // Linux: libsteam_api.so - read ELF header to determine bitness
    if filename == "libsteam_api.so" {
        if let Ok(data) = fs::read(path) {
            // ELF magic: 0x7F 'E' 'L' 'F'
            // e_ident[EI_CLASS] at offset 4: 1 = 32-bit, 2 = 64-bit
            if data.len() >= 5 && &data[0..4] == b"\x7FELF" {
                return data[4] == 2; // ELFCLASS64
            }
        }
        // Fallback to directory hints if ELF read fails
        let path_str = path.to_string_lossy().to_lowercase();
        return path_str.contains("64") || path_str.contains("x86_64");
    }
    // Other files: use directory hints
    let path_str = path.to_string_lossy().to_lowercase();
    path_str.contains("64") || path_str.contains("x86_64")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_steam_api64_is_64bit() {
        let path = PathBuf::from("/game/steam_api64.dll");
        assert!(detect_bitness(&path, "steam_api64.dll"));
    }

    #[test]
    fn test_steam_api_default_32bit() {
        let path = PathBuf::from("/game/steam_api.dll");
        assert!(!detect_bitness(&path, "steam_api.dll"));
    }

    #[test]
    fn test_steam_api_in_win64_dir() {
        let path = PathBuf::from("/game/win64/steam_api.dll");
        assert!(detect_bitness(&path, "steam_api.dll"));
    }

    #[test]
    fn test_steam_api_in_x64_dir() {
        let path = PathBuf::from("/game/bin/x64/steam_api.dll");
        assert!(detect_bitness(&path, "steam_api.dll"));
    }
}
