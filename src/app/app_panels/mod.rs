mod top_bar;
mod left_games;
mod left_settings;
mod right_devices;
mod collapsed;

use crate::app::app::{MenuPage, Splitux};

use eframe::egui::Ui;

impl Splitux {
    pub fn display_panel_left(&mut self, ui: &mut Ui) {
        match self.cur_page {
            MenuPage::Settings => self.display_panel_left_settings(ui),
            _ => self.display_panel_left_games(ui),
        }
    }
}
