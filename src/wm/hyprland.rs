//! Hyprland window manager integration via hyprctl IPC socket

use crate::monitor::Monitor;
use crate::wm::bars::StatusBarManager;
use crate::wm::layout::{calculate_geometry, WindowGeometry};
use crate::wm::{LayoutContext, NestedSession, WindowManager, WmResult};
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
    bar_manager: StatusBarManager,
}

impl HyprlandManager {
    pub fn new() -> Self {
        Self {
            socket_path: Self::find_socket(),
            rules_added: false,
            target_monitor: None,
            bar_manager: StatusBarManager::new(),
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

        // Match all gamescope variants (gamescope, gamescope-splitux, Gamescope, etc.)
        let class_match = "class:^([Gg]amescope.*)$";

        let commands = vec![
            format!("keyword windowrulev2 float,{}", class_match),
            format!("keyword windowrulev2 noborder,{}", class_match),
            format!("keyword windowrulev2 noblur,{}", class_match),
            format!("keyword windowrulev2 noshadow,{}", class_match),
            format!("keyword windowrulev2 noanim,{}", class_match),
            format!("keyword windowrulev2 opaque,{}", class_match),
            format!("keyword windowrulev2 nodim,{}", class_match),
            format!("keyword windowrulev2 forcergbx,{}", class_match),
            format!("keyword windowrulev2 pin,{}", class_match),
            format!("keyword windowrulev2 monitor {},{}", target_monitor, class_match),
        ];

        println!(
            "[splitux] wm::hyprland - Adding window rules for monitor {}",
            target_monitor
        );

        self.hyprctl_batch(&commands)?;
        self.rules_added = true;

        Ok(())
    }

    /// Apply visual properties directly to a window via setprop
    fn apply_window_props(&self, address: &str) -> WmResult<()> {
        let commands = vec![
            format!("setprop address:{} forcenoblur 1 lock", address),
            format!("setprop address:{} forceopaque 1 lock", address),
            format!("setprop address:{} forcenoanims 1 lock", address),
            format!("setprop address:{} forcenoborder 1 lock", address),
            format!("setprop address:{} forcenoshadow 1 lock", address),
            format!("setprop address:{} alpha 1.0 lock", address),
            format!("setprop address:{} alphainactive 1.0 lock", address),
        ];

        self.hyprctl_batch(&commands)
    }

    /// Get list of gamescope window addresses
    fn get_gamescope_windows(&self) -> WmResult<Vec<(String, String)>> {
        let response = self.hyprctl("j/clients")?;
        let clients: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| format!("Failed to parse clients: {}", e))?;

        let mut windows = Vec::new();
        if let Some(arr) = clients.as_array() {
            for client in arr {
                let class = client["class"].as_str().unwrap_or("");
                if class.to_lowercase().starts_with("gamescope") {
                    if let Some(addr) = client["address"].as_str() {
                        windows.push((addr.to_string(), class.to_string()));
                    }
                }
            }
        }
        Ok(windows)
    }

    /// Position windows using shared layout calculations
    fn position_windows(&self, ctx: &LayoutContext) -> WmResult<()> {
        let windows = self.get_gamescope_windows()?;
        if windows.is_empty() {
            return Err("No gamescope windows found".into());
        }

        let monitor_index = ctx.instances.first().map(|i| i.monitor).unwrap_or(0);
        let hypr_mon = self.get_monitor_by_index(monitor_index)?;

        println!(
            "[splitux] wm::hyprland - Target monitor: {} at {}x{}+{}+{}",
            hypr_mon.name, hypr_mon.width, hypr_mon.height, hypr_mon.x, hypr_mon.y
        );

        println!(
            "[splitux] wm::hyprland - Found {} gamescope windows",
            windows.len()
        );

        let player_count = windows.len().min(4);
        let mut commands = Vec::new();

        for (i, (addr, class)) in windows.iter().enumerate() {
            // Use shared layout calculation
            let geom: WindowGeometry = calculate_geometry(
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
                addr, class, geom.width, geom.height, geom.x, geom.y
            );

            commands.push(format!(
                "dispatch movewindowpixel exact {} {},address:{}",
                geom.x, geom.y, addr
            ));
            commands.push(format!(
                "dispatch resizewindowpixel exact {} {},address:{}",
                geom.width, geom.height, addr
            ));
        }

        self.hyprctl_batch(&commands)?;

        // Apply visual properties to each window
        println!("[splitux] wm::hyprland - Applying visual properties...");
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

    fn remove_window_rules(&mut self) -> WmResult<()> {
        if !self.rules_added {
            return Ok(());
        }

        println!("[splitux] wm::hyprland - Removing window rules");
        let commands = vec![
            "keyword windowrulev2 unset,class:^([Gg]amescope.*)$".to_string()
        ];
        self.hyprctl_batch(&commands)?;
        self.rules_added = false;

        Ok(())
    }

    /// Apply layer rules for z-ordering (best-effort)
    fn apply_layer_rules(&self) -> WmResult<()> {
        let commands = vec![
            "keyword layerrule noanim,waybar".to_string(),
            "keyword layerrule noanim,gtk-layer-shell".to_string(),
        ];
        let _ = self.hyprctl_batch(&commands);
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
        println!("[splitux] wm::hyprland - Setting up");

        let monitor_index = ctx.instances.first().map(|i| i.monitor).unwrap_or(0);
        let hypr_mon = self.get_monitor_by_index(monitor_index)?;

        // Hide status bars using shared manager
        self.bar_manager.hide_all();
        self.apply_layer_rules()?;

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

            if start.elapsed().as_secs() % 5 == 0 && start.elapsed().as_millis() % 500 < 100 {
                println!(
                    "[splitux] wm::hyprland - Still waiting... ({}/{} windows)",
                    windows.len(),
                    expected_count
                );
            }

            std::thread::sleep(poll_interval);
        }

        self.position_windows(ctx)
    }

    fn teardown(&mut self) -> WmResult<()> {
        println!("[splitux] wm::hyprland - Tearing down");
        self.remove_window_rules()?;
        self.bar_manager.restore_all();
        Ok(())
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
