use crate::app::app::Splitux;

use eframe::egui::RichText;
use eframe::egui::{self, Ui};
use egui_phosphor::regular as icons;

impl Splitux {
    pub fn display_panel_right(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.add_space(8.0);
        // Header with collapse toggle
        ui.horizontal(|ui| {
            ui.heading("Devices");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(icons::CARET_RIGHT).min_size(egui::vec2(20.0, 20.0)).frame(false))
                    .on_hover_text("Collapse panel")
                    .clicked()
                {
                    self.devices_panel_collapsed = true;
                }
            });
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        let enabled_count = self.input_devices.iter().filter(|d| d.enabled()).count();
        if enabled_count == 0 {
            ui.label(RichText::new("No devices detected").italics().weak());
            ui.add_space(4.0);
            ui.label(RichText::new("Connect a controller").small().weak());
        } else {
            ui.label(RichText::new(format!("{} device(s) ready", enabled_count)).small());
            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - 80.0)
                .show(ui, |ui| {
                    for (idx, pad) in self.input_devices.iter().enumerate() {
                        let display_name = self.device_display_name(idx);
                        let mut dev_text = RichText::new(format!(
                            "{} {}",
                            pad.emoji(),
                            display_name,
                        ));

                        if !pad.enabled() {
                            dev_text = dev_text.weak();
                        } else if pad.has_button_held() {
                            dev_text = dev_text.strong();
                        }

                        ui.label(dev_text);
                    }
                });
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(RichText::new(format!("{} Controller issues?", icons::INFO)).small())
                    .selectable(false)
                    .sense(egui::Sense::click()),
            ).on_hover_ui(|ui| {
                ui.set_max_width(250.0);
                ui.label(RichText::new("Incorrect mappings?").strong());
                ui.label("Edit the handler and change SDL2 Override to \"Steam Runtime\" (32-bit) or \"System Installation\" (64-bit).");
                ui.add_space(8.0);
                ui.label(RichText::new("Devices not detected?").strong());
                ui.label("Add your user to the input group:");
                ui.horizontal(|ui| {
                    ui.code("sudo usermod -aG input $USER");
                    if ui.add(egui::Button::new(icons::CLIPBOARD).min_size(egui::vec2(24.0, 24.0))).on_hover_text("Copy").clicked() {
                        ctx.copy_text("sudo usermod -aG input $USER".to_string());
                    }
                });
            });
            ui.separator();
        });
    }
}
