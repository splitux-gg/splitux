//! Shared status bar management for Wayland compositors
//!
//! Handles hiding/restoring common status bars (waybar, ags, eww, polybar)
//! so game windows can use the full screen.
//!
//! All bars are killed on hide and restarted on restore using their original
//! command line from /proc. This is deterministic — unlike SIGUSR1 toggling,
//! kill/restart always produces the correct end state.

use std::process::Command;

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

            match Command::new(program).args(args).spawn() {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("[splitux] wm::bars - Failed to restart {}: {}", bar.name, e);
                }
            }
        }

        self.hidden_bars.clear();
    }

    /// Check if any bars are currently hidden
    #[allow(dead_code)]
    pub fn has_hidden_bars(&self) -> bool {
        !self.hidden_bars.is_empty()
    }
}
