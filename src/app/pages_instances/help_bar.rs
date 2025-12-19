//! Controls help bar for instance page

use crate::app::app::Splitux;
use crate::app::theme;
use crate::ui::responsive::LayoutMode;
use eframe::egui::{self, RichText, Ui};

impl Splitux {
    /// Display the controls help bar (responsive)
    pub(super) fn display_instance_help_bar(&self, ui: &mut Ui) {
        ui.add_space(8.0);
        let help_mode = LayoutMode::from_ui(ui);
        theme::card_frame()
            .fill(theme::colors::BG_DARK)
            .show(ui, |ui| {
                if help_mode.is_narrow() {
                    // Compact mode: icons only with tooltips, wrapped
                    ui.horizontal_wrapped(|ui| {
                        let add_tip = match self.instance_add_dev {
                            None => "A / Z / Right-Click: Add Instance",
                            Some(i) => &format!("A / Z / Right-Click: Add to P{}", i + 1),
                        };
                        ui.add(
                            egui::Image::new(egui::include_image!("../../../res/BTN_A.png"))
                                .max_height(16.0),
                        )
                        .on_hover_text(add_tip);

                        let remove_tip = match self.instance_add_dev {
                            None => "B / X: Remove",
                            Some(_) => "B / X: Cancel",
                        };
                        ui.add(
                            egui::Image::new(egui::include_image!("../../../res/BTN_B.png"))
                                .max_height(16.0),
                        )
                        .on_hover_text(remove_tip);

                        ui.add(
                            egui::Image::new(egui::include_image!("../../../res/BTN_Y.png"))
                                .max_height(16.0),
                        )
                        .on_hover_text("Y / A: Invite Device");

                        ui.add(
                            egui::Image::new(egui::include_image!("../../../res/BTN_DPAD.png"))
                                .max_height(16.0),
                        )
                        .on_hover_text("D-pad / Left Stick: Navigate");

                        ui.add(
                            egui::Image::new(egui::include_image!("../../../res/BTN_STICK_R.png"))
                                .max_height(16.0),
                        )
                        .on_hover_text("Right Stick: Scroll");
                    });
                } else {
                    // Full mode: icons with labels
                    ui.horizontal(|ui| {
                        // Add instance control
                        ui.add(
                            egui::Image::new(egui::include_image!("../../../res/BTN_A.png"))
                                .max_height(16.0),
                        );
                        ui.label(" / Z / Right-Click:");
                        let add_text = match self.instance_add_dev {
                            None => "Add Instance",
                            Some(i) => &format!("Add to P{}", i + 1),
                        };
                        ui.label(RichText::new(add_text).strong());

                        ui.add_space(16.0);
                        ui.add(egui::Separator::default().vertical());
                        ui.add_space(16.0);

                        // Remove/Cancel control
                        ui.add(
                            egui::Image::new(egui::include_image!("../../../res/BTN_B.png"))
                                .max_height(16.0),
                        );
                        ui.label(" / X:");
                        let remove_text = match self.instance_add_dev {
                            None => "Remove",
                            Some(_) => "Cancel",
                        };
                        ui.label(RichText::new(remove_text).strong());

                        ui.add_space(16.0);
                        ui.add(egui::Separator::default().vertical());
                        ui.add_space(16.0);

                        // Invite control
                        ui.add(
                            egui::Image::new(egui::include_image!("../../../res/BTN_Y.png"))
                                .max_height(16.0),
                        );
                        ui.label(" / A:");
                        ui.label(RichText::new("Invite Device").strong());

                        ui.add_space(16.0);
                        ui.add(egui::Separator::default().vertical());
                        ui.add_space(16.0);

                        // Navigation hints
                        ui.add(
                            egui::Image::new(egui::include_image!("../../../res/BTN_DPAD.png"))
                                .max_height(16.0),
                        );
                        ui.add(
                            egui::Image::new(egui::include_image!("../../../res/BTN_STICK_L.png"))
                                .max_height(16.0),
                        );
                        ui.label(RichText::new("Navigate").strong());

                        ui.add_space(8.0);

                        ui.add(
                            egui::Image::new(egui::include_image!("../../../res/BTN_STICK_R.png"))
                                .max_height(16.0),
                        );
                        ui.label(RichText::new("Scroll").strong());
                    });
                }
            });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
    }
}
