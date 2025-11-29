// Minimal focus management stub
// The actual focus navigation is handled by FocusPane in app.rs

/// Stub FocusManager for compatibility
/// Real navigation uses the simpler pane-based system (FocusPane)
#[derive(Default)]
pub struct FocusManager;

impl FocusManager {
    pub fn new() -> Self {
        Self
    }

    /// Called at start of frame (no-op in current implementation)
    pub fn begin_frame(&mut self) {}

    /// Called on page transitions (no-op in current implementation)
    pub fn focus_first(&mut self) {}
}
