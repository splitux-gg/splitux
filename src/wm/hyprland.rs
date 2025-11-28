//! Hyprland window manager integration via hyprctl IPC socket

use crate::monitor::Monitor;
use crate::wm::{LayoutContext, LayoutOrientation, NestedSession, WindowManager, WmResult};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;

/// Hyprland monitor info from IPC
#[derive(Debug, Clone)]
struct HyprMonitor {
    name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

pub struct HyprlandManager {
    socket_path: Option<PathBuf>,
    rules_added: bool,
    target_monitor: Option<String>,
    hidden_bars: Vec<String>, // Track which bars we've hidden (waybar, ags, eww, etc.)
}

impl HyprlandManager {
    pub fn new() -> Self {
        Self {
            socket_path: Self::find_socket(),
            rules_added: false,
            target_monitor: None,
            hidden_bars: Vec::new(),
        }
    }

    fn find_socket() -> Option<PathBuf> {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
        let signature = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
        let socket_path = PathBuf::from(&runtime_dir)
            .join("hypr")
            .join(&signature)
            .join(".socket.sock");

        if socket_path.exists() {
            Some(socket_path)
        } else {
            let alt_path = PathBuf::from(&runtime_dir)
                .join("hypr")
                .join(format!("{}/.socket.sock", signature));
            if alt_path.exists() {
                Some(alt_path)
            } else {
                None
            }
        }
    }

    fn hyprctl(&self, command: &str) -> WmResult<String> {
        let socket_path = self
            .socket_path
            .as_ref()
            .ok_or("Hyprland socket not found")?;

        let mut stream = UnixStream::connect(socket_path)?;
        stream.write_all(command.as_bytes())?;
        stream.flush()?;

        let mut response = String::new();
        stream.read_to_string(&mut response)?;

        Ok(response)
    }

    fn hyprctl_batch(&self, commands: &[String]) -> WmResult<()> {
        let batch = format!("[[BATCH]] {}", commands.join(" ; "));
        let _ = self.hyprctl(&batch)?;
        Ok(())
    }

    /// Get monitor info from Hyprland IPC
    fn get_monitors(&self) -> WmResult<Vec<HyprMonitor>> {
        let response = self.hyprctl("j/monitors")?;
        let monitors: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| format!("Failed to parse monitors: {}", e))?;

        let mut result = Vec::new();
        if let Some(arr) = monitors.as_array() {
            for mon in arr {
                result.push(HyprMonitor {
                    name: mon["name"].as_str().unwrap_or("").to_string(),
                    x: mon["x"].as_i64().unwrap_or(0) as i32,
                    y: mon["y"].as_i64().unwrap_or(0) as i32,
                    width: mon["width"].as_u64().unwrap_or(1920) as u32,
                    height: mon["height"].as_u64().unwrap_or(1080) as u32,
                });
            }
        }
        Ok(result)
    }

    /// Find Hyprland monitor by index (matching splitux monitor order)
    fn get_monitor_by_index(&self, index: usize) -> WmResult<HyprMonitor> {
        let monitors = self.get_monitors()?;
        monitors
            .into_iter()
            .nth(index)
            .ok_or_else(|| format!("Monitor index {} not found", index).into())
    }

    /// Add window rules for gamescope windows
    /// Uses broad regex to catch various gamescope window classes
    fn add_window_rules(&mut self, target_monitor: &str) -> WmResult<()> {
        self.target_monitor = Some(target_monitor.to_string());

        // Use broader regex to catch all gamescope variants
        // gamescope can report as: gamescope, gamescope-kbm, Gamescope, etc.
        let class_match = "class:^([Gg]amescope.*)$";

        let commands = vec![
            // Float windows so we can position them precisely
            format!("keyword windowrulev2 float,{}", class_match),
            // Remove decorations for clean look
            format!("keyword windowrulev2 noborder,{}", class_match),
            format!("keyword windowrulev2 noblur,{}", class_match),
            format!("keyword windowrulev2 noshadow,{}", class_match),
            format!("keyword windowrulev2 noanim,{}", class_match),
            // Prevent inactive window dimming - CRITICAL for split-screen
            format!("keyword windowrulev2 opaque,{}", class_match),
            format!("keyword windowrulev2 nodim,{}", class_match),
            // Force RGB (ignore alpha channel) - helps with some rendering issues
            format!("keyword windowrulev2 forcergbx,{}", class_match),
            // Pin windows so they appear on all workspaces and above other windows
            format!("keyword windowrulev2 pin,{}", class_match),
            // Move to target monitor
            format!("keyword windowrulev2 monitor {},{}", target_monitor, class_match),
        ];

        println!(
            "[splitux] wm::hyprland - Adding window rules for monitor {} (class pattern: {})",
            target_monitor, class_match
        );

        self.hyprctl_batch(&commands)?;
        self.rules_added = true;

        Ok(())
    }

    /// Apply visual properties directly to a window via setprop
    /// This guarantees the effect regardless of windowrule matching
    fn apply_window_props(&self, address: &str) -> WmResult<()> {
        // Use setprop with 'lock' to prevent dynamic rules from overriding
        let commands = vec![
            // Force disable blur
            format!("setprop address:{} forcenoblur 1 lock", address),
            // Force opaque (no transparency)
            format!("setprop address:{} forceopaque 1 lock", address),
            // Force disable animations
            format!("setprop address:{} forcenoanims 1 lock", address),
            // Force disable border
            format!("setprop address:{} forcenoborder 1 lock", address),
            // Force disable shadow
            format!("setprop address:{} forcenoshadow 1 lock", address),
            // Set full alpha (1.0 = fully visible)
            format!("setprop address:{} alpha 1.0 lock", address),
            // Set inactive alpha to full as well
            format!("setprop address:{} alphainactive 1.0 lock", address),
        ];

        self.hyprctl_batch(&commands)
    }

    /// Get list of gamescope window addresses with their class names for debugging
    fn get_gamescope_windows(&self) -> WmResult<Vec<(String, String)>> {
        let response = self.hyprctl("j/clients")?;
        let clients: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| format!("Failed to parse clients: {}", e))?;

        let mut windows = Vec::new();
        if let Some(arr) = clients.as_array() {
            for client in arr {
                let class = client["class"].as_str().unwrap_or("");
                let class_lower = class.to_lowercase();

                // Match any gamescope variant (case-insensitive)
                if class_lower.starts_with("gamescope") {
                    if let Some(addr) = client["address"].as_str() {
                        windows.push((addr.to_string(), class.to_string()));
                    }
                }
            }
        }
        Ok(windows)
    }

    /// Calculate and apply window positions on the target monitor
    fn position_windows(&self, ctx: &LayoutContext) -> WmResult<()> {
        let windows = self.get_gamescope_windows()?;
        if windows.is_empty() {
            return Err("No gamescope windows found".into());
        }

        // Get the target monitor from Hyprland (use first instance's monitor index)
        let monitor_index = ctx.instances.first().map(|i| i.monitor).unwrap_or(0);
        let hypr_mon = self.get_monitor_by_index(monitor_index)?;

        println!(
            "[splitux] wm::hyprland - Target monitor: {} at {}x{}+{}+{}",
            hypr_mon.name, hypr_mon.width, hypr_mon.height, hypr_mon.x, hypr_mon.y
        );

        // Log detected windows for debugging
        println!(
            "[splitux] wm::hyprland - Found {} gamescope windows:",
            windows.len()
        );
        for (addr, class) in &windows {
            println!("[splitux] wm::hyprland   - {} (class: {})", addr, class);
        }

        // Use full monitor area (windows are pinned so they cover waybar)
        let player_count = windows.len().min(4);
        let mut commands = Vec::new();

        for (i, (addr, class)) in windows.iter().enumerate() {
            let (x, y, w, h) = self.calculate_slot(
                player_count,
                i,
                hypr_mon.x,
                hypr_mon.y,
                hypr_mon.width,
                hypr_mon.height,
                ctx.orientation,
            );

            println!(
                "[splitux] wm::hyprland - Window {} ({}) -> {}x{}+{}+{}",
                addr, class, w, h, x, y
            );

            // Move and resize
            commands.push(format!(
                "dispatch movewindowpixel exact {} {},address:{}",
                x, y, addr
            ));
            commands.push(format!(
                "dispatch resizewindowpixel exact {} {},address:{}",
                w, h, addr
            ));
        }

        self.hyprctl_batch(&commands)?;

        // Apply visual properties directly to each window via setprop
        // This ensures no dimming/blur even if windowrules didn't match
        println!("[splitux] wm::hyprland - Applying visual properties to windows...");
        for (addr, _) in &windows {
            if let Err(e) = self.apply_window_props(addr) {
                println!(
                    "[splitux] wm::hyprland - Warning: Failed to apply props to {}: {}",
                    addr, e
                );
            }
        }

        Ok(())
    }

    /// Calculate position for a player slot
    fn calculate_slot(
        &self,
        player_count: usize,
        player_index: usize,
        mon_x: i32,
        mon_y: i32,
        mon_w: u32,
        mon_h: u32,
        orientation: LayoutOrientation,
    ) -> (i32, i32, u32, u32) {
        match (player_count, orientation) {
            // 1 player: fullscreen
            (1, _) => (mon_x, mon_y, mon_w, mon_h),

            // 2 players horizontal: top/bottom
            (2, LayoutOrientation::Horizontal) => {
                let h = mon_h / 2;
                let y = if player_index == 0 {
                    mon_y
                } else {
                    mon_y + h as i32
                };
                (mon_x, y, mon_w, h)
            }

            // 2 players vertical: left/right
            (2, LayoutOrientation::Vertical) => {
                let w = mon_w / 2;
                let x = if player_index == 0 {
                    mon_x
                } else {
                    mon_x + w as i32
                };
                (x, mon_y, w, mon_h)
            }

            // 3 players: top full, bottom split
            (3, _) => {
                let h = mon_h / 2;
                let w = mon_w / 2;
                match player_index {
                    0 => (mon_x, mon_y, mon_w, h),
                    1 => (mon_x, mon_y + h as i32, w, h),
                    _ => (mon_x + w as i32, mon_y + h as i32, w, h),
                }
            }

            // 4 players: 2x2 grid
            (4, _) => {
                let w = mon_w / 2;
                let h = mon_h / 2;
                let (col, row) = (player_index % 2, player_index / 2);
                (
                    mon_x + (col as u32 * w) as i32,
                    mon_y + (row as u32 * h) as i32,
                    w,
                    h,
                )
            }

            // Fallback
            _ => (mon_x, mon_y, mon_w, mon_h),
        }
    }

    fn remove_window_rules(&mut self) -> WmResult<()> {
        if !self.rules_added {
            return Ok(());
        }

        println!("[splitux] wm::hyprland - Removing window rules");

        let commands =
            vec!["keyword windowrulev2 unset,class:^(gamescope|gamescope-kbm)$".to_string()];

        self.hyprctl_batch(&commands)?;
        self.rules_added = false;

        Ok(())
    }

    /// Hide status bars that might overlay game windows
    /// Supports: waybar, ags, eww, polybar, and other common bars
    /// Uses multiple methods for robustness
    fn hide_status_bars(&mut self) -> WmResult<()> {
        // Common status bars and their hide methods
        // (name, toggle_signal, kill_to_hide)
        let bars: &[(&str, Option<&str>, bool)] = &[
            ("waybar", Some("-SIGUSR1"), false),      // SIGUSR1 toggles visibility
            (".waybar-wrapped", Some("-SIGUSR1"), false), // NixOS wrapped waybar
            ("ags", Some("-SIGUSR1"), false),         // AGS also uses SIGUSR1
            ("eww", None, true),                      // eww needs to be killed
            ("polybar", Some("-SIGUSR1"), false),     // polybar toggle
        ];

        for (bar_name, toggle_signal, kill_to_hide) in bars {
            // Check if this bar is running
            let is_running = Command::new("pgrep")
                .arg("-x")
                .arg(bar_name)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if !is_running {
                continue;
            }

            println!("[splitux] wm::hyprland - Found running bar: {}", bar_name);

            if *kill_to_hide {
                // Some bars need to be killed entirely
                println!("[splitux] wm::hyprland - Killing {} (will restart on teardown)", bar_name);
                let _ = Command::new("pkill").arg("-x").arg(bar_name).status();
                self.hidden_bars.push(format!("kill:{}", bar_name));
            } else if let Some(signal) = toggle_signal {
                // Send toggle signal
                println!("[splitux] wm::hyprland - Sending {} to {}", signal, bar_name);
                let result = Command::new("pkill")
                    .args([signal, bar_name])
                    .status();

                if result.is_ok() {
                    self.hidden_bars.push(format!("toggle:{}", bar_name));
                }
            }
        }

        // Also apply layer rules to push any remaining layer surfaces below
        // This catches bars we might have missed
        self.apply_layer_rules()?;

        if self.hidden_bars.is_empty() {
            println!("[splitux] wm::hyprland - No status bars detected");
        }

        Ok(())
    }

    /// Apply layer rules to ensure game windows appear above layer surfaces
    fn apply_layer_rules(&self) -> WmResult<()> {
        // Try to apply layer rules that might help with z-ordering
        // Note: Not all of these may work depending on Hyprland version
        let commands = vec![
            // Try to set waybar/bar layers to bottom (below normal windows)
            "keyword layerrule noanim,waybar".to_string(),
            "keyword layerrule noanim,gtk-layer-shell".to_string(),
        ];

        // These are best-effort, don't fail if they don't work
        let _ = self.hyprctl_batch(&commands);
        Ok(())
    }

    /// Restore status bars that were hidden
    fn restore_status_bars(&mut self) -> WmResult<()> {
        if self.hidden_bars.is_empty() {
            return Ok(());
        }

        println!("[splitux] wm::hyprland - Restoring {} status bar(s)", self.hidden_bars.len());

        for bar_entry in &self.hidden_bars {
            if let Some(bar_name) = bar_entry.strip_prefix("toggle:") {
                // Re-toggle to show
                println!("[splitux] wm::hyprland - Toggling {} back on", bar_name);
                let _ = Command::new("pkill")
                    .args(["-SIGUSR1", bar_name])
                    .status();
            } else if let Some(bar_name) = bar_entry.strip_prefix("kill:") {
                // Restart the bar
                println!("[splitux] wm::hyprland - Restarting {}", bar_name);
                let _ = Command::new(bar_name)
                    .spawn();
            }
        }

        self.hidden_bars.clear();
        Ok(())
    }
}

impl Default for HyprlandManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowManager for HyprlandManager {
    fn name(&self) -> &'static str {
        "Hyprland"
    }

    fn setup(&mut self, ctx: &LayoutContext) -> WmResult<()> {
        println!("[splitux] wm::hyprland - Setting up window rules");

        // Get target monitor name from Hyprland
        let monitor_index = ctx.instances.first().map(|i| i.monitor).unwrap_or(0);
        let hypr_mon = self.get_monitor_by_index(monitor_index)?;

        // Hide status bars so game windows can use full screen
        self.hide_status_bars()?;

        self.add_window_rules(&hypr_mon.name)
    }

    fn on_instances_launched(&mut self, ctx: &LayoutContext) -> WmResult<()> {
        println!("[splitux] wm::hyprland - Waiting for gamescope windows...");

        let expected_count = ctx.instances.len();
        let max_wait = std::time::Duration::from_secs(120);
        let poll_interval = std::time::Duration::from_millis(500);
        let start = std::time::Instant::now();

        loop {
            let windows = self.get_gamescope_windows().unwrap_or_default();

            if windows.len() >= expected_count {
                println!(
                    "[splitux] wm::hyprland - Found {} windows after {:.1}s",
                    windows.len(),
                    start.elapsed().as_secs_f32()
                );
                // Give windows a moment to fully initialize
                std::thread::sleep(std::time::Duration::from_millis(500));
                break;
            }

            if start.elapsed() > max_wait {
                println!(
                    "[splitux] wm::hyprland - Timeout waiting for windows ({}/{})",
                    windows.len(),
                    expected_count
                );
                break;
            }

            // Log progress periodically
            if start.elapsed().as_secs() % 5 == 0 && start.elapsed().as_millis() % 500 < 100 {
                println!(
                    "[splitux] wm::hyprland - Still waiting... ({}/{} windows)",
                    windows.len(),
                    expected_count
                );
            }

            std::thread::sleep(poll_interval);
        }

        // Position windows on the target monitor and apply visual properties
        self.position_windows(ctx)
    }

    fn teardown(&mut self) -> WmResult<()> {
        println!("[splitux] wm::hyprland - Tearing down");
        self.remove_window_rules()?;
        self.restore_status_bars()
    }

    fn is_available() -> bool {
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() && Self::find_socket().is_some()
    }

    fn is_reactive(&self) -> bool {
        false
    }
}

impl NestedSession for HyprlandManager {
    fn nested_session_command(&self, _splitux_args: &[String], _monitor: &Monitor) -> Command {
        let mut cmd = Command::new("Hyprland");
        cmd.env("WLR_BACKENDS", "wayland");
        cmd
    }
}
