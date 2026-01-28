use crate::paths::{PATH_HOME, PATH_PARTY};

use dialog::{Choice, DialogBox};

/// Check if we're running in a Wayland session.
///
/// Returns true if WAYLAND_DISPLAY is set, indicating the session is Wayland-native.
/// X11 sessions (including XWayland apps on Wayland) won't have this set.
pub fn is_wayland_session() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

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

/// Get all Steam library folders from libraryfolders.vdf
fn get_steam_library_folders() -> Vec<PathBuf> {
    use crate::paths::PATH_STEAM;

    let mut folders = vec![PATH_STEAM.clone()];

    // Read libraryfolders.vdf to find additional Steam libraries
    let vdf_path = PATH_STEAM.join("steamapps/libraryfolders.vdf");
    if let Ok(content) = std::fs::read_to_string(&vdf_path) {
        // Simple parsing: look for "path" entries
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("\"path\"") {
                // Extract path value: "path"		"/some/path"
                if let Some(start) = trimmed.rfind('"') {
                    let before_last = &trimmed[..start];
                    if let Some(path_start) = before_last.rfind('"') {
                        let path_str = &before_last[path_start + 1..];
                        let path = PathBuf::from(path_str);
                        if path.exists() && !folders.contains(&path) {
                            folders.push(path);
                        }
                    }
                }
            }
        }
    }

    folders
}

/// Search for Proton in a directory's common folder
fn find_proton_in_common(base_path: &PathBuf, proton_name: &str) -> Option<PathBuf> {
    let common_path = base_path.join("steamapps/common");
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
    None
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

    // Get all Steam library folders and search each one
    let library_folders = get_steam_library_folders();
    for folder in &library_folders {
        if let Some(proton_bin) = find_proton_in_common(folder, proton_name) {
            return Some(proton_bin);
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

/// Kill all gamescope-splitux processes
/// These are always Splitux-spawned, so safe to kill unconditionally
fn kill_orphaned_gamescope_splitux() -> Result<(), Box<dyn Error>> {
    // pgrep truncates process names to 15 chars, so "gamescope-splitux" becomes "gamescope-split"
    let output = Command::new("pgrep")
        .args(["-x", "gamescope-split"])
        .output()?;

    if output.status.success() {
        let pids = String::from_utf8_lossy(&output.stdout);
        let count = pids.lines().filter(|l| !l.is_empty()).count();
        if count > 0 {
            println!(
                "[splitux] Cleaning up {} orphaned gamescope-splitux process(es)",
                count
            );
            Command::new("pkill")
                .args(["-9", "-x", "gamescope-split"])
                .status()?;
            // Give processes time to terminate
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    Ok(())
}

/// Kill orphaned bwrap processes that were spawned by Splitux
/// Identifies Splitux bwrap by checking cmdline for Splitux-specific paths
fn kill_orphaned_splitux_bwrap() -> Result<(), Box<dyn Error>> {
    // Get all bwrap PIDs
    let output = Command::new("pgrep").args(["-x", "bwrap"]).output()?;

    if !output.status.success() {
        return Ok(()); // No bwrap processes
    }

    let pids_str = String::from_utf8_lossy(&output.stdout);

    // Markers that identify Splitux-spawned bwrap processes
    let splitux_markers = ["splitux/tmp", "splitux/profiles", "/splitux/"];

    for pid in pids_str.lines() {
        let pid = pid.trim();
        if pid.is_empty() {
            continue;
        }

        // Read cmdline to check for Splitux markers
        let cmdline_path = format!("/proc/{}/cmdline", pid);
        let Ok(cmdline_raw) = std::fs::read_to_string(&cmdline_path) else {
            continue;
        };
        // cmdline uses null bytes as separators
        let cmdline = cmdline_raw.replace('\0', " ");

        let is_splitux_bwrap = splitux_markers.iter().any(|marker| cmdline.contains(marker));

        if !is_splitux_bwrap {
            continue;
        }

        // Check if it's orphaned (parent PID is 1)
        let stat_path = format!("/proc/{}/stat", pid);
        let Ok(stat) = std::fs::read_to_string(&stat_path) else {
            continue;
        };

        // stat format: pid (comm) state ppid ...
        // We need to find ppid which is after the closing parenthesis
        let Some(paren_end) = stat.rfind(')') else {
            continue;
        };
        let after_comm = &stat[paren_end + 1..];
        let fields: Vec<&str> = after_comm.split_whitespace().collect();
        // fields[0] is state, fields[1] is ppid
        let Some(ppid) = fields.get(1) else {
            continue;
        };

        if *ppid == "1" {
            // Orphaned - safe to kill
            println!("[splitux] Killing orphaned Splitux bwrap (PID {})", pid);
            let _ = Command::new("kill").args(["-9", pid]).status();
        }
    }

    Ok(())
}

/// Clean up all orphaned Splitux processes
/// Safe to call at startup and before launches
pub fn cleanup_orphaned_processes() {
    println!("[splitux] Checking for orphaned processes...");

    if let Err(e) = kill_orphaned_gamescope_splitux() {
        println!(
            "[splitux] Warning: Failed to clean gamescope-splitux: {}",
            e
        );
    }

    if let Err(e) = kill_orphaned_splitux_bwrap() {
        println!("[splitux] Warning: Failed to clean orphaned bwrap: {}", e);
    }
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
