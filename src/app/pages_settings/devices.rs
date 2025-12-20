//! Devices/Controllers settings section

use crate::app::app::Splitux;
use crate::app::theme;
use eframe::egui::{self, RichText, Ui};
use egui_phosphor::regular as icons;
use std::collections::HashSet;

impl Splitux {
    pub fn display_settings_devices(&mut self, ui: &mut Ui) {
        ui.label("Assign custom names to your controllers for easy identification.");
        ui.add_space(8.0);

        // Collect connected gamepad info (avoiding borrow issues)
        struct GamepadInfo {
            idx: Option<usize>, // None = offline device
            uniq: String,
            emoji: String,
            hw_name: String,
            is_online: bool,
        }

        // Get connected gamepads
        let mut gamepads: Vec<GamepadInfo> = self
            .input_devices
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                d.device_type() == crate::input::DeviceType::Gamepad && !d.uniq().is_empty()
            })
            .map(|(idx, d)| {
                // Use type prefix if available, otherwise fall back to fancyname
                let type_prefix = d.type_prefix();
                let hw_name = if type_prefix.is_empty() {
                    d.fancyname().to_string()
                } else {
                    format!("{} Controller", type_prefix)
                };
                GamepadInfo {
                    idx: Some(idx),
                    uniq: d.uniq().to_string(),
                    emoji: d.emoji().to_string(),
                    hw_name,
                    is_online: true,
                }
            })
            .collect();

        // Collect unique IDs of connected devices
        let connected_uniqs: HashSet<String> =
            gamepads.iter().map(|g| g.uniq.clone()).collect();

        // Add offline devices that have saved aliases
        for (uniq, alias) in &self.options.device_aliases {
            if !connected_uniqs.contains(uniq) {
                gamepads.push(GamepadInfo {
                    idx: None,
                    uniq: uniq.clone(),
                    emoji: icons::GAME_CONTROLLER.to_string(),
                    hw_name: alias.clone(), // Use alias as hw_name for offline devices
                    is_online: false,
                });
            }
        }

        if gamepads.is_empty() {
            ui.label(RichText::new("No controllers connected or saved.").weak());
            ui.add_space(4.0);
            ui.label(
                RichText::new("Connect a controller to assign it a custom name.")
                    .weak()
                    .small(),
            );
        } else {
            // Pre-compute display names
            let display_names = self.device_display_names.clone();

            for gp in gamepads {
                let current_alias = self.options.device_aliases.get(&gp.uniq).cloned();
                let display_name = if let Some(idx) = gp.idx {
                    display_names.get(idx).cloned().unwrap_or_else(|| gp.hw_name.clone())
                } else {
                    current_alias.clone().unwrap_or_else(|| gp.hw_name.clone())
                };
                let is_renaming = gp.idx.is_some() && self.device_rename_index == gp.idx;

                let frame = if gp.is_online {
                    theme::card_frame()
                } else {
                    theme::card_frame().fill(theme::colors::BG_DARK)
                };

                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if is_renaming {
                            // Rename mode
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut self.device_rename_buffer)
                                    .desired_width(180.0)
                                    .hint_text("Enter name"),
                            );
                            edit.request_focus();

                            if ui.button("Save").clicked()
                                || (edit.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            {
                                let new_name = self.device_rename_buffer.trim().to_string();
                                if !new_name.is_empty() {
                                    self.options.device_aliases.insert(gp.uniq.clone(), new_name);
                                    self.refresh_device_display_names();
                                }
                                self.device_rename_index = None;
                                self.device_rename_buffer.clear();
                            }

                            if ui.button("Cancel").clicked()
                                || ui.input(|i| i.key_pressed(egui::Key::Escape))
                            {
                                self.device_rename_index = None;
                                self.device_rename_buffer.clear();
                            }
                        } else {
                            // Display mode
                            let name_text = if gp.is_online {
                                RichText::new(format!("{} {}", gp.emoji, display_name))
                            } else {
                                RichText::new(format!("{} {} (offline)", gp.emoji, display_name)).weak()
                            };
                            ui.label(name_text);

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Clear/Forget button (if has custom alias)
                                    if current_alias.is_some() {
                                        let btn_text = if gp.is_online { "Clear" } else { "Forget" };
                                        let hover = if gp.is_online {
                                            "Remove custom name"
                                        } else {
                                            "Remove saved device"
                                        };
                                        if ui.button(btn_text).on_hover_text(hover).clicked() {
                                            self.options.device_aliases.remove(&gp.uniq);
                                            self.refresh_device_display_names();
                                        }
                                    }

                                    // Rename button (only for online devices)
                                    if gp.is_online {
                                        if ui.button("Rename").clicked() {
                                            self.device_rename_index = gp.idx;
                                            self.device_rename_buffer =
                                                current_alias.unwrap_or_else(|| gp.hw_name.clone());
                                        }

                                        // Show hardware name if different from display name
                                        if gp.hw_name != display_name {
                                            ui.label(
                                                RichText::new(format!("({})", gp.hw_name))
                                                    .weak()
                                                    .small(),
                                            );
                                        }
                                    }
                                },
                            );
                        }
                    });
                });
                ui.add_space(4.0);
            }
        }

        ui.add_space(8.0);
        ui.label(
            RichText::new("Tip: Custom names help identify controllers when you have multiple of the same type.")
                .weak()
                .small(),
        );
    }
}
