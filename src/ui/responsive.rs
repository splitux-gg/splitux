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

    /// Check if this is the wide layout mode
    pub fn is_wide(&self) -> bool {
        matches!(self, LayoutMode::Wide)
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

/// Calculate responsive width for a button
///
/// Returns a width that scales with available space.
/// - Wide: returns ideal width
/// - Medium: returns 85% of ideal (clamped to min)
/// - Narrow: returns minimum width
pub fn button_width(ui: &Ui, ideal: f32, min: f32) -> f32 {
    let available = ui.available_width();
    let mode = LayoutMode::from_width(available);

    match mode {
        LayoutMode::Wide => ideal,
        LayoutMode::Medium => (ideal * 0.85).max(min),
        LayoutMode::Narrow => min,
    }
}

/// Calculate responsive text input width
///
/// Returns a width that fills available space minus reserved space for labels.
pub fn text_input_width(ui: &Ui, label_reserve: f32, min: f32) -> f32 {
    let available = ui.available_width();
    (available - label_reserve).max(min)
}

/// Get appropriate text for a button based on layout mode
///
/// Returns full text in wide mode, abbreviated in medium, and abbreviated/empty in narrow.
pub fn button_text(mode: LayoutMode, full: &str, abbreviated: &str, narrow: &str) -> String {
    match mode {
        LayoutMode::Wide => full.to_string(),
        LayoutMode::Medium => abbreviated.to_string(),
        LayoutMode::Narrow => narrow.to_string(),
    }
}

/// Truncate text to max length with ellipsis if needed
pub fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else if max_len > 3 {
        format!("{}...", &text[..max_len - 3])
    } else {
        text[..max_len].to_string()
    }
}
