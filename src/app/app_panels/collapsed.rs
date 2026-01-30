use crate::app::app::{MenuPage, Splitux};

use eframe::egui::RichText;
use eframe::egui::{self, Ui};
use egui_phosphor::regular as icons;

impl Splitux {
    /// Display collapsed left panel (just expand button)
    pub fn display_collapsed_games_panel(&mut self, ui: &mut Ui) {
        let label = match self.cur_page {
            MenuPage::Settings => "Settings",
            _ => "Games",
        };

        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            if ui
                .add(egui::Button::new(icons::CARET_RIGHT).min_size(egui::vec2(24.0, 24.0)))
                .on_hover_text(format!("Expand {} panel", label))
                .clicked()
            {
                if self.cur_page == MenuPage::Settings {
                    self.settings_panel_collapsed = false;
                } else {
                    self.games_panel_collapsed = false;
                }
            }
            ui.add_space(8.0);
            // Vertical label
            for ch in label.chars() {
                ui.label(RichText::new(ch.to_string()).small().weak());
            }
        });
    }

    /// Display collapsed devices panel (just expand button)
    pub fn display_collapsed_devices_panel(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            if ui
                .add(egui::Button::new(icons::CARET_LEFT).min_size(egui::vec2(24.0, 24.0)))
                .on_hover_text("Expand Devices panel")
                .clicked()
            {
                self.devices_panel_collapsed = false;
            }
            ui.add_space(8.0);
            // Vertical label
            for ch in "Devices".chars() {
                ui.label(RichText::new(ch.to_string()).small().weak());
            }
        });
    }
}
