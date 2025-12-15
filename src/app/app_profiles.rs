// Profile management and profile-related UI

use super::app::Splitux;
use crate::profiles::*;
use crate::util::*;

use eframe::egui;

impl Splitux {
    /// Get the selected profile index for the current handler
    pub fn get_current_profile(&self) -> usize {
        if self.handlers.is_empty() {
            return 0;
        }
        let handler_key = self.handlers[self.selected_handler]
            .path_handler
            .to_string_lossy()
            .to_string();
        *self.game_profiles.get(&handler_key).unwrap_or(&0)
    }

    /// Set the selected profile index for the current handler
    pub fn set_current_profile(&mut self, profile_idx: usize) {
        if self.handlers.is_empty() {
            return;
        }
        let handler_key = self.handlers[self.selected_handler]
            .path_handler
            .to_string_lossy()
            .to_string();
        self.game_profiles.insert(handler_key, profile_idx);
    }

    pub fn display_profile_dropdown(&mut self, ctx: &egui::Context) {
        let mut selected_idx: Option<usize> = None;
        let mut show_new_profile = false;

        egui::Window::new("Select Profile")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(200.0);

                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Select Profile").strong().size(16.0));
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // List profiles
                    for i in 0..self.profiles.len() {
                        let is_selected = i == self.profile_dropdown_selection;
                        let profile_name = self.profiles[i].clone();
                        let btn = ui.add(
                            egui::Button::new(format!("  {}  ", profile_name))
                                .min_size(egui::vec2(180.0, 28.0))
                                .stroke(if is_selected {
                                    egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 255))
                                } else {
                                    egui::Stroke::NONE
                                }),
                        );
                        if btn.clicked() {
                            selected_idx = Some(i);
                        }
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // New Profile option
                    let is_new_selected = self.profile_dropdown_selection >= self.profiles.len();
                    let new_btn = ui.add(
                        egui::Button::new("+ New Profile...")
                            .min_size(egui::vec2(180.0, 28.0))
                            .stroke(if is_new_selected {
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 255))
                            } else {
                                egui::Stroke::NONE
                            }),
                    );
                    if new_btn.clicked() {
                        show_new_profile = true;
                    }

                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("A: Select  B: Cancel  Up/Down: Navigate").small().weak());
                });
            });

        // Handle selections after window closure
        if let Some(idx) = selected_idx {
            self.set_current_profile(idx);
            self.profile_dropdown_open = false;
        }
        if show_new_profile {
            self.profile_dropdown_open = false;
            self.show_new_profile_dialog = true;
        }
    }

    pub fn display_new_profile_dialog(&mut self, _ctx: &egui::Context) {
        // Use the dialog crate for input (same as existing profile creation)
        use dialog::DialogBox;

        if let Some(name) = dialog::Input::new("Enter name (must be alphanumeric):")
            .title("New Profile")
            .show()
            .expect("Could not display dialog box")
        {
            if !name.is_empty() && name.chars().all(char::is_alphanumeric) {
                if let Err(e) = create_profile(&name) {
                    msg("Error", &format!("Failed to create profile: {}", e));
                } else {
                    self.profiles = scan_profiles(false);
                    // Select the new profile
                    if let Some(idx) = self.profiles.iter().position(|p| p == &name) {
                        self.set_current_profile(idx);
                    }
                }
            } else if !name.is_empty() {
                msg("Error", "Invalid name - must be alphanumeric");
            }
        }
        self.show_new_profile_dialog = false;
    }
}
