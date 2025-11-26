//! Gamescope-only mode - no external window manager positioning needed.
//! Used on SteamOS or when running inside gamescope compositor directly.

use crate::wm::{LayoutContext, WindowManager, WmResult};

pub struct GamescopeOnlyManager;

impl GamescopeOnlyManager {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GamescopeOnlyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowManager for GamescopeOnlyManager {
    fn name(&self) -> &'static str {
        "Gamescope (No WM)"
    }

    fn setup(&mut self, _ctx: &LayoutContext) -> WmResult<()> {
        // No setup needed - gamescope handles its own window via --display-index
        println!("[splitux] wm::gamescope - No external WM positioning needed");
        Ok(())
    }

    fn teardown(&mut self) -> WmResult<()> {
        // Nothing to clean up
        Ok(())
    }

    fn is_available() -> bool {
        // Always available as a fallback
        true
    }

    fn is_reactive(&self) -> bool {
        // Not applicable - no WM positioning
        true
    }
}
