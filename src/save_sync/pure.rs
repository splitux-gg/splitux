// Pure functions for save synchronization
// No side effects - only computation and path manipulation

use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::{PATH_HOME, PATH_PARTY};
use regex::Regex;
use std::path::PathBuf;

/// Expand ~ and $HOME in path
pub fn expand_path(path: &str) -> PathBuf {
    let mut s = path.to_string();
    if s.starts_with("~/") {
        s = s.replacen("~", &PATH_HOME.to_string_lossy(), 1);
    }
    s = s.replace("$HOME", &PATH_HOME.to_string_lossy());
    PathBuf::from(s)
}

/// Get the game root directory from handler
pub fn get_game_root(h: &Handler) -> Option<PathBuf> {
    if !h.path_gameroot.is_empty() {
        return Some(PathBuf::from(&h.path_gameroot));
    }
    // Game root is resolved elsewhere for steam_appid games
    // The handler should have path_gameroot populated by launch time
    None
}

/// Get handler directory name (used for gamesaves subdir)
pub fn get_handler_name(h: &Handler) -> String {
    PathBuf::from(&h.path_handler)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Determine where to copy saves in the profile
/// Returns (profile_save_path, is_inside_game_dir)
pub fn get_profile_save_path(profile_name: &str, h: &Handler) -> (PathBuf, bool) {
    let profile_path = PATH_PARTY.join("profiles").join(profile_name);
    let original = expand_path(&h.original_save_path);
    let handler_name = get_handler_name(h);

    // Check if save path is inside game directory
    if let Some(game_root) = get_game_root(h) {
        if let Ok(relative) = original.strip_prefix(&game_root) {
            // Saves are inside game dir -> goes to gamesaves overlay upperdir
            let dest = profile_path
                .join("gamesaves")
                .join(&handler_name)
                .join(relative);
            return (dest, true);
        }
    }

    // Check if under HOME (Linux native games)
    if let Ok(relative) = original.strip_prefix(&*PATH_HOME) {
        let dest = profile_path.join("home").join(relative);
        return (dest, false);
    }

    // For Windows games or other paths, use windata if it looks like AppData
    if h.win() || h.original_save_path.contains("AppData") {
        // Keep the relative structure for windata
        let dest = profile_path.join("windata").join(&h.original_save_path);
        return (dest, false);
    }

    // Fallback: put in gamesaves
    let dest = profile_path.join("gamesaves").join(&handler_name);
    (dest, false)
}

/// Get the original save path (just expand variables)
pub fn get_original_save_path(h: &Handler) -> Option<PathBuf> {
    if h.original_save_path.is_empty() {
        return None;
    }
    Some(expand_path(&h.original_save_path))
}

/// Steam64 ID regex pattern - matches 17-digit Steam IDs starting with 7656119
/// Format: 76561197960265728 + account_id (0 to ~4 billion)
pub fn steam_id_regex() -> Regex {
    Regex::new(r"^(7656119\d{10})(.*)$").unwrap()
}

/// Detect if a filename has a Steam ID prefix
/// Returns Some((steam_id, rest_of_filename)) if detected
pub fn extract_steam_id_from_filename(filename: &str) -> Option<(u64, String)> {
    let re = steam_id_regex();
    if let Some(caps) = re.captures(filename) {
        if let (Some(id_match), Some(rest_match)) = (caps.get(1), caps.get(2)) {
            if let Ok(steam_id) = id_match.as_str().parse::<u64>() {
                return Some((steam_id, rest_match.as_str().to_string()));
            }
        }
    }
    None
}

/// Find first named (non-guest) profile
pub fn find_first_named_profile(instances: &[Instance]) -> Option<&str> {
    instances
        .iter()
        .find(|i| !i.profname.starts_with('.'))
        .map(|i| i.profname.as_str())
}
