use crate::paths::{PATH_HOME, PATH_PARTY};

use dialog::{Choice, DialogBox};
use eframe::egui::TextBuffer;
use rfd::FileDialog;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

pub fn msg(title: &str, contents: &str) {
    let _ = dialog::Message::new(contents).title(title).show();
}

pub fn yesno(title: &str, contents: &str) -> bool {
    if let Ok(prompt) = dialog::Question::new(contents).title(title).show() {
        if prompt == Choice::Yes {
            return true;
        }
    }
    false
}

pub fn dir_dialog() -> Result<PathBuf, Box<dyn Error>> {
    let dir = FileDialog::new()
        .set_title("Select Folder")
        .set_directory(&*PATH_HOME)
        .pick_folder()
        .ok_or_else(|| "No folder selected")?;
    Ok(dir)
}

pub fn file_dialog_relative(base_dir: &PathBuf) -> Result<PathBuf, Box<dyn Error>> {
    let file = FileDialog::new()
        .set_title("Select File")
        .set_directory(base_dir)
        .pick_file()
        .ok_or_else(|| "No file selected")?;

    if file.starts_with(base_dir) {
        let relative_path = file.strip_prefix(base_dir)?;
        Ok(relative_path.to_path_buf())
    } else {
        Err("Selected file is not within the base directory".into())
    }
}

pub fn copy_dir_recursive(src: &PathBuf, dest: &PathBuf) -> Result<(), Box<dyn Error>> {
    println!(
        "[splitux] util::copy_dir_recursive - src: {}, dest: {}",
        src.display(),
        dest.display()
    );

    let walk_path = walkdir::WalkDir::new(src).min_depth(1).follow_links(false);

    for entry in walk_path {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(src)?;
        let new_path = dest.join(rel_path);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&new_path)?;
        } else if entry.file_type().is_symlink() {
            let symlink_src = std::fs::read_link(entry.path())?;
            std::os::unix::fs::symlink(symlink_src, new_path)?;
        } else {
            if let Some(parent) = new_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if new_path.exists() {
                std::fs::remove_file(&new_path)?;
            }

            std::fs::copy(entry.path(), new_path)?;
        }
    }

    Ok(())
}

pub fn zip_dir(src_dir: &PathBuf, dest: &PathBuf) -> Result<(), Box<dyn Error>> {
    // Temp, should maybe be done with a crate
    std::process::Command::new("zip")
        .current_dir(src_dir)
        .arg("-r")
        .arg(dest.to_string_lossy().as_str())
        .arg(".")
        .output()?;
    Ok(())
}

pub fn get_installed_steamapps() -> Vec<Option<steamlocate::App>> {
    let mut games = Vec::new();
    games.push(None);

    if let Ok(steam_dir) = steamlocate::SteamDir::locate()
        && let Ok(libraries) = steam_dir.libraries()
    {
        for library in libraries {
            let library = match library {
                Ok(lib) => lib,
                Err(_) => continue,
            };

            for app in library.apps() {
                if let Ok(app) = app {
                    games.push(Some(app));
                }
            }
        }
    }

    return games;
}

/// Resolve a Proton path from a name (e.g., "Proton - Experimental") or full path
/// Returns the full path to the proton executable if found
pub fn resolve_proton_path(proton_name: &str) -> Option<PathBuf> {
    use crate::paths::PATH_STEAM;

    // If it's already a full path, use it directly
    let as_path = std::path::Path::new(proton_name);
    if as_path.is_absolute() {
        let proton_bin = as_path.join("proton");
        if proton_bin.exists() {
            return Some(proton_bin);
        }
        // Maybe they specified the proton binary directly
        if as_path.exists() && as_path.file_name().map(|n| n == "proton").unwrap_or(false) {
            return Some(as_path.to_path_buf());
        }
        return None;
    }

    // Search in Steam's common folder for a matching Proton installation
    let common_path = PATH_STEAM.join("steamapps/common");
    if let Ok(entries) = std::fs::read_dir(&common_path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == proton_name || name_str.to_lowercase() == proton_name.to_lowercase() {
                let proton_bin = entry.path().join("proton");
                if proton_bin.exists() {
                    return Some(proton_bin);
                }
            }
        }
    }

    // Also check Steam's compatibilitytools.d for custom Proton versions
    let compat_path = PATH_STEAM.join("compatibilitytools.d");
    if let Ok(entries) = std::fs::read_dir(&compat_path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == proton_name || name_str.to_lowercase() == proton_name.to_lowercase() {
                let proton_bin = entry.path().join("proton");
                if proton_bin.exists() {
                    return Some(proton_bin);
                }
            }
        }
    }

    None
}

fn is_mount_point(dir: &PathBuf) -> Result<bool, Box<dyn std::error::Error>> {
    if let Ok(status) = Command::new("mountpoint").arg(dir).status()
        && status.success()
    {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn fuse_overlayfs_unmount_gamedirs() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = PATH_PARTY.join("tmp");

    let Ok(entries) = std::fs::read_dir(&tmp) else {
        return Err("Failed to read directory".into());
    };

    for entry_result in entries {
        if let Ok(entry) = entry_result
            && entry.path().is_dir()
            && entry.file_name().to_string_lossy().starts_with("game-")
            && is_mount_point(&entry.path())?
        {
            let status = Command::new("umount")
                .arg("-l")
                .arg("-v")
                .arg(entry.path())
                .status()?;
            if !status.success() {
                return Err(format!("Unmounting {} failed", entry.path().to_string_lossy()).into());
            }
        }
    }

    Ok(())
}

pub fn clear_tmp() -> Result<(), Box<dyn Error>> {
    let tmp = PATH_PARTY.join("tmp");

    if !tmp.exists() {
        return Ok(());
    }

    fuse_overlayfs_unmount_gamedirs()?;

    std::fs::remove_dir_all(&tmp)?;

    Ok(())
}

pub fn check_for_splitux_update() -> bool {
    // Try to get the latest release tag from GitHub
    if let Ok(client) = reqwest::blocking::Client::new()
        .get("https://api.github.com/repos/wunnr/splitux/releases/latest")
        .header("User-Agent", "splitux")
        .send()
    {
        if let Ok(release) = client.json::<serde_json::Value>() {
            // Extract the tag name (vX.X.X format)
            if let Some(tag_name) = release["tag_name"].as_str() {
                // Strip the 'v' prefix
                let latest_version = tag_name.strip_prefix('v').unwrap_or(tag_name);

                // Get current version from env!
                let current_version = env!("CARGO_PKG_VERSION");

                // Compare versions using semver
                if let (Ok(latest_semver), Ok(current_semver)) = (
                    semver::Version::parse(latest_version),
                    semver::Version::parse(current_version),
                ) {
                    return latest_semver > current_semver;
                }
            }
        }
    }

    // Default to false if any part of the process fails
    false
}

// KWin D-Bus functions have been moved to src/wm/kwin.rs

pub trait SanitizePath {
    fn sanitize_path(&self) -> String;
}

impl SanitizePath for String {
    fn sanitize_path(&self) -> String {
        if self.is_empty() {
            return String::new();
        }

        let mut sanitized = self.clone();

        // Remove potentially dangerous characters
        // Allow single quotes in paths since they are quoted when launching
        // commands. Double quotes would break the quoting though, so we still
        // strip those along with other potentially dangerous characters.
        let chars_to_sanitize = [';', '&', '|', '$', '`', '(', ')', '<', '>', '"', '\\', '/'];

        if chars_to_sanitize.iter().any(|&c| sanitized.contains(c)) {
            sanitized = sanitized
                .replace(";", "")
                .replace("&", "")
                .replace("|", "")
                .replace("$", "")
                .replace("`", "")
                .replace("(", "")
                .replace(")", "")
                .replace("<", "")
                .replace(">", "")
                .replace("\"", "")
                .replace("\\", "/") // Convert Windows backslashes to forward slashes
                .replace("//", "/"); // Remove any doubled slashes
        }

        // Prevent path traversal attacks
        while sanitized.contains("../") || sanitized.contains("./") {
            sanitized = sanitized.replace("../", "").replace("./", "");
        }

        // Remove leading slash to allow joining with other paths
        if sanitized.starts_with('/') {
            sanitized = sanitized[1..].to_string();
        }

        sanitized
    }
}

pub trait OsFmt {
    fn os_fmt(&self, win: bool) -> String;
}

impl OsFmt for String {
    fn os_fmt(&self, win: bool) -> String {
        if !win {
            return self.clone();
        } else {
            let path_fmt = self.replace("/", "\\");
            format!("Z:{}", path_fmt)
        }
    }
}

impl OsFmt for PathBuf {
    fn os_fmt(&self, win: bool) -> String {
        if !win {
            return self.to_string_lossy().to_string();
        } else {
            let path_fmt = self.to_string_lossy().replace("/", "\\");
            format!("Z:{}", path_fmt)
        }
    }
}
