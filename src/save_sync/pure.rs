// Pure functions for save synchronization
// No side effects - only computation and path manipulation

use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::{PATH_HOME, PATH_PARTY};
use regex::Regex;
use std::path::{Path, PathBuf};

/// For a Windows save path, return the portion AFTER the prefix's Windows user
/// home (`…/drive_c/users/<user>/…`). splitux bind-mounts a profile's `windata`
/// over that user home, so this suffix is exactly the windata-relative dest.
/// Returns None for a path that has no `users/<user>/` segment (e.g. an already
/// windata-relative `AppData/...` string), so callers can use it verbatim.
fn windata_relative(p: &Path) -> Option<PathBuf> {
    let comps: Vec<_> = p.components().collect();
    for i in 0..comps.len() {
        if comps[i].as_os_str() == "users" && i + 2 <= comps.len().saturating_sub(1) {
            let rest: PathBuf = comps[i + 2..].iter().collect();
            if !rest.as_os_str().is_empty() {
                return Some(rest);
            }
        }
    }
    None
}

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

    // Windows games: the profile's windata is bind-mounted over the prefix's
    // drive_c/users/steamuser, so the dest is the part of the original AFTER that
    // user home. An ABSOLUTE Proton/compatdata save (…/users/steamuser/AppData/…)
    // maps by stripping the prefix up to and including `users/<user>/`; an already
    // windata-relative `AppData/...` string is used verbatim. (The previous code
    // joined the whole original, so an absolute path replaced the windata base and
    // mapped the save onto itself.)
    if h.win() || h.original_save_path.contains("AppData") {
        let rel = windata_relative(&original)
            .unwrap_or_else(|| PathBuf::from(&h.original_save_path));
        let dest = profile_path.join("windata").join(rel);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windata_relative_strips_proton_prefix() {
        // Absolute compatdata save → windata-relative AppData suffix.
        let abs = Path::new(
            "/mnt/games/SteamLibrary/steamapps/compatdata/1347970/pfx/drive_c/users/steamuser/AppData/LocalLow/Lychee Game Labs/Patch Quest",
        );
        assert_eq!(
            windata_relative(abs),
            Some(PathBuf::from("AppData/LocalLow/Lychee Game Labs/Patch Quest"))
        );
    }

    #[test]
    fn windata_relative_none_for_already_relative() {
        // A windata-relative AppData string has no users/<user>/ segment.
        assert_eq!(
            windata_relative(Path::new("AppData/LocalLow/Co/Game")),
            None
        );
    }
}
