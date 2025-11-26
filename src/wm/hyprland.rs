//! Hyprland window manager integration via hyprctl IPC socket

use crate::monitor::Monitor;
use crate::wm::{
    calculate_all_geometries, LayoutContext, NestedSession, WindowManager, WmResult,
};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;

pub struct HyprlandManager {
    socket_path: Option<PathBuf>,
    rules_added: bool,
    layout_ctx: Option<LayoutContext>,
}

impl HyprlandManager {
    pub fn new() -> Self {
        Self {
            socket_path: Self::find_socket(),
            rules_added: false,
            layout_ctx: None,
        }
    }

    fn find_socket() -> Option<PathBuf> {
        // Hyprland socket is at $XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
        let signature = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
        let socket_path = PathBuf::from(&runtime_dir)
            .join("hypr")
            .join(&signature)
            .join(".socket.sock");

        if socket_path.exists() {
            Some(socket_path)
        } else {
            // Try alternative path format (older Hyprland versions)
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
        // Hyprland supports batch commands via [[BATCH]]
        let batch = format!("[[BATCH]] {}", commands.join(" ; "));
        let _ = self.hyprctl(&batch)?;
        Ok(())
    }

    /// Add window rules for gamescope windows (float, no border, etc.)
    fn add_window_rules(&mut self, ctx: &LayoutContext) -> WmResult<()> {
        // Store context for later positioning
        self.layout_ctx = Some(ctx.clone());

        let mut commands = Vec::new();

        // Add rules for gamescope windows - these apply to all matching windows
        commands.push("keyword windowrulev2 float,class:^(gamescope|gamescope-kbm)$".to_string());
        commands.push("keyword windowrulev2 noborder,class:^(gamescope|gamescope-kbm)$".to_string());

        println!("[splitux] wm::hyprland - Adding window rules (float, noborder)");

        self.hyprctl_batch(&commands)?;
        self.rules_added = true;

        Ok(())
    }

    /// Get list of gamescope window addresses from Hyprland
    fn get_gamescope_windows(&self) -> WmResult<Vec<String>> {
        let response = self.hyprctl("j/clients")?;

        // Parse JSON response to find gamescope windows
        let clients: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| format!("Failed to parse hyprctl clients: {}", e))?;

        let mut addresses = Vec::new();

        if let Some(arr) = clients.as_array() {
            for client in arr {
                let class = client["class"].as_str().unwrap_or("");
                if class == "gamescope" || class == "gamescope-kbm" {
                    if let Some(addr) = client["address"].as_str() {
                        addresses.push(addr.to_string());
                    }
                }
            }
        }

        Ok(addresses)
    }

    /// Position all gamescope windows according to the layout
    fn position_windows(&self, ctx: &LayoutContext) -> WmResult<()> {
        let geometries = calculate_all_geometries(&ctx.instances, &ctx.monitors, ctx.orientation);
        let addresses = self.get_gamescope_windows()?;

        println!(
            "[splitux] wm::hyprland - Found {} gamescope windows, positioning {} instances",
            addresses.len(),
            geometries.len()
        );

        let mut commands = Vec::new();

        for (i, addr) in addresses.iter().enumerate() {
            if let Some(geom) = geometries.get(i) {
                // Move and resize the window
                commands.push(format!(
                    "dispatch movewindowpixel exact {} {},address:{}",
                    geom.x, geom.y, addr
                ));
                commands.push(format!(
                    "dispatch resizewindowpixel exact {} {},address:{}",
                    geom.width, geom.height, addr
                ));

                println!(
                    "[splitux] wm::hyprland - Window {} -> {}x{}+{}+{}",
                    addr, geom.width, geom.height, geom.x, geom.y
                );
            }
        }

        if !commands.is_empty() {
            self.hyprctl_batch(&commands)?;
        }

        Ok(())
    }

    /// Remove window rules added during setup
    fn remove_window_rules(&mut self) -> WmResult<()> {
        if !self.rules_added {
            return Ok(());
        }

        println!("[splitux] wm::hyprland - Removing window rules");

        // Remove all gamescope rules
        let commands = vec![
            "keyword windowrulev2 unset,class:^(gamescope|gamescope-kbm)$".to_string(),
        ];

        self.hyprctl_batch(&commands)?;
        self.rules_added = false;

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
        self.add_window_rules(ctx)
    }

    fn on_instances_launched(&mut self, ctx: &LayoutContext) -> WmResult<()> {
        println!("[splitux] wm::hyprland - Positioning windows after launch");
        // Small delay to ensure windows are registered with Hyprland
        std::thread::sleep(std::time::Duration::from_millis(500));
        self.position_windows(ctx)
    }

    fn teardown(&mut self) -> WmResult<()> {
        println!("[splitux] wm::hyprland - Tearing down");
        self.remove_window_rules()
    }

    fn is_available() -> bool {
        // Check for HYPRLAND_INSTANCE_SIGNATURE environment variable
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() && Self::find_socket().is_some()
    }

    fn is_reactive(&self) -> bool {
        // Hyprland is NOT reactive - we position windows explicitly after spawn
        false
    }
}

impl NestedSession for HyprlandManager {
    fn nested_session_command(
        &self,
        _splitux_args: &[String],
        _monitor: &Monitor,
    ) -> Command {
        let mut cmd = Command::new("Hyprland");
        // Hyprland nested session configuration
        // This is typically done via config file or environment
        cmd.env("WLR_BACKENDS", "wayland");
        cmd
    }
}
