//! Responsive layout utilities for adapting UI to available width
//!
//! Provides breakpoint-based layout decisions for egui UI elements.

use eframe::egui::Ui;

/// Layout mode based on available width
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// >600px - full horizontal layout with all elements visible
    Wide,
    /// 400-600px - compact horizontal, abbreviated text, reduced spacing
    Medium,
    /// <400px - vertical stacking, icon-only buttons, minimal text
    Narrow,
}

impl LayoutMode {
    /// Determine layout mode from pixel width
    pub fn from_width(width: f32) -> Self {
        if width > 600.0 {
            LayoutMode::Wide
        } else if width > 400.0 {
            LayoutMode::Medium
        } else {
            LayoutMode::Narrow
        }
    }

    /// Determine layout mode from UI's available width
    pub fn from_ui(ui: &Ui) -> Self {
        Self::from_width(ui.available_width())
    }

    /// Check if this is the narrow layout mode
    pub fn is_narrow(&self) -> bool {
        matches!(self, LayoutMode::Narrow)
    }
}

/// Calculate responsive width for a ComboBox
///
/// Returns a width that scales with available space but respects min/max bounds.
/// - Wide: returns ideal width
/// - Medium: returns 80% of ideal (clamped to min)
/// - Narrow: returns 40% of available width (clamped between min and ideal)
pub fn combo_width(ui: &Ui, ideal: f32, min: f32) -> f32 {
    let available = ui.available_width();
    let mode = LayoutMode::from_width(available);

    match mode {
        LayoutMode::Wide => ideal,
        LayoutMode::Medium => (ideal * 0.8).max(min),
        LayoutMode::Narrow => (available * 0.4).clamp(min, ideal),
    }
}
