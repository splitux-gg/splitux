//! Focus helper functions for instance page elements

use crate::app::app::InstanceFocus;
use crate::app::theme;
use crate::ui::focus::types::InstanceCardFocus;
use eframe::egui;

/// Check if a specific element in an instance card is focused (pure function)
pub(super) fn is_element_focused(focus: &InstanceFocus, instance_idx: usize, element: InstanceCardFocus) -> bool {
    matches!(focus, InstanceFocus::InstanceCard(i, e) if *i == instance_idx && *e == element)
}

/// Get focus stroke for an element (accent color if focused)
pub(super) fn element_focus_stroke(focus: &InstanceFocus, instance_idx: usize, element: InstanceCardFocus) -> egui::Stroke {
    if is_element_focused(focus, instance_idx, element) {
        theme::focus_stroke()
    } else {
        egui::Stroke::NONE
    }
}
