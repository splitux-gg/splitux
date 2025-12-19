//! Audio settings section (options 13-19)

use crate::app::app::Splitux;
use crate::audio::{resolve_audio_system, scan_sinks, AudioSystem, AudioSystemPreference};
use crate::ui::responsive::LayoutMode;
use eframe::egui::{self, Ui};

impl Splitux {
    pub fn display_settings_audio(&mut self, ui: &mut Ui) {
        // Option 13: Enable audio routing
        let r = self.settings_option_frame(13).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.audio.enabled, "Enable per-instance audio routing");
            if check.hovered() || self.is_settings_option_focused(13) {
                self.infotext = "DEFAULT: Disabled\n\nWhen enabled, each game instance can output audio to a different device.".to_string();
            }
            if self.is_settings_option_focused(13) && self.activate_focused {
                self.options.audio.enabled = !self.options.audio.enabled;
            }
        });
        self.scroll_to_settings_option_if_needed(13, &r.response);

        ui.add_space(4.0);

        // Option 14: Audio system selection
        let layout_mode = LayoutMode::from_ui(ui);
        let r = self.settings_option_frame(14).show(ui, |ui| {
            let sys_label = ui.label("Audio System");
            let (r1, r2, r3) = if layout_mode.is_narrow() {
                ui.horizontal_wrapped(|ui| {
                    let r1 = ui.radio_value(&mut self.options.audio.system, AudioSystemPreference::Auto, "Auto");
                    let r2 = ui.radio_value(&mut self.options.audio.system, AudioSystemPreference::PulseAudio, "Pulse");
                    let r3 = ui.radio_value(&mut self.options.audio.system, AudioSystemPreference::PipeWireNative, "PipeWire");
                    (r1, r2, r3)
                }).inner
            } else {
                ui.horizontal(|ui| {
                    let r1 = ui.radio_value(&mut self.options.audio.system, AudioSystemPreference::Auto, "Auto");
                    let r2 = ui.radio_value(&mut self.options.audio.system, AudioSystemPreference::PulseAudio, "PulseAudio");
                    let r3 = ui.radio_value(&mut self.options.audio.system, AudioSystemPreference::PipeWireNative, "PipeWire");
                    (r1, r2, r3)
                }).inner
            };

            if sys_label.hovered() || r1.hovered() || r2.hovered() || r3.hovered() || self.is_settings_option_focused(14) {
                self.infotext = "DEFAULT: Auto\n\nSelect audio system for virtual sink management.".to_string();
            }

            if r1.clicked() || r2.clicked() || r3.clicked() {
                self.audio_system = resolve_audio_system(self.options.audio.system);
                self.audio_devices = scan_sinks(self.audio_system).unwrap_or_default();
            }
        });
        self.scroll_to_settings_option_if_needed(14, &r.response);

        ui.add_space(8.0);

        // Option 15: Refresh audio devices button
        let r = self.settings_option_frame(15).show(ui, |ui| {
            ui.horizontal(|ui| {
                let btn = ui.button("Refresh Audio Devices");
                if btn.clicked() || (self.is_settings_option_focused(15) && self.activate_focused) {
                    self.audio_system = resolve_audio_system(self.options.audio.system);
                    self.audio_devices = scan_sinks(self.audio_system).unwrap_or_default();
                }
                // Show detected system status
                let status = if self.audio_system != AudioSystem::None {
                    format!("Detected: {} ({} devices)", self.audio_system.name(), self.audio_devices.len())
                } else {
                    "No audio system detected".to_string()
                };
                ui.label(status);
            });
        });
        self.scroll_to_settings_option_if_needed(15, &r.response);

        ui.add_space(8.0);

        // Show available audio devices (read-only list, not navigable)
        if !self.audio_devices.is_empty() {
            ui.label("Available Audio Outputs:");
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(100.0)
                .show(ui, |ui| {
                    for device in &self.audio_devices {
                        let icon = match device.device_type {
                            crate::audio::AudioDeviceType::Speaker => "🔊",
                            crate::audio::AudioDeviceType::Headphone => "🎧",
                            crate::audio::AudioDeviceType::Hdmi => "📺",
                            crate::audio::AudioDeviceType::Bluetooth => "📶",
                            crate::audio::AudioDeviceType::Virtual => "🔈",
                            crate::audio::AudioDeviceType::Unknown => "🔉",
                        };
                        let default_marker = if device.is_default { " (default)" } else { "" };
                        ui.label(format!("{} {}{}", icon, device.description, default_marker));
                    }
                });

            ui.add_space(8.0);

            // Instance audio assignments
            ui.label("Instance Audio Assignments:");
            ui.add_space(4.0);

            // Options 16-19: Instance 1-4 audio assignments
            for instance_idx in 0..4usize {
                let option_index = 16 + instance_idx;
                let is_focused = self.is_settings_option_focused(option_index);
                let r = self.settings_option_frame(option_index).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("Instance {}:", instance_idx + 1));

                        let current_assignment = self
                            .options
                            .audio
                            .default_assignments
                            .get(&instance_idx)
                            .cloned();
                        let current_label = current_assignment
                            .as_ref()
                            .and_then(|name| {
                                self.audio_devices
                                    .iter()
                                    .find(|d| &d.name == name)
                                    .map(|d| d.description.clone())
                            })
                            .unwrap_or_else(|| "Default".to_string());

                        egui::ComboBox::from_id_salt(format!("audio_instance_{}", instance_idx))
                            .selected_text(&current_label)
                            .show_ui(ui, |ui| {
                                // Default option (no assignment)
                                if ui
                                    .selectable_label(current_assignment.is_none(), "Default")
                                    .clicked()
                                {
                                    self.options.audio.default_assignments.remove(&instance_idx);
                                }

                                // Available devices
                                for device in &self.audio_devices {
                                    let is_selected =
                                        current_assignment.as_ref() == Some(&device.name);
                                    if ui
                                        .selectable_label(is_selected, &device.description)
                                        .clicked()
                                    {
                                        self.options
                                            .audio
                                            .default_assignments
                                            .insert(instance_idx, device.name.clone());
                                    }
                                }
                            });
                    });
                });
                if is_focused && self.settings_scroll_to_focus {
                    r.response.scroll_to_me(Some(egui::Align::Center));
                    self.settings_scroll_to_focus = false;
                }
            }
        } else if self.options.audio.enabled {
            ui.label("No audio devices found. Click 'Refresh Audio Devices' to scan.");
        }
    }
}
