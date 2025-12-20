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

use super::app::{SettingsCategory, SettingsFocus, Splitux};
use super::theme;
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

    /// Main settings page - center panel content only (left panel is in app_panels.rs)
    pub fn display_page_settings(&mut self, ui: &mut Ui) {
        self.infotext.clear();

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_space(8.0);
                match self.settings_category {
                    SettingsCategory::General => {
                        self.display_settings_general(ui);
                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.label(RichText::new("Gamescope").strong().size(14.0));
                        ui.add_space(4.0);
                        self.display_settings_gamescope(ui);
                    }
                    SettingsCategory::Audio => {
                        self.display_settings_audio(ui);
                    }
                    SettingsCategory::Profiles => {
                        self.display_settings_profiles(ui);
                    }
                    SettingsCategory::Controllers => {
                        self.display_settings_devices(ui);
                    }
                }
                ui.add_space(8.0);
            });
    }
}
