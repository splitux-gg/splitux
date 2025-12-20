//! Gamepad-compatible dropdown component for unified navigation
//!
//! Provides a reusable dropdown that works with both mouse and gamepad input,
//! following the pattern established in the Settings page profiles.

use eframe::egui;
use crate::ui::theme;

/// An item in the dropdown list
pub struct DropdownItem<T> {
    /// The value this item represents
    pub value: T,
    /// Display label for the item
    pub label: String,
    /// Whether this item is currently selected/active
    pub is_selected: bool,
}

impl<T> DropdownItem<T> {
    pub fn new(value: T, label: impl Into<String>, is_selected: bool) -> Self {
        Self {
            value,
            label: label.into(),
            is_selected,
        }
    }
}

/// Response from rendering a dropdown
pub struct DropdownResponse<T> {
    /// Button was clicked while dropdown was closed - caller should open it
    pub toggled: bool,
    /// An item was selected - caller should close dropdown and apply value
    pub selected: Option<T>,
}

impl<T> DropdownResponse<T> {
    fn new() -> Self {
        Self {
            toggled: false,
            selected: None,
        }
    }
}

/// Render a gamepad-compatible dropdown
///
/// Returns a `DropdownResponse` that tells the caller:
/// - `toggled`: Button was clicked, toggle the open state
/// - `selected`: An item was selected, close and apply the value
///
/// The caller is responsible for:
/// - Managing `active_dropdown` state (set/clear based on response)
/// - Managing `dropdown_selection_idx` for navigation
/// - Building the items list (can add conditional items)
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `id` - Unique ID for this dropdown
/// * `button_text` - Text shown on the closed dropdown button
/// * `width` - Width of the dropdown
/// * `items` - Items to display
/// * `is_focused` - Whether this dropdown has gamepad focus
/// * `is_open` - Whether the popup is currently open
/// * `selection_idx` - Current highlighted index for gamepad nav
/// * `activate` - Whether activate button was pressed (gamepad A)
pub fn render_gamepad_dropdown<T: Clone>(
    ui: &mut egui::Ui,
    id: &str,
    button_text: &str,
    width: f32,
    items: &[DropdownItem<T>],
    is_focused: bool,
    is_open: bool,
    selection_idx: usize,
    activate: bool,
) -> DropdownResponse<T> {
    let mut response = DropdownResponse::new();

    // Apply focus stroke directly to button (not via frame wrapper)
    let mut button = egui::Button::new(format!("{} \u{25BC}", button_text));
    if is_focused {
        button = button.stroke(theme::focus_stroke());
    }

    let btn = ui.add_sized([width, 24.0], button);

    // Handle button click to toggle
    if btn.clicked() {
        response.toggled = true;
    }

    // Show popup if open
    if is_open {
        let popup_id = ui.make_persistent_id(format!("dropdown_popup_{}", id));

        egui::Popup::from_response(&btn)
            .id(popup_id)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(width);

                for (idx, item) in items.iter().enumerate() {
                    let is_highlighted = selection_idx == idx;
                    let label = if is_highlighted {
                        format!("\u{25B6} {}", item.label)
                    } else {
                        format!("  {}", item.label)
                    };

                    let resp = ui.selectable_label(item.is_selected || is_highlighted, label);
                    if resp.clicked() || (activate && is_highlighted) {
                        response.selected = Some(item.value.clone());
                    }
                }
            });
    }

    response
}
