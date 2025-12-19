//! Launch options bar for instance page

use crate::app::app::{InstanceFocus, Splitux};
use crate::app::theme;
use eframe::egui::{self, RichText, Ui};

impl Splitux {
    /// Display the bottom bar with launch options and start button
    pub(super) fn display_launch_options(&mut self, ui: &mut Ui) {
        if self.instances.is_empty() {
            return;
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(12.0);
            let start_btn = ui.add(
                egui::Button::image_and_text(
                    egui::Image::new(egui::include_image!("../../../res/BTN_START_NEW.png"))
                        .fit_to_exact_size(egui::vec2(24.0, 24.0)),
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

            // Launch options
            let is_launch_options_focused = self.instance_focus == InstanceFocus::LaunchOptions;
            let frame_stroke = if is_launch_options_focused {
                egui::Stroke::new(2.0, theme::colors::ACCENT)
            } else {
                egui::Stroke::NONE
            };

            theme::card_frame()
                .fill(theme::colors::BG_DARK)
                .stroke(frame_stroke)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Launch Options").strong());
                        ui.add_space(16.0);

                        let mut option_idx = 0;

                        // Split style - only relevant for 2 players
                        if self.instances.len() == 2 {
                            let split_focused =
                                is_launch_options_focused && self.launch_option_index == option_idx;

                            ui.label("Split:");

                            // Horizontal option
                            let h_selected = !self.options.vertical_two_player;
                            let h_text = if split_focused && h_selected {
                                RichText::new("▶ Horizontal")
                                    .strong()
                                    .color(theme::colors::ACCENT)
                            } else if h_selected {
                                RichText::new("Horizontal").strong()
                            } else if split_focused {
                                RichText::new("Horizontal").color(theme::colors::TEXT_MUTED)
                            } else {
                                RichText::new("Horizontal").color(theme::colors::TEXT_MUTED)
                            };
                            let r1 = ui.selectable_label(h_selected, h_text);

                            // Vertical option
                            let v_selected = self.options.vertical_two_player;
                            let v_text = if split_focused && v_selected {
                                RichText::new("▶ Vertical")
                                    .strong()
                                    .color(theme::colors::ACCENT)
                            } else if v_selected {
                                RichText::new("Vertical").strong()
                            } else if split_focused {
                                RichText::new("Vertical").color(theme::colors::TEXT_MUTED)
                            } else {
                                RichText::new("Vertical").color(theme::colors::TEXT_MUTED)
                            };
                            let r2 = ui.selectable_label(v_selected, v_text);

                            if r1.clicked() {
                                self.options.vertical_two_player = false;
                            }
                            if r2.clicked() {
                                self.options.vertical_two_player = true;
                            }
                            if r1.hovered() || r2.hovered() || split_focused {
                                self.infotext = "Horizontal: Players stacked top/bottom. Vertical: Players side-by-side. Press A to toggle.".to_string();
                            }

                            ui.add_space(16.0);
                            ui.add(egui::Separator::default().vertical());
                            ui.add_space(16.0);
                            option_idx += 1;
                        }

                        // Keyboard/mouse support option
                        let kb_focused =
                            is_launch_options_focused && self.launch_option_index == option_idx;
                        let kb_text = if kb_focused {
                            RichText::new("Keyboard/mouse support").color(theme::colors::ACCENT)
                        } else {
                            RichText::new("Keyboard/mouse support")
                        };

                        let checkbox_response =
                            ui.checkbox(&mut self.options.input_holding, kb_text);

                        if checkbox_response.hovered() || kb_focused {
                            self.infotext = "Uses gamescope-splitux with input device holding support. This allows assigning keyboards and mice to specific players. Press A to toggle.".to_string();
                        }
                    });
                });
            ui.add_space(8.0);
            ui.separator();
        });
    }
}
