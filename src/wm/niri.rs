//! Niri window manager integration via niri msg CLI

use crate::wm::bars::StatusBarManager;
use crate::wm::pure::layout::plan_tiling_layout;
use crate::wm::types::{get_layout_type, LayoutType, WmMonitor};
use crate::wm::{LayoutContext, WindowManager, WmResult};
use std::process::Command;

/// Niri window info from IPC (niri-specific: needs is_floating + u64 id)
#[derive(Debug, Clone)]
struct NiriWindow {
    id: u64,
    app_id: String,
    is_floating: bool,
}

/// Whether `pid`'s process is a gamescope binary. Used to identify gamescope
/// windows when the compositor reports no app_id (e.g. Proton titles on niri).
/// Checks the exe symlink first (most reliable), then the thread comm name.
fn pid_is_gamescope(pid: u64) -> bool {
    if let Ok(exe) = std::fs::read_link(format!("/proc/{pid}/exe"))
        && exe.to_string_lossy().to_lowercase().contains("gamescope") {
            return true;
        }
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|c| c.to_lowercase().contains("gamescope"))
        .unwrap_or(false)
}

pub struct NiriManager {
    target_monitor: Option<String>,
    bar_manager: StatusBarManager,
    /// Set from `LayoutContext::no_gamescope` in `setup`. When true the launch
    /// bypassed gamescope (single local seat) and the game window is a plain
    /// host surface, so window matching relaxes the gamescope marker and
    /// positioning misses are non-fatal.
    no_gamescope: bool,
}

impl NiriManager {
    pub fn new() -> Self {
        Self {
            target_monitor: None,
            bar_manager: StatusBarManager::new(),
            no_gamescope: false,
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
                if let Some(logical) = output.get("logical")
                    && !logical.is_null() {
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

    /// Get list of gamescope windows belonging to THIS launch.
    fn get_gamescope_windows(&self) -> WmResult<Vec<NiriWindow>> {
        let response = self.niri_msg(&["windows"])?;
        let windows: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| format!("Failed to parse windows: {}", e))?;

        // CONCURRENCY: only windows owned by THIS launch's systemd scope, so two
        // splitux sessions never grab each other's gamescope windows (which
        // stacked all of them into one column + fought over the output). Every
        // process in a launch (gamescope → bwrap → game) runs inside
        // `splitux-<launch_ns>-iN.scope`, so the window's pid cgroup carries our
        // launch namespace. When this launch isn't scoped there's no concurrency
        // to disambiguate, so fall back to matching all gamescope windows.
        let scoped = crate::launch::scope::enabled();
        let ns = crate::paths::launch_ns();
        let is_mine = |pid: u64| -> bool {
            if !scoped {
                return true;
            }
            std::fs::read_to_string(format!("/proc/{pid}/cgroup"))
                .map(|c| c.contains(&format!("splitux-{ns}")))
                .unwrap_or(false)
        };

        let mut result = Vec::new();
        if let Some(arr) = windows.as_array() {
            for win in arr {
                let app_id = win["app_id"].as_str().unwrap_or("");
                // Primary match: app_id carries "gamescope" (native games). But
                // gamescope's libdecor app_id doesn't always reach the host
                // compositor — Proton titles on niri surface with an UNSET
                // app_id (the title is the game name instead), so app_id-only
                // matching silently finds 0 windows and the launch times out.
                // Fall back to the window's PID: niri exposes it, and a window
                // backed by a gamescope binary is ours regardless of app_id.
                let pid = win["pid"].as_u64();
                let pid_is_gs = pid.is_some_and(pid_is_gamescope);
                // Gamescope-bypass launch: the game runs directly under niri with
                // NO gamescope process, so the gamescope marker never matches.
                // Ownership alone (the window's pid is in our launch scope) then
                // identifies our window — there's exactly one instance in this
                // mode, so the scoped match can't grab anything but the game.
                // Requires `scoped` (else `is_mine` is unconditionally true and
                // we'd grab every window on the desktop).
                let is_gamescope = (self.no_gamescope && scoped)
                    || app_id.to_lowercase().contains("gamescope")
                    || pid_is_gs;
                if is_gamescope && pid.is_some_and(is_mine)
                    && let Some(id) = win["id"].as_u64() {
                        result.push(NiriWindow {
                            id,
                            app_id: app_id.to_string(),
                            is_floating: win["is_floating"].as_bool().unwrap_or(false),
                        });
                    }
            }
        }
        Ok(result)
    }

    /// Resolve the niri output name for an instance's assigned monitor.
    ///
    /// Maps the instance's SDL monitor index → connector name → niri output,
    /// falling back to the session target monitor if the lookup fails.
    fn resolve_instance_monitor(&self, ctx: &LayoutContext, instance_idx: usize) -> Option<String> {
        if let Some(inst) = ctx.instances.get(instance_idx)
            && let Some(sdl_monitor) = ctx.monitors.get(inst.monitor) {
                let connector = sdl_monitor.connector_name();
                if let Ok(monitor) = self.get_monitor_by_name(connector) {
                    return Some(monitor.name);
                }
            }
        self.target_monitor.clone()
    }

    /// Fullscreen layout: give each gamescope window its own true-fullscreen
    /// surface on the monitor its instance was assigned to (the play-config
    /// display choice). Each window is placed in its own tiled column then put
    /// into niri's fullscreen state, so it covers the whole output edge-to-edge
    /// (over gaps and bars), 1:1 with the full-resolution gamescope surface —
    /// what splitux-together needs to capture. Multiple instances on one
    /// monitor land in adjacent columns you scroll between; each is its own
    /// fullscreen window.
    /// Pick a distinct output for a fullscreen window: prefer the instance's
    /// assigned monitor, but if it's already taken by an earlier fullscreen
    /// window, hand back the first still-free output (position order). Records the
    /// choice in `used`. Returns the preferred name (sharing) only when every
    /// output is already taken — i.e. more instances than monitors.
    fn pick_distinct_output(
        preferred: Option<&str>,
        outputs: &[WmMonitor],
        used: &mut Vec<String>,
    ) -> Option<String> {
        if let Some(p) = preferred
            && !used.iter().any(|u| u == p) {
                used.push(p.to_string());
                return Some(p.to_string());
            }
        if let Some(free) = outputs.iter().find(|o| !used.iter().any(|u| u == &o.name)) {
            used.push(free.name.clone());
            return Some(free.name.clone());
        }
        preferred.map(|p| p.to_string())
    }

    fn position_windows_fullscreen(
        &self,
        ctx: &LayoutContext,
        windows: &[NiriWindow],
    ) -> WmResult<()> {
        println!(
            "[splitux] wm::niri - Fullscreen layout: {} window(s)",
            windows.len()
        );

        // Every niri output, position-sorted. Used to spill colliding instances
        // onto distinct displays (see pick_distinct_output).
        let outputs = self.get_monitors().unwrap_or_default();
        let mut used_outputs: Vec<String> = Vec::new();

        // Pass 1: resolve each window's ACTUAL target output. The instance's
        // assigned monitor is the PREFERRED output. When the launch EXPLICITLY
        // assigned displays (`--display` / a GUI/TUI pick), honor that exactly —
        // two windows targeting one output share it and STACK (fullscreen each)
        // below. Otherwise (a bare fullscreen launch with no display intent),
        // pick_distinct_output spills a colliding instance onto the next FREE
        // display so independent fullscreen windows don't stack invisibly — only
        // sharing an output once there are more instances than outputs.
        let mut assigned: Vec<(usize, Option<String>)> = Vec::with_capacity(windows.len());
        for (i, window) in windows.iter().enumerate() {
            let preferred = self.resolve_instance_monitor(ctx, i);
            let target = if ctx.displays_assigned {
                preferred.clone()
            } else {
                Self::pick_distinct_output(preferred.as_deref(), &outputs, &mut used_outputs)
            };
            println!(
                "[splitux] wm::niri - Fullscreen window {}: id={} app_id={} -> monitor {:?} (preferred {:?}, assigned={})",
                i, window.id, window.app_id, target, preferred, ctx.displays_assigned
            );
            assigned.push((i, target));
        }

        // Pass 2: fullscreen EACH window on its resolved output. Two windows that
        // share an output STACK (each full-res; switch focus to swap) — that IS
        // what the Fullscreen layout means, and it's what splitux-together
        // captures cleanly per seat. For both-visible SIDE-BY-SIDE on a single
        // display, pick a tiled layout (vertical / horizontal / grid) instead;
        // those route through the tiling path, not here.
        for (i, target) in assigned {
            self.ensure_window_fullscreen(&windows[i], target.as_deref(), &outputs)?;
        }

        Ok(())
    }

    /// Put one window into true edge-to-edge fullscreen on its target output,
    /// IDEMPOTENTLY. `fullscreen-window` is a TOGGLE and niri exposes no
    /// fullscreen-state field — so a blind toggle is a coin flip. gamescope's
    /// `-f` boots the surface ALREADY fullscreen, so a blind toggle flips it
    /// BACK OUT to a tiled column (the "kicked to split" bug). The only
    /// fullscreen tell niri gives is geometry: a fullscreen window's
    /// `window_size` equals its output's logical size exactly (a tiled
    /// full-width column is slightly smaller — gaps/border, e.g. 1894x1054 vs
    /// 1920x1080). So: settle, then toggle ONLY when the window isn't already
    /// covering its target output. Correct whether the window booted fullscreen
    /// (`-f`) or tiled (no `-f`).
    fn ensure_window_fullscreen(
        &self,
        win: &NiriWindow,
        target: Option<&str>,
        outputs: &[WmMonitor],
    ) -> WmResult<()> {
        self.niri_action("focus-window", &["--id", &win.id.to_string()])?;
        std::thread::sleep(std::time::Duration::from_millis(30));

        if let Some(name) = target {
            self.niri_action("move-window-to-monitor", &[name])?;
            std::thread::sleep(std::time::Duration::from_millis(30));
        }

        // Ensure the window is tiled so its underlying slot is a full column
        // (it returns to a column when fullscreen is later toggled off).
        if win.is_floating {
            self.niri_action("move-window-to-tiling", &[])?;
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        std::thread::sleep(std::time::Duration::from_millis(60));
        let out_size = target.and_then(|name| {
            outputs.iter().find(|o| o.name == name).map(|o| (o.width as i64, o.height as i64))
        });
        let already_fullscreen =
            matches!((self.window_size_now(win.id), out_size), (Some(w), Some(o)) if w == o);
        if already_fullscreen {
            println!(
                "[splitux] wm::niri -   window {} already fullscreen on {:?}, leaving it",
                win.id, target
            );
        } else {
            self.niri_action("fullscreen-window", &["--id", &win.id.to_string()])?;
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        Ok(())
    }

    /// Current on-screen size of a window by id, `(width, height)`. niri exposes
    /// no `is_fullscreen`, so "window_size equals the output's logical size" is the
    /// fullscreen tell used by [`Self::position_windows_fullscreen`].
    fn window_size_now(&self, id: u64) -> Option<(i64, i64)> {
        let response = self.niri_msg(&["windows"]).ok()?;
        let windows: serde_json::Value = serde_json::from_str(&response).ok()?;
        for w in windows.as_array()? {
            if w["id"].as_u64() == Some(id) {
                let sz = w.get("layout")?.get("window_size")?;
                return Some((sz.get(0)?.as_i64()?, sz.get(1)?.as_i64()?));
            }
        }
        None
    }

    /// If `win` is currently covering its target output (the geometry tell for
    /// niri fullscreen state — see [`Self::window_size_now`]), toggle it OUT of
    /// fullscreen so it becomes a tiled column again. Geometry-gated so a window
    /// that booted tiled is never accidentally toggled INTO fullscreen. Best
    /// effort: any niri error is swallowed (tiling proceeds regardless).
    fn unfullscreen_if_covering(
        &self,
        win: &NiriWindow,
        target: Option<&str>,
        outputs: &[WmMonitor],
    ) {
        let out_size = match target.and_then(|name| outputs.iter().find(|o| o.name == name)) {
            Some(o) => (o.width as i64, o.height as i64),
            None => return,
        };
        std::thread::sleep(std::time::Duration::from_millis(40));
        if self.window_size_now(win.id) == Some(out_size) {
            println!(
                "[splitux] wm::niri -   window {} is fullscreen on {:?}, un-fullscreening to tile",
                win.id, target
            );
            let _ = self.niri_action("fullscreen-window", &["--id", &win.id.to_string()]);
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
    }

    /// Position all gamescope windows according to layout using tiled mode
    fn position_windows(&self, ctx: &LayoutContext) -> WmResult<()> {
        let windows = self.get_gamescope_windows()?;
        if windows.is_empty() {
            return Err("No gamescope windows found".into());
        }

        // Fullscreen layout has its own placement path (per-instance monitor,
        // each window a full-width column) rather than splitting one monitor.
        // A single gamescope window also owns the whole monitor, so always give
        // it that true-fullscreen path (edge-to-edge) instead of a tiled column:
        // tiling leaves niri's border/gap insets, so the game renders slightly
        // under-res (e.g. 1894x1054 instead of 1920x1080) and softens bench text.
        // Local-split couch games collapse to one instance, so this covers them.
        if get_layout_type(ctx.preset.id) == LayoutType::Fullscreen || windows.len() == 1 {
            return self.position_windows_fullscreen(ctx, &windows);
        }

        // How many DISTINCT monitors does this launch span? Window i ↔ instance i
        // (spawn order). A single-monitor split (incl. every single-game launch)
        // takes the original one-monitor tiling path, byte-identical. A
        // multi-monitor / multi-game split tiles EACH monitor independently
        // instead of cramming all windows onto one.
        let mut monitors_used: Vec<usize> = Vec::new();
        for i in 0..windows.len() {
            let mon = ctx.instances.get(i).map(|inst| inst.monitor).unwrap_or(0);
            if !monitors_used.contains(&mon) {
                monitors_used.push(mon);
            }
        }
        if monitors_used.len() <= 1 {
            return self.position_windows_single_monitor(ctx, &windows);
        }

        // Multi-monitor: group window indices by their instance's monitor, then
        // place each group on its own output — fullscreen a lone window, tile a
        // shared one with a plan sized for THAT monitor's window count.
        let outputs = self.get_monitors().unwrap_or_default();
        for mon_idx in monitors_used {
            let group: Vec<usize> = (0..windows.len())
                .filter(|&i| ctx.instances.get(i).map(|inst| inst.monitor).unwrap_or(0) == mon_idx)
                .collect();
            let target = group.first().and_then(|&i| self.resolve_instance_monitor(ctx, i));
            if group.len() == 1 {
                self.ensure_window_fullscreen(&windows[group[0]], target.as_deref(), &outputs)?;
            } else {
                self.tile_windows_on_monitor(ctx, &group, target.as_deref(), ctx.preset.id, &outputs)?;
            }
        }
        Ok(())
    }

    /// Original single-monitor tiling: move every window onto the one target
    /// monitor and tile them per the preset's plan. Kept verbatim so single-game
    /// and any single-display split is byte-identical to pre-multi-game.
    fn position_windows_single_monitor(
        &self,
        ctx: &LayoutContext,
        windows: &[NiriWindow],
    ) -> WmResult<()> {
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

    /// Tile one monitor's group of windows. `group` holds GLOBAL window indices
    /// (into the launch's window list); `preset_id` selects the split geometry
    /// (its column indices are group-local, mapped back through `group`).
    /// `outputs` is used only to detect a window that booted fullscreen so it
    /// can be un-fullscreened before tiling.
    fn tile_windows_on_monitor(
        &self,
        _ctx: &LayoutContext,
        group: &[usize],
        target: Option<&str>,
        preset_id: &str,
        outputs: &[WmMonitor],
    ) -> WmResult<()> {
        let plan = plan_tiling_layout(preset_id, group.len());

        // Step 1: move this group onto its monitor + ensure tiled.
        let windows = self.get_gamescope_windows()?;
        for &gi in group {
            if let Some(win) = windows.get(gi) {
                self.niri_action("focus-window", &["--id", &win.id.to_string()])?;
                std::thread::sleep(std::time::Duration::from_millis(30));
                if let Some(name) = target {
                    self.niri_action("move-window-to-monitor", &[name])?;
                    std::thread::sleep(std::time::Duration::from_millis(30));
                }
                if win.is_floating {
                    self.niri_action("move-window-to-tiling", &[])?;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                // A window that booted fullscreen (gamescope `-f`, the Fullscreen
                // preset path) is in niri's fullscreen state, where set-column-width
                // is a no-op — it would stay a stacked full-screen surface, not
                // split. Toggle it back to a tiled column FIRST, but only when it's
                // actually fullscreen (geometry == output size); a window that
                // booted tiled (non-fullscreen presets) is left alone so we don't
                // accidentally toggle it INTO fullscreen.
                self.unfullscreen_if_covering(win, target, outputs);
            }
        }

        // Step 2: apply the plan, mapping group-local column indices → global.
        let windows = self.get_gamescope_windows()?;
        for column in &plan.columns {
            let width = format!("{}%", column.width_percent);
            let actual: Vec<usize> = column
                .windows
                .iter()
                .filter_map(|&local| group.get(local).copied())
                .collect();
            if actual.is_empty() {
                continue;
            }
            if let Some(win) = windows.get(actual[0]) {
                self.niri_action("focus-window", &["--id", &win.id.to_string()])?;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            for &ai in &actual[1..] {
                if let Some(win) = windows.get(ai) {
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

        // Remember whether this launch bypassed gamescope (single local seat):
        // it changes how we match and how strictly we position the window.
        self.no_gamescope = ctx.no_gamescope;

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
        let expected_count = ctx.instances.len();

        // Gamescope-bypass: the game is already rendering directly under niri, so
        // window placement is a best-effort nicety, not a launch precondition. A
        // wait timeout or positioning error must NOT propagate — that would trip
        // the launch's early-abort guard and kill a game the user is watching for
        // the very artifact we're testing. Warn and carry on instead.
        if self.no_gamescope {
            println!("[splitux] wm::niri - Waiting for the game window (gamescope bypassed)...");
            if let Err(e) = crate::wm::operations::poll::wait_for_windows(
                "niri",
                expected_count,
                || self.get_gamescope_windows().unwrap_or_default().len(),
            ) {
                println!(
                    "[splitux] wm::niri - game window not matched ({e}); leaving placement to \
                     niri (game still running)"
                );
                return Ok(());
            }
            if let Err(e) = self.position_windows(ctx) {
                println!("[splitux] wm::niri - positioning skipped ({e}); game still running");
            }
            return Ok(());
        }

        println!("[splitux] wm::niri - Waiting for gamescope windows...");
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
