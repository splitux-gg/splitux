use crate::app::app::{SettingsCategory, SettingsFocus, Splitux};
use crate::app::theme;

use eframe::egui::{self, Ui};
use egui_phosphor::regular as icons;

impl Splitux {
    /// Left panel content for Settings page
    pub(super) fn display_panel_left_settings(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        // Header with collapse toggle
        ui.horizontal(|ui| {
            ui.heading("Settings");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(icons::CARET_LEFT).min_size(egui::vec2(20.0, 20.0)).frame(false))
                    .on_hover_text("Collapse panel")
                    .clicked()
                {
                    self.settings_panel_collapsed = true;
                }
            });
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Category list
        let categories = [
            SettingsCategory::General,
            SettingsCategory::Audio,
            SettingsCategory::Profiles,
            SettingsCategory::Controllers,
            SettingsCategory::ProfileBuilder,
        ];

        for cat in categories {
            let is_selected = self.settings_category == cat;
            let is_focused = self.settings_focus == SettingsFocus::CategoryList
                && self.settings_category == cat;

            let frame = if is_selected {
                egui::Frame::NONE
                    .fill(theme::colors::SELECTION_BG)
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .stroke(if is_focused {
                        theme::focus_stroke()
                    } else {
                        egui::Stroke::new(1.0, theme::colors::ACCENT_DIM)
                    })
            } else {
                egui::Frame::NONE
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .stroke(if is_focused {
                        theme::focus_stroke()
                    } else {
                        egui::Stroke::NONE
                    })
            };

            let frame_resp = frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(
                    egui::Label::new(cat.label())
                        .selectable(false)
                        .sense(egui::Sense::click()),
                )
            });

            // Click on inner label or the frame
            if frame_resp.response.clicked() || frame_resp.inner.clicked() {
                self.settings_category = cat;
                self.settings_focus = SettingsFocus::Options;
                self.settings_option_index = 0;
            }
        }

        // Bottom buttons
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(8.0);
            self.display_settings_bottom_buttons_panel(ui);
        });
    }

    /// Settings bottom buttons for left panel
    pub(super) fn display_settings_bottom_buttons_panel(&mut self, ui: &mut Ui) {
        use crate::app::config::{save_cfg, SplituxConfig};
        use crate::input::scan_input_devices;

        let is_buttons_focused = self.settings_focus == SettingsFocus::BottomButtons;

        // Restore Defaults button (bottom-up, so this appears at bottom)
        let mut restore_btn = egui::Button::new("Restore Defaults");
        if is_buttons_focused && self.settings_button_index == 1 {
            restore_btn = restore_btn.stroke(theme::focus_stroke());
        }
        let restore_response = ui.add_sized([ui.available_width(), 24.0], restore_btn);
        if restore_response.clicked()
            || (is_buttons_focused && self.settings_button_index == 1 && self.activate_focused)
        {
            self.options = SplituxConfig::default();
            self.input_devices = scan_input_devices(&self.options.pad_filter_type);
            self.refresh_device_display_names();
        }

        ui.add_space(4.0);

        // Save Settings button
        let mut save_btn = egui::Button::new("Save Settings");
        if is_buttons_focused && self.settings_button_index == 0 {
            save_btn = save_btn.stroke(theme::focus_stroke());
        }
        let save_response = ui.add_sized([ui.available_width(), 24.0], save_btn);
        if save_response.clicked()
            || (is_buttons_focused && self.settings_button_index == 0 && self.activate_focused)
        {
            if let Err(e) = save_cfg(&self.options) {
                crate::util::msg("Error", &format!("Couldn't save settings: {}", e));
            }
        }

        ui.add_space(4.0);
        ui.separator();
    }
}
