//! Shared status bar management for Wayland compositors
//!
//! Handles hiding/restoring common status bars (waybar, ags, eww, polybar)
//! so game windows can use the full screen.

use std::process::Command;

/// Tracks which bars have been hidden and how to restore them
#[derive(Default)]
pub struct StatusBarManager {
    hidden_bars: Vec<HiddenBar>,
}

enum HiddenBar {
    /// Bar hidden via signal toggle (send same signal to restore)
    Toggle { name: String, signal: String },
    /// Bar killed (needs to be restarted)
    Killed { name: String },
}

/// Known status bars and their hide methods
const KNOWN_BARS: &[BarConfig] = &[
    BarConfig { name: "waybar", signal: Some("-SIGUSR1"), kill: false },
    BarConfig { name: ".waybar-wrapped", signal: Some("-SIGUSR1"), kill: false }, // NixOS
    BarConfig { name: "ags", signal: Some("-SIGUSR1"), kill: false },
    BarConfig { name: "eww", signal: None, kill: true },
    BarConfig { name: "polybar", signal: Some("-SIGUSR1"), kill: false },
];

struct BarConfig {
    name: &'static str,
    signal: Option<&'static str>,
    kill: bool,
}

impl StatusBarManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a process is running by name
    fn is_running(name: &str) -> bool {
        Command::new("pgrep")
            .arg("-x")
            .arg(name)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Hide all detected status bars
    pub fn hide_all(&mut self) {
        for bar in KNOWN_BARS {
            if !Self::is_running(bar.name) {
                continue;
            }

            println!("[splitux] wm::bars - Found running bar: {}", bar.name);

            if bar.kill {
                println!("[splitux] wm::bars - Killing {} (will restart on teardown)", bar.name);
                let _ = Command::new("pkill").arg("-x").arg(bar.name).status();
                self.hidden_bars.push(HiddenBar::Killed {
                    name: bar.name.to_string(),
                });
            } else if let Some(signal) = bar.signal {
                println!("[splitux] wm::bars - Sending {} to {}", signal, bar.name);
                if Command::new("pkill").args([signal, bar.name]).status().is_ok() {
                    self.hidden_bars.push(HiddenBar::Toggle {
                        name: bar.name.to_string(),
                        signal: signal.to_string(),
                    });
                }
            }
        }

        if self.hidden_bars.is_empty() {
            println!("[splitux] wm::bars - No status bars detected");
        }
    }

    /// Restore all previously hidden bars
    pub fn restore_all(&mut self) {
        if self.hidden_bars.is_empty() {
            return;
        }

        println!("[splitux] wm::bars - Restoring {} status bar(s)", self.hidden_bars.len());

        for bar in &self.hidden_bars {
            match bar {
                HiddenBar::Toggle { name, signal } => {
                    println!("[splitux] wm::bars - Toggling {} back on", name);
                    let _ = Command::new("pkill").args([signal.as_str(), name.as_str()]).status();
                }
                HiddenBar::Killed { name } => {
                    println!("[splitux] wm::bars - Restarting {}", name);
                    let _ = Command::new(name).spawn();
                }
            }
        }

        self.hidden_bars.clear();
    }

    /// Check if any bars are currently hidden
    pub fn has_hidden_bars(&self) -> bool {
        !self.hidden_bars.is_empty()
    }
}
