//! Instance warning display

use crate::app::app::Splitux;
use crate::ui::theme;
use eframe::egui::{self, RichText, Ui};
use egui_phosphor::regular as icons;

impl Splitux {
    /// Display controller and audio warnings
    pub(super) fn display_instance_warnings(&self, ui: &mut Ui) {
        if !self.controller_warnings.is_empty() {
            theme::card_frame()
                .fill(egui::Color32::from_rgb(80, 60, 20))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icons::WARNING).size(16.0));
                        ui.label(RichText::new("Missing preferred controllers:").strong());
                    });
                    for warning in &self.controller_warnings {
                        ui.label(format!("  \u{2022} {}", warning));
                    }
                });
            ui.add_space(4.0);
        }

        if !self.audio_warnings.is_empty() {
            theme::card_frame()
                .fill(egui::Color32::from_rgb(80, 60, 20))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icons::SPEAKER_SLASH).size(16.0));
                        ui.label(RichText::new("Missing preferred audio devices:").strong());
                    });
                    for warning in &self.audio_warnings {
                        ui.label(format!("  \u{2022} {}", warning));
                    }
                });
            ui.add_space(4.0);
        }
    }
}
