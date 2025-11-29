// Games page display functions

use super::app::{FocusPane, PartyApp};
use crate::handler::HANDLER_SPEC_CURRENT_VERSION;
use crate::util::msg;
use eframe::egui::{self, RichText, Ui};

macro_rules! cur_handler {
    ($self:expr) => {
        &$self.handlers[$self.selected_handler]
    };
}

impl PartyApp {
    pub fn display_page_games(&mut self, ui: &mut Ui) {
        // If no handlers or in lite mode, show welcome screen
        if self.handlers.is_empty() && !self.is_lite() {
            self.display_welcome_screen(ui);
            return;
        }

        // Show selected game info
        self.display_game_info(ui);
    }

    fn display_welcome_screen(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.heading("Welcome to Splitux");
        ui.add_space(4.0);
        ui.label("Local co-op split-screen gaming for Linux");
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
            .spacing([24.0, 6.0])
            .show(ui, |ui| {
                ui.label("Navigate tabs:");
                ui.label("LB/RB  or  Tab");
                ui.end_row();

                ui.label("Select/Confirm:");
                ui.label("A  or  Enter");
                ui.end_row();

                ui.label("Back:");
                ui.label("B  or  Escape");
                ui.end_row();

                ui.label("Navigate UI:");
                ui.label("D-Pad / Left Stick  or  Arrow Keys");
                ui.end_row();
            });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            ui.label("Based on");
            ui.hyperlink_to("PartyDeck", "https://github.com/wunnr/partydeck");
            ui.label("by @wunnr");
        });
    }

    fn display_game_info(&mut self, ui: &mut Ui) {
        // Extract all handler data we need upfront to avoid borrow issues
        let icon = self.handlers[self.selected_handler].icon();
        let display = self.handlers[self.selected_handler].display();
        let is_win = self.handlers[self.selected_handler].win();
        let author = self.handlers[self.selected_handler].author.clone();
        let version = self.handlers[self.selected_handler].version.clone();
        let img_paths = self.handlers[self.selected_handler].img_paths.clone();
        let info = self.handlers[self.selected_handler].info.clone();
        let readme_path = self.handlers[self.selected_handler].path_handler.join("README.md");
        let readme_content = std::fs::read_to_string(&readme_path).ok();

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(icon).max_width(32.0).corner_radius(4));
            ui.add_space(8.0);
            ui.heading(display);
        });
        ui.add_space(8.0);
        ui.separator();

        ui.add_space(8.0);
        let mut play_clicked = false;
        let mut edit_clicked = false;

        // Pane-based focus for action bar
        let is_action_bar_focused = self.focus_pane == FocusPane::ActionBar;
        let activate = self.activate_focused && is_action_bar_focused;
        let focus_color = egui::Color32::from_rgb(100, 200, 255);

        ui.horizontal(|ui| {
            // Play button (action_bar_index = 0)
            let play_focused = is_action_bar_focused && self.action_bar_index == 0;
            let playbtn = ui.add(
                egui::Button::image_and_text(
                    egui::include_image!("../../res/BTN_START.png"),
                    " Play ",
                )
                .min_size(egui::vec2(100.0, 32.0))
                .stroke(if play_focused {
                    egui::Stroke::new(2.0, focus_color)
                } else {
                    egui::Stroke::NONE
                }),
            );
            if playbtn.clicked() || (play_focused && activate) {
                play_clicked = true;
            }

            ui.add(egui::Separator::default().vertical());

            // Profile display (Y button to change) (action_bar_index = 1)
            ui.label("Profile:");
            let current_profile_idx = self.get_current_profile();
            let profile_text = if self.profiles.is_empty() {
                "No profiles".to_string()
            } else if current_profile_idx < self.profiles.len() {
                self.profiles[current_profile_idx].clone()
            } else {
                "Select...".to_string()
            };

            // Show profile button (highlight if focused or dropdown open)
            let profile_focused = is_action_bar_focused && self.action_bar_index == 1;
            let profile_btn = ui.add(
                egui::Button::new(format!("  {}  ", profile_text))
                    .min_size(egui::vec2(100.0, 28.0))
                    .stroke(if self.profile_dropdown_open || profile_focused {
                        egui::Stroke::new(2.0, focus_color)
                    } else {
                        egui::Stroke::NONE
                    }),
            );
            if profile_btn.clicked() || (profile_focused && activate) {
                self.profile_dropdown_selection = current_profile_idx;
                self.profile_dropdown_open = !self.profile_dropdown_open;
            }
            profile_btn.on_hover_text("Press Y to change profile");

            ui.add(egui::Separator::default().vertical());

            // Edit button (action_bar_index = 2)
            let edit_focused = is_action_bar_focused && self.action_bar_index == 2;
            let editbtn = ui.add(
                egui::Button::image_and_text(
                    egui::include_image!("../../res/BTN_WEST.png"),
                    " Edit ",
                )
                .min_size(egui::vec2(80.0, 32.0))
                .stroke(if edit_focused {
                    egui::Stroke::new(2.0, focus_color)
                } else {
                    egui::Stroke::NONE
                }),
            );
            if editbtn.clicked() || (edit_focused && activate) {
                edit_clicked = true;
            }

            ui.add(egui::Separator::default().vertical());
            if is_win {
                ui.label(" Proton");
            } else {
                ui.label("Native");
            }
            if !author.is_empty() {
                ui.add(egui::Separator::default().vertical());
                ui.label(format!("Author: {}", author));
            }
            if !version.is_empty() {
                ui.add(egui::Separator::default().vertical());
                ui.label(format!("Version: {}", version));
            }
        });

        // Handle button clicks after UI closure
        if play_clicked {
            self.check_and_start_game();
        }
        if edit_clicked {
            self.handler_edit = Some(self.handlers[self.selected_handler].clone());
            self.show_edit_modal = true;
        }

        // Game images
        if !img_paths.is_empty() {
            ui.add_space(8.0);
            egui::ScrollArea::horizontal()
                .max_width(f32::INFINITY)
                .show(ui, |ui| {
                    let available_height = (ui.available_height() - 100.0).max(150.0);
                    ui.horizontal(|ui| {
                        for img in img_paths.iter() {
                            ui.add(
                                egui::Image::new(format!("file://{}", img.display()))
                                    .fit_to_exact_size(egui::vec2(
                                        available_height * 1.77,
                                        available_height,
                                    ))
                                    .maintain_aspect_ratio(true),
                            );
                        }
                    });
                });
        }

        // README or info text at bottom
        let has_readme = readme_content.is_some();
        let has_info = !info.is_empty();

        if has_readme || has_info {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - 20.0)
                .auto_shrink(false)
                .show(ui, |ui| {
                    // Show README if available (takes priority)
                    if let Some(readme) = &readme_content {
                        // Simple markdown-ish rendering
                        for line in readme.lines() {
                            if line.starts_with("# ") {
                                ui.add_space(4.0);
                                ui.label(RichText::new(&line[2..]).strong().size(18.0));
                                ui.add_space(2.0);
                            } else if line.starts_with("## ") {
                                ui.add_space(6.0);
                                ui.label(RichText::new(&line[3..]).strong().size(15.0));
                                ui.add_space(2.0);
                            } else if line.starts_with("### ") {
                                ui.add_space(4.0);
                                ui.label(RichText::new(&line[4..]).strong());
                                ui.add_space(1.0);
                            } else if line.starts_with("- ") {
                                ui.horizontal(|ui| {
                                    ui.label("  *");
                                    ui.label(&line[2..]);
                                });
                            } else if line.starts_with("```") {
                                // Skip code fence markers
                            } else if line.trim().is_empty() {
                                ui.add_space(4.0);
                            } else {
                                ui.label(line);
                            }
                        }
                    } else if has_info {
                        // Fallback to info field
                        ui.label(&info);
                    }
                });
        }
    }

    fn check_and_start_game(&mut self) {
        let h = cur_handler!(self);
        if h.spec_ver != HANDLER_SPEC_CURRENT_VERSION {
            let mismatch = match h.spec_ver < HANDLER_SPEC_CURRENT_VERSION {
                true => "an older",
                false => "a newer",
            };
            let mismatch2 = match h.spec_ver < HANDLER_SPEC_CURRENT_VERSION {
                true => "Up-to-date handlers can be found by clicking the download button on the top bar of the launcher.",
                false => "It is recommended to update Splitux to the latest version.",
            };
            msg(
                "Handler version mismatch",
                &format!("This handler was meant for use with {} version of Splitux; you may experience issues or the game may not work at all. {} If everything still works fine, you can prevent this message appearing in the future by editing the handler, updating the spec version and saving.",
                    mismatch, mismatch2
                )
            );
        }
        self.start_game_setup();
    }
}
