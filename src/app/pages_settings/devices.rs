//! Devices/Controllers settings section

use crate::app::app::Splitux;
use crate::config::IgnoredDevice;
use crate::ui::theme;
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

        self.display_settings_ignored_devices(ui);
    }

    /// "Ignored input devices" section. Some keyboards/mice (e.g. a ZSA Moonlander
    /// or a Ploopy trackball) expose several evdev nodes — System/Consumer Control
    /// endpoints, secondary mouse/keyboard interfaces — that aren't real seats and
    /// only clutter the device strip (or get grabbed as a phantom player). Ignoring
    /// a device drops it from scanning entirely, the in-app version of the
    /// `99-splitux-not-joystick` udev rule. Matches by exact evdev name.
    fn display_settings_ignored_devices(&mut self, ui: &mut Ui) {
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        ui.heading("Ignored Input Devices");
        ui.label(
            RichText::new(
                "Hide phantom endpoints some keyboards/mice expose so they can't be \
                 picked as a player seat. Ignored devices are dropped from scanning.",
            )
            .weak()
            .small(),
        );
        ui.add_space(8.0);

        // Mutations are deferred so we don't borrow self.options while iterating.
        let mut ignore: Option<IgnoredDevice> = None;
        let mut unignore: Option<IgnoredDevice> = None;

        // Currently-detected devices (ignored ones are already filtered out of the
        // scan, so everything here is a candidate to ignore). The kind is carried
        // so we can ignore the EXACT endpoint — same-named keyboard vs mouse nodes
        // are distinct, so ignoring one must not drop the other.
        let detected: Vec<(String, String, &'static str)> = self
            .input_devices
            .iter()
            .map(|d| (d.fancyname().to_string(), d.emoji().to_string(), d.device_type().kind_str()))
            .filter(|(name, _, _)| !name.is_empty())
            .collect();

        if detected.is_empty() {
            ui.label(RichText::new("No input devices detected.").weak());
        } else {
            for (name, emoji, kind) in &detected {
                theme::card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} {} ({})", emoji, name, kind));
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .button(format!("{} Ignore", icons::EYE_SLASH))
                                    .on_hover_text("Drop this exact device (name + kind) from input scanning")
                                    .clicked()
                                {
                                    ignore = Some(IgnoredDevice::Typed {
                                        name: name.clone(),
                                        kind: kind.to_string(),
                                    });
                                }
                            },
                        );
                    });
                });
                ui.add_space(4.0);
            }
        }

        // Currently-ignored names (may include devices that aren't plugged in).
        if !self.options.input_blacklist.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("Ignored:").strong());
            ui.add_space(4.0);
            for entry in self.options.input_blacklist.clone() {
                let label = match entry.kind() {
                    Some(kind) => format!("{} {} ({})", icons::EYE_SLASH, entry.name(), kind),
                    None => format!("{} {} (any)", icons::EYE_SLASH, entry.name()),
                };
                theme::card_frame().fill(theme::colors::BG_DARK).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).weak());
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .button("Remove")
                                    .on_hover_text("Stop ignoring this device")
                                    .clicked()
                                {
                                    unignore = Some(entry.clone());
                                }
                            },
                        );
                    });
                });
                ui.add_space(4.0);
            }
        }

        if let Some(entry) = ignore {
            if !self.options.input_blacklist.contains(&entry) {
                self.options.input_blacklist.push(entry);
            }
            self.apply_input_blacklist_change();
        }
        if let Some(entry) = unignore {
            self.options.input_blacklist.retain(|e| e != &entry);
            self.apply_input_blacklist_change();
        }
    }

    /// Re-scan input devices and persist after the ignore list changes, so the
    /// device strip updates immediately and the choice survives a restart.
    fn apply_input_blacklist_change(&mut self) {
        let devices = crate::input::scan_input_devices(
            &self.options.pad_filter_type,
            &self.options.input_blacklist,
        );
        self.set_input_devices(devices);
        let _ = crate::config::save_cfg(&self.options);
    }
}
