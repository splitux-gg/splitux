// Re-export from new ui/theme domain for backward compatibility
pub use crate::ui::theme::*;

// Keep the colors submodule for compatibility with existing imports like super::theme::colors::*
pub mod colors {
    pub use crate::ui::theme::colors::*;
}
