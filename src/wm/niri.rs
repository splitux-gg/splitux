//! Niri window manager integration via niri msg CLI

use crate::wm::bars::StatusBarManager;
use crate::wm::{LayoutContext, WindowManager, WmResult};
use std::process::Command;

/// Layout type for tiled window arrangement
#[derive(Debug, Clone, Copy, PartialEq)]
enum LayoutType {
    /// N separate columns (side by side)
    Columns,
    /// All windows stacked in one column
    Stacked,
    /// 2x2 grid (2 columns with 2 stacked each)
    Grid,
}

/// Niri monitor info from IPC
#[derive(Debug, Clone)]
struct NiriMonitor {
    name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// Niri window info from IPC
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
    fn get_monitors(&self) -> WmResult<Vec<NiriMonitor>> {
        let response = self.niri_msg(&["outputs"])?;
        let outputs: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| format!("Failed to parse outputs: {}", e))?;

        let mut result = Vec::new();
        if let Some(obj) = outputs.as_object() {
            for (name, output) in obj {
                // Only include outputs with logical info (connected and enabled)
                if let Some(logical) = output.get("logical") {
                    if !logical.is_null() {
                        result.push(NiriMonitor {
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

    /// Find monitor by index
    fn get_monitor_by_index(&self, index: usize) -> WmResult<NiriMonitor> {
        let monitors = self.get_monitors()?;
        monitors
            .into_iter()
            .nth(index)
            .ok_or_else(|| format!("Monitor index {} not found", index).into())
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

    /// Determine layout type from preset ID
    fn get_layout_type(preset_id: &str) -> LayoutType {
        match preset_id {
            // Vertical = side-by-side columns
            "2p_vertical" | "3p_vertical" => LayoutType::Columns,
            // Horizontal = stacked in one column
            "2p_horizontal" | "3p_horizontal" => LayoutType::Stacked,
            // Grid = 2 columns with 2 stacked each
            "4p_grid" | "4p_rows" | "4p_columns" => LayoutType::Grid,
            _ => LayoutType::Columns, // Default fallback
        }
    }

    /// Consume N-1 windows into the current column (stack them)
    fn consume_into_column(&self, count: usize) -> WmResult<()> {
        for _ in 0..(count.saturating_sub(1)) {
            self.niri_action("consume-window-into-column", &[])?;
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        Ok(())
    }


    /// Position all gamescope windows according to layout using tiled mode
    fn position_windows(&self, ctx: &LayoutContext) -> WmResult<()> {
        let windows = self.get_gamescope_windows()?;
        if windows.is_empty() {
            return Err("No gamescope windows found".into());
        }

        let monitor_index = ctx.instances.first().map(|i| i.monitor).unwrap_or(0);
        let monitor = self.get_monitor_by_index(monitor_index)?;
        let layout_type = Self::get_layout_type(ctx.preset.id);
        let window_count = windows.len();

        println!(
            "[splitux] wm::niri - Target monitor: {} ({}x{}), layout: {:?}, {} windows",
            monitor.name, monitor.width, monitor.height, layout_type, window_count
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

        // Step 2: Apply layout based on type
        match layout_type {
            LayoutType::Columns => {
                // Each window in its own column with equal width
                let width_percent = format!("{}%", 100 / window_count);

                // Re-fetch windows after tiling changes
                let windows = self.get_gamescope_windows()?;

                for win in &windows {
                    self.niri_action("focus-window", &["--id", &win.id.to_string()])?;
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    self.niri_action("set-column-width", &[&width_percent])?;
                }

                println!(
                    "[splitux] wm::niri - Arranged {} columns at {}",
                    window_count, width_percent
                );
            }

            LayoutType::Stacked => {
                // All windows in one column stacked vertically
                // Focus first window, then consume others into it
                let windows = self.get_gamescope_windows()?;

                if let Some(first) = windows.first() {
                    self.niri_action("focus-window", &["--id", &first.id.to_string()])?;
                    std::thread::sleep(std::time::Duration::from_millis(50));

                    // Consume remaining windows into the column
                    self.consume_into_column(window_count)?;

                    // Set column to full width
                    self.niri_action("set-column-width", &["100%"])?;
                }

                println!(
                    "[splitux] wm::niri - Stacked {} windows in single column",
                    window_count
                );
            }

            LayoutType::Grid => {
                // 2x2 grid: 2 columns with 2 stacked windows each
                // The order depends on the preset (rows vs columns read order)
                let windows = self.get_gamescope_windows()?;

                if windows.len() >= 4 {
                    // Determine window order based on preset
                    let order: [usize; 4] = match ctx.preset.id {
                        // 4p_columns: P1/P2 left column, P3/P4 right column
                        "4p_columns" => [0, 1, 2, 3],
                        // 4p_grid/4p_rows: P1/P3 left column (top/bottom), P2/P4 right column
                        _ => [0, 2, 1, 3],
                    };

                    // Focus left column windows and stack them
                    self.niri_action("focus-window", &["--id", &windows[order[0]].id.to_string()])?;
                    std::thread::sleep(std::time::Duration::from_millis(50));

                    // Move second left-column window next to first, then consume
                    self.niri_action("focus-window", &["--id", &windows[order[1]].id.to_string()])?;
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    self.niri_action("focus-column-left", &[])?;
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    self.niri_action("consume-window-into-column", &[])?;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    self.niri_action("set-column-width", &["50%"])?;

                    // Focus right column windows and stack them
                    self.niri_action("focus-window", &["--id", &windows[order[2]].id.to_string()])?;
                    std::thread::sleep(std::time::Duration::from_millis(50));

                    self.niri_action("focus-window", &["--id", &windows[order[3]].id.to_string()])?;
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    self.niri_action("focus-column-left", &[])?;
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    self.niri_action("consume-window-into-column", &[])?;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    self.niri_action("set-column-width", &["50%"])?;

                    println!("[splitux] wm::niri - Arranged 4 windows in 2x2 grid");
                }
            }
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

        let monitor_index = ctx.instances.first().map(|i| i.monitor).unwrap_or(0);
        let monitor = self.get_monitor_by_index(monitor_index)?;
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
        let max_wait = std::time::Duration::from_secs(120);
        let poll_interval = std::time::Duration::from_millis(500);
        let start = std::time::Instant::now();

        loop {
            let windows = self.get_gamescope_windows().unwrap_or_default();

            if windows.len() >= expected_count {
                println!(
                    "[splitux] wm::niri - Found {} windows after {:.1}s",
                    windows.len(),
                    start.elapsed().as_secs_f32()
                );
                std::thread::sleep(std::time::Duration::from_millis(500));
                break;
            }

            if start.elapsed() > max_wait {
                println!(
                    "[splitux] wm::niri - Timeout waiting for windows ({}/{})",
                    windows.len(),
                    expected_count
                );
                break;
            }

            if start.elapsed().as_secs() % 5 == 0 && start.elapsed().as_millis() % 500 < 100 {
                println!(
                    "[splitux] wm::niri - Still waiting... ({}/{} windows)",
                    windows.len(),
                    expected_count
                );
            }

            std::thread::sleep(poll_interval);
        }

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
