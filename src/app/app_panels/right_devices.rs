use crate::app::app::Splitux;

use eframe::egui::RichText;
use eframe::egui::{self, Ui};
use egui_phosphor::regular as icons;

impl Splitux {
    /// Compact horizontal device strip for the top of the Instances page.
    ///
    /// Replaces the old collapsible right-side Devices panel (which was almost
    /// always closed and ate horizontal space). Shows connected input devices as
    /// chips — a device with a button held highlights live, so you can tell which
    /// controller is which while assigning — plus an inline troubleshooting hint.
    pub fn display_device_strip(&mut self, ui: &mut Ui) {
        let enabled_count = self.input_devices.iter().filter(|d| d.enabled()).count();

        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{} Devices", icons::GAME_CONTROLLER)).strong());
            ui.add_space(8.0);

            if enabled_count == 0 {
                ui.label(
                    RichText::new("none detected — connect a controller")
                        .italics()
                        .weak(),
                );
            } else {
                for (idx, pad) in self.input_devices.iter().enumerate() {
                    let display_name = self.device_display_name(idx);
                    let mut chip =
                        RichText::new(format!("{} {}", pad.emoji(), display_name)).small();
                    if !pad.enabled() {
                        chip = chip.weak();
                    } else if pad.has_button_held() {
                        // Live highlight: this is the controller you're pressing.
                        chip = chip.strong().color(crate::ui::theme::colors::ACCENT);
                    }
                    egui::Frame::NONE
                        .fill(crate::ui::theme::colors::BG_MID)
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.label(chip);
                        });
                    ui.add_space(4.0);
                }
            }

            // Troubleshooting hint, right-aligned.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(format!("{} Controller issues?", icons::INFO))
                            .small()
                            .weak(),
                    )
                    .selectable(false)
                    .sense(egui::Sense::hover()),
                )
                .on_hover_ui(|ui| {
                    ui.set_max_width(280.0);
                    ui.label(RichText::new("Incorrect mappings?").strong());
                    ui.label("Edit the handler and change SDL2 Override to \"Steam Runtime\" (32-bit) or \"System Installation\" (64-bit).");
                    ui.add_space(8.0);
                    ui.label(RichText::new("Devices not detected?").strong());
                    ui.label("Add your user to the input group:");
                    ui.horizontal(|ui| {
                        ui.code("sudo usermod -aG input $USER");
                        if ui
                            .add(egui::Button::new(icons::CLIPBOARD).min_size(egui::vec2(24.0, 24.0)))
                            .on_hover_text("Copy")
                            .clicked()
                        {
                            ui.ctx().copy_text("sudo usermod -aG input $USER".to_string());
                        }
                    });
                });
            });
        });
        ui.add_space(4.0);
        ui.separator();
    }
}
