//! Welcome screen displayed when no games are configured

use crate::app::app::Splitux;
use eframe::egui::{self, RichText, Ui};

impl Splitux {
    pub(super) fn display_welcome_screen(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.add(
            egui::Image::new(egui::include_image!("../../../assets/logo.png"))
                .max_height(120.0),
        );
        ui.add_space(4.0);
        ui.label("They killed splitscreen. We brought it back.");
        ui.add_space(12.0);
        ui.separator();

        // Quick Start Guide
        ui.add_space(8.0);
        ui.label(RichText::new("Getting Started").strong().size(16.0));
        ui.add_space(8.0);

        egui::Grid::new("quick_start_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("1.").strong());
                ui.label("Select a game from the sidebar, or add one with the + button");
                ui.end_row();

                ui.label(RichText::new("2.").strong());
                ui.label("Click Play to enter the instance setup screen");
                ui.end_row();

                ui.label(RichText::new("3.").strong());
                ui.label("Connect controllers and press A/Right-click to create instances");
                ui.end_row();

                ui.label(RichText::new("4.").strong());
                ui.label("Press Start when ready to launch");
                ui.end_row();
            });

        ui.add_space(16.0);
        ui.separator();

        // Controls Reference
        ui.add_space(8.0);
        ui.label(RichText::new("Controls").strong().size(16.0));
        ui.add_space(8.0);

        egui::Grid::new("controls_grid")
            .num_columns(2)
            .spacing([24.0, 8.0])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::new(egui::include_image!("../../../assets/BTN_DPAD.png"))
                            .max_height(18.0),
                    );
                    ui.add(
                        egui::Image::new(egui::include_image!("../../../assets/BTN_STICK_L.png"))
                            .max_height(18.0),
                    );
                });
                ui.label("Navigate UI");
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::new(egui::include_image!("../../../assets/BTN_LB.png"))
                            .max_height(18.0),
                    );
                    ui.add(
                        egui::Image::new(egui::include_image!("../../../assets/BTN_RB.png"))
                            .max_height(18.0),
                    );
                });
                ui.label("Switch Tabs");
                ui.end_row();

                ui.add(
                    egui::Image::new(egui::include_image!("../../../assets/BTN_STICK_R.png"))
                        .max_height(18.0),
                );
                ui.label("Scroll");
                ui.end_row();

                ui.add(
                    egui::Image::new(egui::include_image!("../../../assets/BTN_A.png"))
                        .max_height(18.0),
                );
                ui.label("Select / Confirm");
                ui.end_row();

                ui.add(
                    egui::Image::new(egui::include_image!("../../../assets/BTN_B.png"))
                        .max_height(18.0),
                );
                ui.label("Back");
                ui.end_row();
            });
    }
}
