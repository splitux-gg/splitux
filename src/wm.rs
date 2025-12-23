//! Window Manager abstraction layer for Splitux
//!
//! This module provides a trait-based interface for different window managers,
//! allowing Splitux to work with Hyprland, KWin, or Gamescope-only mode.

mod bars;
mod gamescope;
mod hyprland;
mod kwin;
mod layout;
pub mod presets;

pub use gamescope::GamescopeOnlyManager;
pub use hyprland::HyprlandManager;
pub use kwin::KWinManager;

use crate::instance::Instance;
use crate::monitor::Monitor;
use std::error::Error;
use std::process::Command;

/// Result type for WM operations
pub type WmResult<T> = Result<T, Box<dyn Error>>;

/// Information needed by the WM to position windows
#[derive(Clone)]
pub struct LayoutContext {
    pub instances: Vec<Instance>,
    #[allow(dead_code)] // Used by calculate_all_geometries for multi-monitor layout
    pub monitors: Vec<Monitor>,
    /// Layout preset for positioning windows
    pub preset: &'static presets::LayoutPreset,
    /// Maps spawn index to region index (for custom layout ordering)
    /// e.g., [1, 0] means window 0 goes to region 1, window 1 goes to region 0
    pub instance_to_region: Vec<usize>,
}

/// The core window manager trait
pub trait WindowManager: Send + Sync {
    /// Human-readable name for this WM backend
    fn name(&self) -> &'static str;

    /// Initialize the WM integration (load scripts, connect to IPC, etc.)
    /// Called before launching games.
    fn setup(&mut self, ctx: &LayoutContext) -> WmResult<()>;

    /// Called after all game instances have been spawned but before waiting for them to exit.
    /// Used by non-reactive WMs (like Hyprland) to position windows.
    /// Default implementation does nothing (for reactive WMs like KWin).
    fn on_instances_launched(&mut self, _ctx: &LayoutContext) -> WmResult<()> {
        Ok(())
    }

    /// Clean up WM integration (unload scripts, disconnect IPC, etc.)
    /// Called after all games have exited.
    fn teardown(&mut self) -> WmResult<()>;

    /// Check if this WM is currently running and available
    fn is_available() -> bool
    where
        Self: Sized;

    /// Whether this WM handles window positioning automatically via reactive rules/scripts
    fn is_reactive(&self) -> bool;
}

/// Capability to launch a nested WM session
pub trait NestedSession: WindowManager {
    /// Build the command to launch a nested session with Splitux
    fn nested_session_command(
        &self,
        splitux_args: &[String],
        monitor: &Monitor,
    ) -> Command;
}

/// Enum wrapper for dynamic dispatch with type safety
pub enum WindowManagerBackend {
    KWin(KWinManager),
    Hyprland(HyprlandManager),
    GamescopeOnly(GamescopeOnlyManager),
}

impl WindowManagerBackend {
    /// Detect the running window manager and return appropriate backend
    pub fn detect() -> Self {
        // 1. Check for Hyprland first (has unique env var)
        if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() && HyprlandManager::is_available() {
            println!("[splitux] wm - Detected Hyprland compositor");
            return Self::Hyprland(HyprlandManager::new());
        }

        // 2. Check for KWin (via D-Bus or KDE_SESSION_VERSION)
        if std::env::var("KDE_SESSION_VERSION").is_ok()
            || std::env::var("KDE_FULL_SESSION").is_ok()
        {
            if KWinManager::is_available() {
                println!("[splitux] wm - Detected KWin compositor");
                return Self::KWin(KWinManager::new());
            }
        }

        // 3. Check for SteamOS / Gamescope session
        if std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok()
            || std::env::var("SteamOS").is_ok()
        {
            println!("[splitux] wm - Detected Gamescope/SteamOS session");
            return Self::GamescopeOnly(GamescopeOnlyManager::new());
        }

        // 4. Try process detection as last resort
        if let Ok(output) = std::process::Command::new("pgrep")
            .args(["-x", "Hyprland"])
            .output()
        {
            if output.status.success() && HyprlandManager::is_available() {
                println!("[splitux] wm - Detected Hyprland via process");
                return Self::Hyprland(HyprlandManager::new());
            }
        }

        if let Ok(output) = std::process::Command::new("pgrep")
            .args(["-x", "kwin_wayland"])
            .output()
        {
            if output.status.success() && KWinManager::is_available() {
                println!("[splitux] wm - Detected KWin via process");
                return Self::KWin(KWinManager::new());
            }
        }

        // 5. Default to no WM positioning
        println!("[splitux] wm - No supported WM detected, using Gamescope-only mode");
        Self::GamescopeOnly(GamescopeOnlyManager::new())
    }

}

// Implement WindowManager for the enum to delegate to inner types
impl WindowManager for WindowManagerBackend {
    fn name(&self) -> &'static str {
        match self {
            Self::KWin(wm) => wm.name(),
            Self::Hyprland(wm) => wm.name(),
            Self::GamescopeOnly(wm) => wm.name(),
        }
    }

    fn setup(&mut self, ctx: &LayoutContext) -> WmResult<()> {
        match self {
            Self::KWin(wm) => wm.setup(ctx),
            Self::Hyprland(wm) => wm.setup(ctx),
            Self::GamescopeOnly(wm) => wm.setup(ctx),
        }
    }

    fn on_instances_launched(&mut self, ctx: &LayoutContext) -> WmResult<()> {
        match self {
            Self::KWin(wm) => wm.on_instances_launched(ctx),
            Self::Hyprland(wm) => wm.on_instances_launched(ctx),
            Self::GamescopeOnly(wm) => wm.on_instances_launched(ctx),
        }
    }

    fn teardown(&mut self) -> WmResult<()> {
        match self {
            Self::KWin(wm) => wm.teardown(),
            Self::Hyprland(wm) => wm.teardown(),
            Self::GamescopeOnly(wm) => wm.teardown(),
        }
    }

    fn is_available() -> bool
    where
        Self: Sized,
    {
        // This is checked per-variant
        true
    }

    fn is_reactive(&self) -> bool {
        match self {
            Self::KWin(wm) => wm.is_reactive(),
            Self::Hyprland(wm) => wm.is_reactive(),
            Self::GamescopeOnly(wm) => wm.is_reactive(),
        }
    }
}
