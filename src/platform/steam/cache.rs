//! Steam artwork cache resolution
//!
//! Steam stores game artwork in the librarycache folder:
//! {STEAM}/appcache/librarycache/{appid}/{hash}/{filename}
//! or directly as: {STEAM}/appcache/librarycache/{appid}/{filename}

use crate::paths::PATH_STEAM;
use std::path::PathBuf;

/// Find a file in Steam's librarycache for an app
///
/// Steam stores files in: {STEAM}/appcache/librarycache/{appid}/{hash}/{filename}
/// or directly as: {STEAM}/appcache/librarycache/{appid}/{filename}
pub fn find_cache_file(app_id: u32, filename: &str) -> Option<PathBuf> {
    let app_cache = PATH_STEAM
        .join("appcache/librarycache")
        .join(app_id.to_string());

    if !app_cache.exists() {
        return None;
    }

    // Check directly in app folder
    let direct_path = app_cache.join(filename);
    if direct_path.exists() {
        return Some(direct_path);
    }

    // Search in hash subfolders
    if let Ok(entries) = std::fs::read_dir(&app_cache) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let file_path = entry.path().join(filename);
                if file_path.exists() {
                    return Some(file_path);
                }
            }
        }
    }

    None
}

/// Find the small icon (32x32 jpg) for an app
///
/// Icon files have hash names like "b3a992fd5991bd2f4c956d58e062b0ce2988d6cd.jpg"
/// directly in the app folder (not in subfolders).
/// Skip files named library_*, header*, logo* as those are other artwork.
pub fn find_icon(app_id: u32) -> Option<PathBuf> {
    let app_cache = PATH_STEAM
        .join("appcache/librarycache")
        .join(app_id.to_string());

    if !app_cache.exists() {
        return None;
    }

    // Look for image files directly in the app folder (not in subfolders)
    if let Ok(entries) = std::fs::read_dir(&app_cache) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    // Skip known non-icon files
                    if filename.starts_with("library_")
                        || filename.starts_with("header")
                        || filename.starts_with("logo")
                    {
                        continue;
                    }

                    if let Some(ext) = path.extension() {
                        if ext == "jpg" || ext == "png" || ext == "ico" {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    None
}

/// Get the box art (library_600x900.jpg) as a file:// URI
pub fn box_art_uri(app_id: u32) -> Option<String> {
    find_cache_file(app_id, "library_600x900.jpg").map(|p| format!("file://{}", p.display()))
}

/// Get the game logo (logo.png) as a file:// URI
pub fn logo_uri(app_id: u32) -> Option<String> {
    find_cache_file(app_id, "logo.png").map(|p| format!("file://{}", p.display()))
}

/// Get the hero image (library_hero.jpg, 1920x620 banner) as a file:// URI
pub fn hero_uri(app_id: u32) -> Option<String> {
    find_cache_file(app_id, "library_hero.jpg").map(|p| format!("file://{}", p.display()))
}

/// Get the header image (library_header.jpg) as a file:// URI
pub fn header_uri(app_id: u32) -> Option<String> {
    find_cache_file(app_id, "library_header.jpg").map(|p| format!("file://{}", p.display()))
}

/// Get the icon as a file:// URI
pub fn icon_uri(app_id: u32) -> Option<String> {
    find_icon(app_id).map(|p| format!("file://{}", p.display()))
}
