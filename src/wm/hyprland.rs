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
    waybar_hidden: bool,
}

impl HyprlandManager {
    pub fn new() -> Self {
        Self {
            socket_path: Self::find_socket(),
            rules_added: false,
            target_monitor: None,
            waybar_hidden: false,
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
    fn add_window_rules(&mut self, target_monitor: &str) -> WmResult<()> {
        self.target_monitor = Some(target_monitor.to_string());

        let commands = vec![
            // Float windows so we can position them precisely
            "keyword windowrulev2 float,class:^(gamescope|gamescope-kbm)$".to_string(),
            // Remove decorations for clean look
            "keyword windowrulev2 noborder,class:^(gamescope|gamescope-kbm)$".to_string(),
            "keyword windowrulev2 noblur,class:^(gamescope|gamescope-kbm)$".to_string(),
            "keyword windowrulev2 noshadow,class:^(gamescope|gamescope-kbm)$".to_string(),
            "keyword windowrulev2 noanim,class:^(gamescope|gamescope-kbm)$".to_string(),
            // Prevent inactive window dimming - keep full opacity even when unfocused
            "keyword windowrulev2 opaque,class:^(gamescope|gamescope-kbm)$".to_string(),
            "keyword windowrulev2 nodim,class:^(gamescope|gamescope-kbm)$".to_string(),
            // Move to target monitor
            format!(
                "keyword windowrulev2 monitor {},class:^(gamescope|gamescope-kbm)$",
                target_monitor
            ),
        ];

        println!(
            "[splitux] wm::hyprland - Adding window rules for monitor {}",
            target_monitor
        );

        self.hyprctl_batch(&commands)?;
        self.rules_added = true;

        Ok(())
    }

    /// Get list of gamescope window addresses
    fn get_gamescope_windows(&self) -> WmResult<Vec<String>> {
        let response = self.hyprctl("j/clients")?;
        let clients: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| format!("Failed to parse clients: {}", e))?;

        let mut addresses = Vec::new();
        if let Some(arr) = clients.as_array() {
            for client in arr {
                let class = client["class"].as_str().unwrap_or("");
                if class == "gamescope" || class == "gamescope-kbm" || class.starts_with("gamescope")
                {
                    if let Some(addr) = client["address"].as_str() {
                        addresses.push(addr.to_string());
                    }
                }
            }
        }
        Ok(addresses)
    }

    /// Calculate and apply window positions on the target monitor
    fn position_windows(&self, ctx: &LayoutContext) -> WmResult<()> {
        let addresses = self.get_gamescope_windows()?;
        if addresses.is_empty() {
            return Err("No gamescope windows found".into());
        }

        // Get the target monitor from Hyprland (use first instance's monitor index)
        let monitor_index = ctx.instances.first().map(|i| i.monitor).unwrap_or(0);
        let hypr_mon = self.get_monitor_by_index(monitor_index)?;

        println!(
            "[splitux] wm::hyprland - Target monitor: {} at {}x{}+{}+{}",
            hypr_mon.name, hypr_mon.width, hypr_mon.height, hypr_mon.x, hypr_mon.y
        );

        // Use full monitor area (windows are pinned so they cover waybar)
        let player_count = addresses.len().min(4);
        let mut commands = Vec::new();

        for (i, addr) in addresses.iter().enumerate() {
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
                "[splitux] wm::hyprland - Window {} -> {}x{}+{}+{}",
                addr, w, h, x, y
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

        self.hyprctl_batch(&commands)
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

    /// Hide waybar by sending SIGUSR1 (toggle signal)
    /// This is an ephemeral change - waybar will be restored on teardown
    fn hide_waybar(&mut self) -> WmResult<()> {
        // Check if waybar is running
        let waybar_running = Command::new("pgrep")
            .arg("-x")
            .arg("waybar")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !waybar_running {
            println!("[splitux] wm::hyprland - Waybar not running, skipping hide");
            return Ok(());
        }

        println!("[splitux] wm::hyprland - Hiding waybar (SIGUSR1 toggle)");

        // Send SIGUSR1 to toggle waybar visibility
        let result = Command::new("pkill")
            .args(["-SIGUSR1", "waybar"])
            .status();

        match result {
            Ok(status) if status.success() => {
                self.waybar_hidden = true;
                Ok(())
            }
            Ok(_) => {
                println!("[splitux] wm::hyprland - Warning: pkill waybar returned non-zero");
                Ok(()) // Non-fatal, continue anyway
            }
            Err(e) => {
                println!("[splitux] wm::hyprland - Warning: Failed to hide waybar: {}", e);
                Ok(()) // Non-fatal, continue anyway
            }
        }
    }

    /// Restore waybar by sending SIGUSR1 again (toggle back)
    fn restore_waybar(&mut self) -> WmResult<()> {
        if !self.waybar_hidden {
            return Ok(());
        }

        println!("[splitux] wm::hyprland - Restoring waybar (SIGUSR1 toggle)");

        // Send SIGUSR1 to toggle waybar visibility back
        let _ = Command::new("pkill")
            .args(["-SIGUSR1", "waybar"])
            .status();

        self.waybar_hidden = false;
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

        // Hide waybar so game windows can use full screen
        self.hide_waybar()?;

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

            std::thread::sleep(poll_interval);
        }

        // Position windows on the target monitor
        self.position_windows(ctx)
    }

    fn teardown(&mut self) -> WmResult<()> {
        println!("[splitux] wm::hyprland - Tearing down");
        self.remove_window_rules()?;
        self.restore_waybar()
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
