//! gptokeyb profile storage operations
//!
//! Handles loading/saving user-created profiles to:
//! ~/.local/share/splitux/gptokeyb/profiles/

use std::path::PathBuf;

use crate::paths::{PATH_PARTY, PATH_RES};

use super::parser::{parse_gptk, serialize_gptk};
use super::profile::GptokeybProfile;

/// Get the user profiles directory path
pub fn profiles_dir() -> PathBuf {
    PATH_PARTY.join("gptokeyb/profiles")
}

/// Ensure the profiles directory exists
pub fn ensure_profiles_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(profiles_dir())
}

/// List all user-created profile names (without .gptk extension)
pub fn list_user_profiles() -> Vec<String> {
    let dir = profiles_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut profiles = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "gptk") {
                if let Some(stem) = path.file_stem() {
                    profiles.push(stem.to_string_lossy().into_owned());
                }
            }
        }
    }
    profiles.sort();
    profiles
}

/// List built-in profile names from res/gptokeyb/
pub fn list_builtin_profiles() -> Vec<String> {
    let dir = PATH_RES.join("gptokeyb");
    if !dir.exists() {
        return Vec::new();
    }

    let mut profiles = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "gptk") {
                if let Some(stem) = path.file_stem() {
                    profiles.push(stem.to_string_lossy().into_owned());
                }
            }
        }
    }
    profiles.sort();
    profiles
}

/// Load a user-created profile by name
pub fn load_user_profile(name: &str) -> Result<GptokeybProfile, String> {
    let path = profiles_dir().join(format!("{}.gptk", name));
    load_profile_from_path(&path, name)
}

/// Load a profile from a specific path
fn load_profile_from_path(path: &PathBuf, name: &str) -> Result<GptokeybProfile, String> {
    if !path.exists() {
        return Err(format!("Profile not found: {}", path.display()));
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read profile: {}", e))?;

    parse_gptk(&content, name)
}

/// Save a profile to the user profiles directory
pub fn save_profile(profile: &GptokeybProfile) -> Result<(), String> {
    if profile.name.is_empty() {
        return Err("Profile name cannot be empty".to_string());
    }

    // Validate name (alphanumeric, underscores, hyphens only)
    if !profile
        .name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Profile name can only contain letters, numbers, underscores, and hyphens".to_string());
    }

    ensure_profiles_dir().map_err(|e| format!("Failed to create profiles directory: {}", e))?;

    let path = profiles_dir().join(format!("{}.gptk", profile.name));
    let content = serialize_gptk(profile);

    std::fs::write(&path, content).map_err(|e| format!("Failed to write profile: {}", e))?;

    Ok(())
}

/// Delete a user profile by name
pub fn delete_profile(name: &str) -> Result<(), String> {
    let path = profiles_dir().join(format!("{}.gptk", name));

    if !path.exists() {
        return Err(format!("Profile not found: {}", name));
    }

    std::fs::remove_file(&path).map_err(|e| format!("Failed to delete profile: {}", e))?;

    Ok(())
}
