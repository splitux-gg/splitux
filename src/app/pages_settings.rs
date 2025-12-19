//! Settings page display functions
//!
//! This module is split into submodules for better organization:
//! - `general` - General and Gamescope settings (options 0-12)
//! - `audio` - Audio routing settings (options 13-19)
//! - `profiles` - Profile management (options 20+)
//! - `devices` - Controller naming

mod audio;
mod devices;
mod general;
mod profiles;

use super::app::{SettingsFocus, Splitux};
use super::config::{save_cfg, PartyConfig};
use super::theme;
use crate::input::scan_input_devices;
use crate::util::msg;
use eframe::egui::{self, RichText, Ui};

impl Splitux {
    /// Check if a settings option is currently focused
    pub(crate) fn is_settings_option_focused(&self, index: usize) -> bool {
        self.settings_focus == SettingsFocus::Options && self.settings_option_index == index
    }

    /// Scroll to focused option only when focus changed (clears the flag after scrolling)
    pub(crate) fn scroll_to_settings_option_if_needed(&mut self, index: usize, response: &egui::Response) {
        if self.settings_scroll_to_focus && self.is_settings_option_focused(index) {
            response.scroll_to_me(Some(egui::Align::Center));
            self.settings_scroll_to_focus = false;
        }
    }

    /// Get a frame for a settings option (with focus indicator if focused)
    pub(crate) fn settings_option_frame(&self, index: usize) -> egui::Frame {
        if self.is_settings_option_focused(index) {
            egui::Frame::NONE
                .fill(theme::colors::SELECTION_BG)
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(4, 2))
                .stroke(theme::focus_stroke())
        } else {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(4, 2))
        }
    }

    /// Main settings page orchestrator
    pub fn display_page_settings(&mut self, ui: &mut Ui) {
        self.infotext.clear();
        ui.add_space(8.0);
        ui.heading("Settings");
        ui.add_space(4.0);
        ui.label("Configure Splitux behavior and game launch options");
        ui.add_space(8.0);
        ui.separator();

        // Single scrollable settings page with sections
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(ui.available_height() - 60.0)
            .show(ui, |ui| {
                // === General Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("General").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_general(ui);

                ui.add_space(16.0);
                ui.separator();

                // === Gamescope Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("Gamescope").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_gamescope(ui);

                ui.add_space(16.0);
                ui.separator();

                // === Audio Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("Audio").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_audio(ui);

                ui.add_space(16.0);
                ui.separator();

                // === Profiles Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("Profiles").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_profiles(ui);

                ui.add_space(16.0);
                ui.separator();

                // === Devices Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("Controllers").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_devices(ui);

                ui.add_space(8.0);
            });

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            let is_buttons_focused = self.settings_focus == SettingsFocus::BottomButtons;
            ui.horizontal(|ui| {
                // Save Settings button
                let mut save_btn = egui::Button::new("Save Settings");
                if is_buttons_focused && self.settings_button_index == 0 {
                    save_btn = save_btn.stroke(theme::focus_stroke());
                }
                let save_response = ui.add(save_btn);
                if save_response.clicked() || (is_buttons_focused && self.settings_button_index == 0 && self.activate_focused) {
                    if let Err(e) = save_cfg(&self.options) {
                        msg("Error", &format!("Couldn't save settings: {}", e));
                    }
                }

                // Restore Defaults button
                let mut restore_btn = egui::Button::new("Restore Defaults");
                if is_buttons_focused && self.settings_button_index == 1 {
                    restore_btn = restore_btn.stroke(theme::focus_stroke());
                }
                let restore_response = ui.add(restore_btn);
                if restore_response.clicked() || (is_buttons_focused && self.settings_button_index == 1 && self.activate_focused) {
                    self.options = PartyConfig::default();
                    self.input_devices = scan_input_devices(&self.options.pad_filter_type);
                    self.refresh_device_display_names();
                }
            });
            ui.separator();
        });
    }
}
