use crate::app::app::{MenuPage, Splitux};

use eframe::egui::RichText;
use eframe::egui::{self, Ui};
use egui_phosphor::regular as icons;

impl Splitux {
    pub fn display_panel_top(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // === Main Navigation Tabs (2 tabs: Games, Settings) ===
            let games_text = match self.is_lite() {
                true => "Play",
                false => "Games",
            };
            let games_page = match self.is_lite() {
                true => MenuPage::Instances,
                false => MenuPage::Games,
            };

            // Games/Play tab
            let games_selected = self.cur_page == MenuPage::Games || (self.is_lite() && self.cur_page == MenuPage::Instances);
            ui.add_space(4.0);
            let games_btn = ui.add(
                egui::Button::new(games_text)
                    .min_size(egui::vec2(70.0, 28.0))
                    .selected(games_selected),
            );
            if games_btn.clicked() {
                self.cur_page = games_page;
            }

            // Registry tab (only show in full mode, not lite)
            if !self.is_lite() {
                let registry_btn = ui.add(
                    egui::Button::new("Registry")
                        .min_size(egui::vec2(70.0, 28.0))
                        .selected(self.cur_page == MenuPage::Registry),
                );
                if registry_btn.clicked() {
                    self.cur_page = MenuPage::Registry;
                    // Fetch registry if not already loaded
                    if self.registry_index.is_none() && !self.registry_loading {
                        self.fetch_registry();
                    }
                }
            }

            // Settings tab
            let settings_btn = ui.add(
                egui::Button::new("Settings")
                    .min_size(egui::vec2(70.0, 28.0))
                    .selected(self.cur_page == MenuPage::Settings),
            );
            if settings_btn.clicked() {
                self.cur_page = MenuPage::Settings;
            }

            // === Right Side: Version & Close ===
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let close_btn = ui.add(
                    egui::Button::new(icons::X)
                        .min_size(egui::vec2(28.0, 28.0)),
                ).on_hover_text("Close");
                if close_btn.clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }

                ui.add_space(8.0);
                let version_label = if self.needs_update.load(std::sync::atomic::Ordering::Relaxed) {
                    format!("v{} (update available)", env!("CARGO_PKG_VERSION"))
                } else {
                    format!("v{}", env!("CARGO_PKG_VERSION"))
                };
                ui.label(RichText::new(version_label).small().weak());
            });
        });
    }
}
