//! Niri window manager integration via niri msg CLI

use crate::wm::bars::StatusBarManager;
use crate::wm::pure::layout::plan_tiling_layout;
use crate::wm::types::WmMonitor;
use crate::wm::{LayoutContext, WindowManager, WmResult};
use std::process::Command;

/// Niri window info from IPC (niri-specific: needs is_floating + u64 id)
#[derive(Debug, Clone)]
struct NiriWindow {
    id: u64,
    app_id: String,
    is_floating: bool,
}

pub struct NiriManager {
    target_monitor: Option<String>,
    bar_manager: StatusBarManager,
}

impl NiriManager {
    pub fn new() -> Self {
        Self {
            target_monitor: None,
            bar_manager: StatusBarManager::new(),
        }
    }

    /// Execute niri msg command and return JSON output
    fn niri_msg(&self, args: &[&str]) -> WmResult<String> {
        let output = Command::new("niri")
            .arg("msg")
            .arg("--json")
            .args(args)
            .output()
            .map_err(|e| format!("Failed to execute niri msg: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("niri msg failed: {}", stderr).into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Execute niri msg action (no JSON output expected)
    fn niri_action(&self, action: &str, args: &[&str]) -> WmResult<()> {
        let mut cmd = Command::new("niri");
        cmd.arg("msg").arg("action").arg(action);
        for arg in args {
            cmd.arg(arg);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute niri action: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Some actions return non-zero but still work, log but don't fail
            println!("[splitux] wm::niri - Action {} warning: {}", action, stderr);
        }

        Ok(())
    }

    /// Get monitor info from Niri
    fn get_monitors(&self) -> WmResult<Vec<WmMonitor>> {
        let response = self.niri_msg(&["outputs"])?;
        let outputs: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| format!("Failed to parse outputs: {}", e))?;

        let mut result = Vec::new();
        if let Some(obj) = outputs.as_object() {
            for (name, output) in obj {
                // Only include outputs with logical info (connected and enabled)
                if let Some(logical) = output.get("logical") {
                    if !logical.is_null() {
                        result.push(WmMonitor {
                            name: name.clone(),
                            x: logical["x"].as_i64().unwrap_or(0) as i32,
                            y: logical["y"].as_i64().unwrap_or(0) as i32,
                            width: logical["width"].as_u64().unwrap_or(1920) as u32,
                            height: logical["height"].as_u64().unwrap_or(1080) as u32,
                        });
                    }
                }
            }
        }

        // Sort by x position for consistent ordering
        result.sort_by_key(|m| (m.x, m.y));
        Ok(result)
    }

    /// Find monitor by index (niri's position-sorted order)
    /// Retries a few times if not found, to handle transient monitor enumeration
    fn get_monitor_by_index(&self, index: usize) -> WmResult<WmMonitor> {
        let max_retries = 5;
        let retry_delay = std::time::Duration::from_millis(200);

        for attempt in 0..max_retries {
            let monitors = self.get_monitors()?;
            if let Some(monitor) = monitors.into_iter().nth(index) {
                return Ok(monitor);
            }

            if attempt < max_retries - 1 {
                std::thread::sleep(retry_delay);
            }
        }

        Err(format!("Monitor index {} not found after {} retries", index, max_retries).into())
    }

    /// Find monitor by connector name (e.g., "HDMI-A-1", "DP-1")
    /// Retries a few times if not found, to handle transient monitor enumeration
    fn get_monitor_by_name(&self, connector_name: &str) -> WmResult<WmMonitor> {
        let max_retries = 5;
        let retry_delay = std::time::Duration::from_millis(200);

        for attempt in 0..max_retries {
            let monitors = self.get_monitors()?;
            if let Some(monitor) = monitors.into_iter().find(|m| m.name == connector_name) {
                return Ok(monitor);
            }

            if attempt < max_retries - 1 {
                std::thread::sleep(retry_delay);
            }
        }

        Err(format!("Monitor '{}' not found after {} retries", connector_name, max_retries).into())
    }

    /// Get list of gamescope windows
    fn get_gamescope_windows(&self) -> WmResult<Vec<NiriWindow>> {
        let response = self.niri_msg(&["windows"])?;
        let windows: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| format!("Failed to parse windows: {}", e))?;

        let mut result = Vec::new();
        if let Some(arr) = windows.as_array() {
            for win in arr {
                let app_id = win["app_id"].as_str().unwrap_or("");
                // Match gamescope variants
                if app_id.to_lowercase().contains("gamescope") {
                    if let Some(id) = win["id"].as_u64() {
                        result.push(NiriWindow {
                            id,
                            app_id: app_id.to_string(),
                            is_floating: win["is_floating"].as_bool().unwrap_or(false),
                        });
                    }
                }
            }
        }
        Ok(result)
    }

    /// Position all gamescope windows according to layout using tiled mode
    fn position_windows(&self, ctx: &LayoutContext) -> WmResult<()> {
        let windows = self.get_gamescope_windows()?;
        if windows.is_empty() {
            return Err("No gamescope windows found".into());
        }

        // Use the target monitor set in setup() (looked up by connector name)
        let monitor = match &self.target_monitor {
            Some(name) => self.get_monitor_by_name(name)?,
            None => {
                let monitor_index = ctx.instances.first().map(|i| i.monitor).unwrap_or(0);
                self.get_monitor_by_index(monitor_index)?
            }
        };

        let plan = plan_tiling_layout(ctx.preset.id, windows.len());

        println!(
            "[splitux] wm::niri - Target monitor: {} ({}x{}), {} columns, {} windows",
            monitor.name, monitor.width, monitor.height, plan.columns.len(), windows.len()
        );

        // Step 1: Move all windows to target monitor and ensure tiled
        for (i, win) in windows.iter().enumerate() {
            println!(
                "[splitux] wm::niri - Window {}: id={} app_id={}",
                i, win.id, win.app_id
            );
            self.niri_action("focus-window", &["--id", &win.id.to_string()])?;
            std::thread::sleep(std::time::Duration::from_millis(30));

            if let Some(ref target) = self.target_monitor {
                self.niri_action("move-window-to-monitor", &[target])?;
            }

            // Ensure window is in tiling mode
            if win.is_floating {
                self.niri_action("move-window-to-tiling", &[])?;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        // Step 2: Apply tiling plan — re-fetch windows after tiling changes
        let windows = self.get_gamescope_windows()?;

        for (col_idx, column) in plan.columns.iter().enumerate() {
            let width = format!("{}%", column.width_percent);

            if column.windows.len() == 1 {
                // Single window in this column — just set width
                if let Some(win) = windows.get(column.windows[0]) {
                    self.niri_action("focus-window", &["--id", &win.id.to_string()])?;
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    self.niri_action("set-column-width", &[&width])?;
                }
            } else {
                // Multiple windows stacked in this column
                // Focus the first window
                if let Some(win) = windows.get(column.windows[0]) {
                    self.niri_action("focus-window", &["--id", &win.id.to_string()])?;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }

                // For subsequent windows: focus, move to column, consume
                for &win_idx in &column.windows[1..] {
                    if let Some(win) = windows.get(win_idx) {
                        self.niri_action("focus-window", &["--id", &win.id.to_string()])?;
                        std::thread::sleep(std::time::Duration::from_millis(30));
                        self.niri_action("focus-column-left", &[])?;
                        std::thread::sleep(std::time::Duration::from_millis(30));
                        self.niri_action("consume-window-into-column", &[])?;
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }

                self.niri_action("set-column-width", &[&width])?;
            }

            println!(
                "[splitux] wm::niri - Column {}: {} windows at {}",
                col_idx, column.windows.len(), width
            );
        }

        Ok(())
    }
}

impl Default for NiriManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowManager for NiriManager {
    fn name(&self) -> &'static str {
        "Niri"
    }

    fn setup(&mut self, ctx: &LayoutContext) -> WmResult<()> {
        println!("[splitux] wm::niri - Setting up");

        // Get the target monitor using SDL's index (matches gamescope's --display-index)
        let monitor_index = ctx.instances.first().map(|i| i.monitor).unwrap_or(0);

        // Look up by connector name from SDL monitor (preferred for accuracy)
        let monitor = if let Some(sdl_monitor) = ctx.monitors.get(monitor_index) {
            let connector = sdl_monitor.connector_name();
            println!("[splitux] wm::niri - Looking up monitor by connector: {}", connector);
            self.get_monitor_by_name(connector)?
        } else {
            // Fallback to index if SDL monitor not available
            self.get_monitor_by_index(monitor_index)?
        };

        self.target_monitor = Some(monitor.name.clone());

        println!(
            "[splitux] wm::niri - Target monitor: {} ({}x{})",
            monitor.name, monitor.width, monitor.height
        );

        // Hide status bars
        self.bar_manager.hide_all();

        Ok(())
    }

    fn on_instances_launched(&mut self, ctx: &LayoutContext) -> WmResult<()> {
        println!("[splitux] wm::niri - Waiting for gamescope windows...");

        let expected_count = ctx.instances.len();
        crate::wm::operations::poll::wait_for_windows("niri", expected_count, || {
            self.get_gamescope_windows().unwrap_or_default().len()
        })?;

        self.position_windows(ctx)
    }

    fn teardown(&mut self) -> WmResult<()> {
        println!("[splitux] wm::niri - Tearing down");
        self.bar_manager.restore_all();
        Ok(())
    }

    fn is_available() -> bool {
        // Check for NIRI_SOCKET env var or niri process
        if std::env::var("NIRI_SOCKET").is_ok() {
            return true;
        }

        // Fallback: check if niri msg works
        Command::new("niri")
            .args(["msg", "version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn is_reactive(&self) -> bool {
        false
    }
}
