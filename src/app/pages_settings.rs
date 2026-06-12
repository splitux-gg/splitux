//! Settings page display functions
//!
//! This module is split into submodules for better organization:
//! - `general` - General and Gamescope settings (options 0-12)
//! - `audio` - Audio routing settings (options 13-19)
//! - `profiles` - Profile management (options 20+)
//! - `devices` - Controller naming
//! - `profile_builder` - gptokeyb KB/Mouse Mapper

mod audio;
mod devices;
mod general;
mod profile_builder;
mod profiles;

use super::app::{SettingsCategory, SettingsFocus, Splitux};
use crate::ui::theme;
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
                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.label(RichText::new("Together (remote play)").strong().size(14.0));
                        ui.add_space(4.0);
                        self.display_settings_together(ui);
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
                    SettingsCategory::ProfileBuilder => {
                        self.display_settings_profile_builder(ui);
                    }
                }
                ui.add_space(8.0);
            });
    }

    /// Connection settings for splitux-together (the per-player "Together"
    /// checkbox lives on each instance card; this is the shared plumbing).
    fn display_settings_together(&mut self, ui: &mut Ui) {
        let t = &mut self.options.together;
        let mut changed = false;

        ui.label(
            RichText::new(
                "Mark a player as \"Together (remote)\" on the Instances page to stream their \
                 screen to a browser. These settings point that at your server.",
            )
            .color(theme::colors::TEXT_MUTED)
            .size(12.0),
        );
        ui.add_space(6.0);

        egui::Grid::new("together_settings_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Signalling URL");
                changed |= ui
                    .text_edit_singleline(&mut t.signalling_uri)
                    .on_hover_text("Producer websocket seats dial out to, e.g. wss://together.gabeforge.com/ws/producer")
                    .changed();
                ui.end_row();

                ui.label("Public base URL");
                changed |= ui
                    .text_edit_singleline(&mut t.public_base_url)
                    .on_hover_text("Invite links are {base}/j/<token>, e.g. https://together.gabeforge.com")
                    .changed();
                ui.end_row();

                ui.label("Spawn local orchestrator");
                changed |= ui
                    .checkbox(&mut t.spawn_local_orchestrator, "")
                    .on_hover_text("On: splitux runs its own orchestrator. Off: use the service at the Signalling URL.")
                    .changed();
                ui.end_row();

                ui.label("Encoder");
                changed |= ui
                    .text_edit_singleline(&mut t.encoder)
                    .on_hover_text("va (AMD VCN, recommended), vulkan, or x264 (CPU)")
                    .changed();
                ui.end_row();

                ui.label("Bitrate (kbps)");
                changed |= ui.add(egui::DragValue::new(&mut t.bitrate).range(1000..=60000)).changed();
                ui.end_row();

                ui.label("FPS (0 = auto)");
                changed |= ui.add(egui::DragValue::new(&mut t.fps).range(0..=240)).changed();
                ui.end_row();

                ui.label("TURN relay");
                let mut turn = t.turn.clone().unwrap_or_default();
                if ui
                    .text_edit_singleline(&mut turn)
                    .on_hover_text("Optional, for friends behind NAT: turn://user:pass@turn.gabeforge.com:3478")
                    .changed()
                {
                    t.turn = if turn.trim().is_empty() { None } else { Some(turn) };
                    changed = true;
                }
                ui.end_row();
            });

        if changed {
            let _ = crate::config::save_cfg(&self.options);
        }
    }
}
