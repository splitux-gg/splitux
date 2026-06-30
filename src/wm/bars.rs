//! Shared status bar management for Wayland compositors
//!
//! Handles hiding/restoring common status bars (waybar, ags, eww, polybar)
//! so game windows can use the full screen.
//!
//! All bars are killed on hide and restarted on restore using their original
//! command line from /proc. This is deterministic — unlike SIGUSR1 toggling,
//! kill/restart always produces the correct end state.
//!
//! Bar state is persisted to disk so bars can be restored even after
//! abnormal termination (Ctrl+C, crash, SIGKILL).

use crate::paths::PATH_PARTY;
use std::process::Command;
use std::sync::OnceLock;

/// Whether `systemd-run --user` is usable for launching bars into their own
/// independent scopes (probed once).
fn systemd_run_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("systemd-run")
            .args(["--user", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Env vars a graphical bar needs, forwarded into its transient service (a
/// bare `--user` service otherwise inherits only the user manager's env, which
/// may lack the Wayland/X11 session vars).
const BAR_ENV_PASSTHROUGH: &[&str] = &[
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "NIRI_SOCKET",
    "DISPLAY",
    "PATH",
    "XDG_CURRENT_DESKTOP",
    "HYPRLAND_INSTANCE_SIGNATURE",
    "DBUS_SESSION_BUS_ADDRESS",
];

/// Launch a status bar so it outlives whatever restored it.
///
/// Restored bars must NOT be cgroup children of the restorer (the splitux GUI —
/// itself now in a transient scope — or the detached restore-on-death watcher
/// service), or systemd reaps them when the restorer's unit stops. We launch
/// each bar as its own transient `--user` **service**: `systemd-run` registers
/// it and returns immediately, leaving the bar running fully detached under the
/// user manager (it cannot be reaped by the restorer's teardown). Falls back to
/// a plain spawn when systemd-user is unavailable.
fn spawn_bar(program: &str, args: &[String]) {
    let base = std::path::Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("bar");

    // IDEMPOTENT: never start a bar that's already running — whether the user's
    // own bar, or one a racing restorer/teardown already brought back. This is
    // what stops the "double waybar" after a force-kill, where more than one
    // restore path (a session's in-process restore_all + the restore-on-death
    // watcher) can fire for the same bar.
    if !StatusBarManager::get_pids(base).is_empty() {
        println!("[splitux] wm::bars - {base} already running, not restarting");
        return;
    }

    if systemd_run_available() {
        // DETERMINISTIC unit name (no restorer pid): two restorers racing both
        // try the SAME unit, so systemd registers it once and the loser's
        // `--unit` fails instead of spawning a second bar. `--collect` frees the
        // name when the bar exits, so the next session can restart it.
        let unit = format!("splitux-bar-{base}.service");
        let mut cmd = Command::new("systemd-run");
        cmd.args([
            "--user",
            "--quiet",
            "--collect",
            &format!("--unit={unit}"),
        ]);
        for var in BAR_ENV_PASSTHROUGH {
            if let Ok(val) = std::env::var(var) {
                cmd.arg(format!("--setenv={var}={val}"));
            }
        }
        cmd.arg("--").arg(program).args(args);
        // `--user` service registration returns promptly; status() ensures it's
        // registered before we move on (and exit).
        if let Ok(s) = cmd.status() {
            if s.success() {
                return;
            }
        }
        // systemd-run failed — either systemd-user is unavailable, OR a racer
        // already registered the unit (the bar is up now). Only fall back to a
        // direct spawn if the bar is STILL not running, so we never double it.
        if !StatusBarManager::get_pids(base).is_empty() {
            return;
        }
        eprintln!(
            "[splitux] wm::bars - systemd-run service failed for {program}, spawning directly"
        );
    }
    let _ = Command::new(program).args(args).spawn();
}

/// Tracks which bars have been hidden and how to restore them
#[derive(Default)]
pub struct StatusBarManager {
    hidden_bars: Vec<HiddenBar>,
}

/// A bar that was killed, with its original command line for restart
struct HiddenBar {
    /// Display name (e.g. "waybar")
    name: String,
    /// Full command line captured from /proc before killing: (program, args)
    cmdline: Vec<String>,
}

/// Path to the persisted bar state file
fn state_file() -> std::path::PathBuf {
    PATH_PARTY.join("tmp/hidden_bars.json")
}

/// Known status bars to look for
const KNOWN_BARS: &[&str] = &[
    "waybar",
    ".waybar-wrapped", // NixOS
    "ags",
    "eww",
    "polybar",
];

impl StatusBarManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get PIDs for a process name
    fn get_pids(name: &str) -> Vec<u32> {
        Command::new("pgrep")
            .arg("-x")
            .arg(name)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(
                        String::from_utf8_lossy(&o.stdout)
                            .split_whitespace()
                            .filter_map(|s| s.parse().ok())
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    /// Read a process's command line from /proc/<pid>/cmdline
    fn read_cmdline(pid: u32) -> Option<Vec<String>> {
        let data = std::fs::read(format!("/proc/{}/cmdline", pid)).ok()?;
        let parts: Vec<String> = data
            .split(|&b| b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect();
        if parts.is_empty() { None } else { Some(parts) }
    }

    /// Persist hidden bar state to disk so it survives abnormal termination
    fn persist_state(&self) {
        if self.hidden_bars.is_empty() {
            return;
        }

        let entries: Vec<Vec<&str>> = self
            .hidden_bars
            .iter()
            .map(|b| b.cmdline.iter().map(|s| s.as_str()).collect())
            .collect();

        let json = match serde_json::to_string(&entries) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("[splitux] wm::bars - Failed to serialize bar state: {}", e);
                return;
            }
        };

        let path = state_file();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&path, json) {
            eprintln!("[splitux] wm::bars - Failed to persist bar state: {}", e);
        }
    }

    /// Remove persisted bar state from disk
    fn clear_state() {
        let _ = std::fs::remove_file(state_file());
    }

    /// Hide all detected status bars by killing them
    pub fn hide_all(&mut self) {
        for &name in KNOWN_BARS {
            let pids = Self::get_pids(name);
            if pids.is_empty() {
                continue;
            }

            println!("[splitux] wm::bars - Found running bar: {} (PIDs: {:?})", name, pids);

            // Capture the command line from the first PID before killing
            let cmdline = Self::read_cmdline(pids[0]).unwrap_or_else(|| vec![name.to_string()]);

            println!("[splitux] wm::bars - Killing {} (cmdline: {:?})", name, cmdline);
            let _ = Command::new("pkill").arg("-x").arg(name).status();

            self.hidden_bars.push(HiddenBar {
                name: name.to_string(),
                cmdline,
            });
        }

        if self.hidden_bars.is_empty() {
            println!("[splitux] wm::bars - No status bars detected");
        } else {
            self.persist_state();
        }
    }

    /// Restore all previously hidden bars by restarting them
    pub fn restore_all(&mut self) {
        if self.hidden_bars.is_empty() {
            return;
        }

        println!("[splitux] wm::bars - Restoring {} status bar(s)", self.hidden_bars.len());

        for bar in &self.hidden_bars {
            println!("[splitux] wm::bars - Restarting {} (cmdline: {:?})", bar.name, bar.cmdline);

            let (program, args) = match bar.cmdline.split_first() {
                Some((prog, rest)) => (prog.as_str(), rest),
                None => continue,
            };

            spawn_bar(program, args);
        }

        self.hidden_bars.clear();
        Self::clear_state();
    }

    /// Check if any bars are currently hidden
    #[allow(dead_code)]
    pub fn has_hidden_bars(&self) -> bool {
        !self.hidden_bars.is_empty()
    }
}

/// Restore bars from a previous session that was interrupted (Ctrl+C, crash, etc.)
///
/// Reads persisted state from disk. If bars were hidden and never restored,
/// restarts them now. Safe to call at startup — does nothing if no state file exists.
pub fn restore_from_previous_session() {
    let path = state_file();
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return, // No state file = nothing to restore
    };

    let cmdlines: Vec<Vec<String>> = match serde_json::from_str(&data) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[splitux] wm::bars - Failed to parse bar state: {}", e);
            let _ = std::fs::remove_file(&path);
            return;
        }
    };

    if cmdlines.is_empty() {
        let _ = std::fs::remove_file(&path);
        return;
    }

    println!(
        "[splitux] wm::bars - Restoring {} bar(s) from previous session",
        cmdlines.len()
    );

    for cmdline in &cmdlines {
        let (program, args) = match cmdline.split_first() {
            Some((prog, rest)) => (prog.as_str(), rest),
            None => continue,
        };

        // Only restart if the bar isn't already running
        let name = std::path::Path::new(program)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(program);

        if !StatusBarManager::get_pids(name).is_empty() {
            println!("[splitux] wm::bars - {} already running, skipping", name);
            continue;
        }

        println!("[splitux] wm::bars - Restarting {} (cmdline: {:?})", name, cmdline);
        spawn_bar(program, args);
    }

    let _ = std::fs::remove_file(&path);
}
