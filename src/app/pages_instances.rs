// Instance setup page display functions

use super::app::PartyApp;
use super::theme;
use eframe::egui::{self, RichText, Ui};

impl PartyApp {
    pub fn display_page_instances(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.heading("Instance Setup");
        ui.add_space(4.0);
        ui.label("Connect your controllers and assign them to player instances");
        ui.add_space(8.0);
        ui.separator();

        // Controls help bar
        ui.add_space(8.0);
        theme::card_frame()
            .fill(theme::colors::BG_DARK)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Add instance control
                    ui.add(egui::Image::new(egui::include_image!("../../res/BTN_SOUTH.png")).max_height(16.0));
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
                    ui.add(egui::Image::new(egui::include_image!("../../res/BTN_EAST.png")).max_height(16.0));
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
                    ui.add(egui::Image::new(egui::include_image!("../../res/BTN_NORTH.png")).max_height(16.0));
                    ui.label(" / A:");
                    ui.label(RichText::new("Invite Device").strong());
                });
            });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        let mut devices_to_remove: Vec<(usize, usize)> = Vec::new();

        if self.instances.is_empty() {
            ui.add_space(16.0);
            ui.label(RichText::new("No instances yet").italics());
            ui.add_space(4.0);
            ui.label("Press A or Right-click on a controller to create a player instance");
        }

        // Player colors for visual distinction
        let player_colors = [
            egui::Color32::from_rgb(80, 180, 255),  // P1: Blue
            egui::Color32::from_rgb(255, 100, 100), // P2: Red
            egui::Color32::from_rgb(100, 220, 100), // P3: Green
            egui::Color32::from_rgb(255, 200, 80),  // P4: Yellow
        ];

        for (i, instance) in &mut self.instances.iter_mut().enumerate() {
            let player_color = player_colors.get(i).copied().unwrap_or(theme::colors::ACCENT);

            theme::card_frame()
                .stroke(egui::Stroke::new(2.0, player_color))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("P{}", i + 1)).strong().size(18.0).color(player_color));
                        ui.add_space(8.0);

                        ui.label("Profile:");
                        egui::ComboBox::from_id_salt(format!("{i}"))
                            .width(120.0)
                            .show_index(
                                ui,
                                &mut instance.profselection,
                                self.profiles.len(),
                                |i| self.profiles[i].clone(),
                            );

                        if self.options.gamescope_sdl_backend {
                            ui.add_space(8.0);
                            ui.label("Monitor:");
                            egui::ComboBox::from_id_salt(format!("monitors{i}"))
                                .width(100.0)
                                .show_index(
                                    ui,
                                    &mut instance.monitor,
                                    self.monitors.len(),
                                    |i| self.monitors[i].name(),
                                );
                        }

                        ui.add_space(8.0);
                        if self.instance_add_dev == None {
                            let invitebtn = ui.add(
                                egui::Button::image_and_text(
                                    egui::include_image!("../../res/BTN_NORTH.png"),
                                    " Invite Device",
                                )
                                .min_size(egui::vec2(0.0, 26.0)),
                            );
                            if invitebtn.clicked() {
                                self.instance_add_dev = Some(i);
                            }
                        } else if self.instance_add_dev == Some(i) {
                            ui.label(RichText::new("Waiting for input...").italics());
                            if ui.add(egui::Button::new("x").min_size(egui::vec2(26.0, 26.0))).clicked() {
                                self.instance_add_dev = None;
                            }
                        }
                    });

                    // Device list
                    for &dev in instance.devices.iter() {
                        let mut dev_text = RichText::new(format!(
                            "   {} {}",
                            self.input_devices[dev].emoji(),
                            self.input_devices[dev].fancyname()
                        ));

                        if self.input_devices[dev].has_button_held() {
                            dev_text = dev_text.strong();
                        }

                        ui.horizontal(|ui| {
                            ui.label(dev_text);
                            if ui.add(egui::Button::new("Remove").min_size(egui::vec2(24.0, 24.0))).on_hover_text("Remove device").clicked() {
                                devices_to_remove.push((i, dev));
                            }
                        });
                    }
                });
            ui.add_space(4.0);
        }

        for (i, d) in devices_to_remove {
            self.remove_device_instance(i, d);
        }

        if self.instances.len() > 0 {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                let start_btn = ui.add(
                    egui::Button::image_and_text(
                        egui::include_image!("../../res/BTN_START.png"),
                        "  Start Game  ",
                    )
                    .min_size(egui::vec2(180.0, 48.0))
                    .corner_radius(10)
                    .fill(theme::colors::ACCENT_DIM),
                );
                if start_btn.clicked() {
                    self.prepare_game_launch();
                }
                ui.add_space(8.0);
                ui.separator();
            });
        }
    }
}
