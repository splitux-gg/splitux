// Settings page display functions

use super::app::{SettingsFocus, Splitux};
use super::config::{save_cfg, PartyConfig, PadFilterType, WindowManagerType};
use super::theme;
use crate::audio::{resolve_audio_system, scan_sinks, AudioSystemPreference};
use crate::input::scan_input_devices;
use crate::paths::PATH_PARTY;
use crate::util::{msg, yesno};
use eframe::egui::{self, RichText, Ui};

impl Splitux {
    /// Check if a settings option is currently focused
    fn is_settings_option_focused(&self, index: usize) -> bool {
        self.settings_focus == SettingsFocus::Options && self.settings_option_index == index
    }

    /// Get a frame for a settings option (with focus indicator if focused)
    fn settings_option_frame(&self, index: usize) -> egui::Frame {
        if self.is_settings_option_focused(index) {
            egui::Frame::NONE
                .fill(theme::colors::SELECTION_BG)
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(4, 2))
                .stroke(theme::focus_stroke())
        } else {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(4, 2))
        }
    }

    pub fn display_page_settings(&mut self, ui: &mut Ui) {
        self.infotext.clear();
        ui.add_space(8.0);
        ui.heading("Settings");
        ui.add_space(4.0);
        ui.label("Configure Splitux behavior and game launch options");
        ui.add_space(8.0);
        ui.separator();

        // Single scrollable settings page with sections
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(ui.available_height() - 60.0)
            .show(ui, |ui| {
                // === General Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("General").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_general(ui);

                ui.add_space(16.0);
                ui.separator();

                // === Gamescope Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("Gamescope").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_gamescope(ui);

                ui.add_space(16.0);
                ui.separator();

                // === Audio Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("Audio").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_audio(ui);

                ui.add_space(8.0);
            });

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            let is_buttons_focused = self.settings_focus == SettingsFocus::BottomButtons;
            ui.horizontal(|ui| {
                // Save Settings button
                let mut save_btn = egui::Button::new("Save Settings");
                if is_buttons_focused && self.settings_button_index == 0 {
                    save_btn = save_btn.stroke(theme::focus_stroke());
                }
                let save_response = ui.add(save_btn);
                if save_response.clicked() || (is_buttons_focused && self.settings_button_index == 0 && self.activate_focused) {
                    if let Err(e) = save_cfg(&self.options) {
                        msg("Error", &format!("Couldn't save settings: {}", e));
                    }
                }

                // Restore Defaults button
                let mut restore_btn = egui::Button::new("Restore Defaults");
                if is_buttons_focused && self.settings_button_index == 1 {
                    restore_btn = restore_btn.stroke(theme::focus_stroke());
                }
                let restore_response = ui.add(restore_btn);
                if restore_response.clicked() || (is_buttons_focused && self.settings_button_index == 1 && self.activate_focused) {
                    self.options = PartyConfig::default();
                    self.input_devices = scan_input_devices(&self.options.pad_filter_type);
                }
            });
            ui.separator();
        });
    }

    pub fn display_settings_general(&mut self, ui: &mut Ui) {
        // Option 0: Window Manager
        let frame_resp = self.settings_option_frame(0).show(ui, |ui| {
            ui.horizontal(|ui| {
                let wm_label = ui.label("Window Manager");
                let r1 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::Auto, "Auto");
                let r2 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::KWin, "KWin");
                let r3 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::Hyprland, "Hyprland");
                let r4 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::GamescopeOnly, "None");

                if wm_label.hovered() || r1.hovered() || r2.hovered() || r3.hovered() || r4.hovered() || self.is_settings_option_focused(0) {
                    self.infotext = "DEFAULT: Auto\n\nSelect window manager for positioning game windows. Auto detects your WM. Use 'None' for manual positioning or Gamescope-only mode.".to_string();
                }
            });
        });
        if self.is_settings_option_focused(0) { frame_resp.response.scroll_to_me(Some(egui::Align::Center)); }

        // Option 1: Controller filter
        let r = self.settings_option_frame(1).show(ui, |ui| {
            ui.horizontal(|ui| {
                let filter_label = ui.label("Controller filter");
                let r1 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::All, "All controllers");
                let r2 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::NoSteamInput, "No Steam Input");
                let r3 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::OnlySteamInput, "Only Steam Input");

                if filter_label.hovered() || r1.hovered() || r2.hovered() || r3.hovered() || self.is_settings_option_focused(1) {
                    self.infotext = "DEFAULT: No Steam Input\n\nSelect which controllers to filter out.".to_string();
                }

                if r1.clicked() || r2.clicked() || r3.clicked() {
                    self.input_devices = scan_input_devices(&self.options.pad_filter_type);
                }
            });
        });
        if self.is_settings_option_focused(1) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        // Option 2: Proton version
        let r = self.settings_option_frame(2).show(ui, |ui| {
            ui.horizontal(|ui| {
                let proton_ver_label = ui.label("Proton version");
                let proton_ver_editbox = ui.add(
                    egui::TextEdit::singleline(&mut self.options.proton_version).hint_text("GE-Proton"),
                );
                if proton_ver_label.hovered() || proton_ver_editbox.hovered() || self.is_settings_option_focused(2) {
                    self.infotext = "DEFAULT: GE-Proton\n\nSpecify a Proton version.".to_string();
                }
            });
        });
        if self.is_settings_option_focused(2) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        // Option 3: Separate Proton prefixes
        let r = self.settings_option_frame(3).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.proton_separate_pfxs, "Run instances in separate Proton prefixes");
            if check.hovered() || self.is_settings_option_focused(3) {
                self.infotext = "DEFAULT: Enabled\n\nRuns each instance in separate Proton prefixes.".to_string();
            }
            if self.is_settings_option_focused(3) && self.activate_focused {
                self.options.proton_separate_pfxs = !self.options.proton_separate_pfxs;
            }
        });
        if self.is_settings_option_focused(3) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        // Option 4: Allow multiple instances
        let r = self.settings_option_frame(4).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.allow_multiple_instances_on_same_device, "(Debug) Allow multiple instances from one gamepad");
            if check.hovered() || self.is_settings_option_focused(4) {
                self.infotext = "DEFAULT: Disabled\n\nAllow multiple instances on the same device.".to_string();
            }
            if self.is_settings_option_focused(4) && self.activate_focused {
                self.options.allow_multiple_instances_on_same_device = !self.options.allow_multiple_instances_on_same_device;
            }
        });
        if self.is_settings_option_focused(4) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        // Option 5: Force run from original directory
        let r = self.settings_option_frame(5).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.disable_mount_gamedirs, "(Debug) Force run instances from original game directory");
            if check.hovered() || self.is_settings_option_focused(5) {
                self.infotext = "DEFAULT: Disabled\n\nForces instances to launch from the original game directory.".to_string();
            }
            if self.is_settings_option_focused(5) && self.activate_focused {
                self.options.disable_mount_gamedirs = !self.options.disable_mount_gamedirs;
            }
        });
        if self.is_settings_option_focused(5) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        ui.separator();

        // Option 6: Erase Proton Prefix Data
        let r = self.settings_option_frame(6).show(ui, |ui| {
            let btn = ui.button("Erase All Proton Prefix Data");
            if btn.clicked() || (self.is_settings_option_focused(6) && self.activate_focused) {
                if yesno("Erase Prefix?", "This will erase all Proton prefixes. Are you sure?")
                    && PATH_PARTY.join("prefixes").exists()
                {
                    if let Err(err) = std::fs::remove_dir_all(PATH_PARTY.join("prefixes")) {
                        msg("Error", &format!("Couldn't erase pfx data: {}", err));
                    } else {
                        msg("Data Erased", "Proton prefix data successfully erased.");
                    }
                }
            }
        });
        if self.is_settings_option_focused(6) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        // Option 7: Open Splitux Data Folder
        let r = self.settings_option_frame(7).show(ui, |ui| {
            let btn = ui.button("Open Splitux Data Folder");
            if btn.clicked() || (self.is_settings_option_focused(7) && self.activate_focused) {
                let _ = std::process::Command::new("xdg-open").arg(PATH_PARTY.clone()).status();
            }
        });
        if self.is_settings_option_focused(7) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        // Photon Networking section
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("Photon Networking").strong());
        ui.add_space(4.0);

        // Option 8: PUN App ID
        let r = self.settings_option_frame(8).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("PUN App ID:");
                ui.add(egui::TextEdit::singleline(&mut self.options.photon_app_ids.pun_app_id)
                    .password(true).desired_width(280.0).hint_text("Enter your Photon PUN App ID"));
            });
            if self.is_settings_option_focused(8) {
                self.infotext = "Get a free Photon PUN App ID from dashboard.photonengine.com.".to_string();
            }
        });
        if self.is_settings_option_focused(8) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        // Option 9: Voice App ID
        let r = self.settings_option_frame(9).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Voice App ID:");
                ui.add(egui::TextEdit::singleline(&mut self.options.photon_app_ids.voice_app_id)
                    .password(true).desired_width(280.0).hint_text("Optional - for voice chat"));
            });
            if self.is_settings_option_focused(9) {
                self.infotext = "Optional: Photon Voice App ID for games that use voice chat.".to_string();
            }
        });
        if self.is_settings_option_focused(9) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        ui.horizontal(|ui| {
            ui.add_space(80.0);
            ui.hyperlink_to("Get Photon App IDs (free)", "https://dashboard.photonengine.com");
        });
    }

    pub fn display_settings_gamescope(&mut self, ui: &mut Ui) {
        // Option 10: Fix low resolution
        let r = self.settings_option_frame(10).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.gamescope_fix_lowres, "Automatically fix low resolution instances");
            if check.hovered() || self.is_settings_option_focused(10) {
                self.infotext = "Many games have graphical problems below 600p. This auto-resizes such instances.".to_string();
            }
            if self.is_settings_option_focused(10) && self.activate_focused {
                self.options.gamescope_fix_lowres = !self.options.gamescope_fix_lowres;
            }
        });
        if self.is_settings_option_focused(10) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        // Option 11: SDL backend
        let r = self.settings_option_frame(11).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.gamescope_sdl_backend, "Use SDL backend");
            if check.hovered() || self.is_settings_option_focused(11) {
                self.infotext = "Required for multi-monitor support. Disable if you see black screens on Nvidia + Wayland.".to_string();
            }
            if self.is_settings_option_focused(11) && self.activate_focused {
                self.options.gamescope_sdl_backend = !self.options.gamescope_sdl_backend;
            }
        });
        if self.is_settings_option_focused(11) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        // Option 12: Force grab cursor
        let r = self.settings_option_frame(12).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.gamescope_force_grab_cursor, "Force grab cursor for Gamescope");
            if check.hovered() || self.is_settings_option_focused(12) {
                self.infotext = "Keeps the cursor within the Gamescope window.".to_string();
            }
            if self.is_settings_option_focused(12) && self.activate_focused {
                self.options.gamescope_force_grab_cursor = !self.options.gamescope_force_grab_cursor;
            }
        });
        if self.is_settings_option_focused(12) { r.response.scroll_to_me(Some(egui::Align::Center)); }
    }

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
        if self.is_settings_option_focused(13) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        ui.add_space(4.0);

        // Option 14: Audio system selection
        let r = self.settings_option_frame(14).show(ui, |ui| {
            ui.horizontal(|ui| {
                let sys_label = ui.label("Audio System");
                let r1 = ui.radio_value(&mut self.options.audio.system, AudioSystemPreference::Auto, "Auto");
                let r2 = ui.radio_value(&mut self.options.audio.system, AudioSystemPreference::PulseAudio, "PulseAudio");
                let r3 = ui.radio_value(&mut self.options.audio.system, AudioSystemPreference::PipeWireNative, "PipeWire");

                if sys_label.hovered() || r1.hovered() || r2.hovered() || r3.hovered() || self.is_settings_option_focused(14) {
                    self.infotext = "DEFAULT: Auto\n\nSelect audio system for virtual sink management.".to_string();
                }

                if r1.clicked() || r2.clicked() || r3.clicked() {
                    self.audio_system = resolve_audio_system(self.options.audio.system);
                    self.audio_devices = scan_sinks(self.audio_system).unwrap_or_default();
                }
            });
        });
        if self.is_settings_option_focused(14) { r.response.scroll_to_me(Some(egui::Align::Center)); }

        ui.add_space(8.0);

        // Option 15: Refresh audio devices button
        let r = self.settings_option_frame(15).show(ui, |ui| {
            ui.horizontal(|ui| {
                let btn = ui.button("Refresh Audio Devices");
                if btn.clicked() || (self.is_settings_option_focused(15) && self.activate_focused) {
                    self.audio_system = resolve_audio_system(self.options.audio.system);
                    self.audio_devices = scan_sinks(self.audio_system).unwrap_or_default();
                }
                ui.label(format!("{} devices found", self.audio_devices.len()));
            });
        });
        if self.is_settings_option_focused(15) { r.response.scroll_to_me(Some(egui::Align::Center)); }

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
                if is_focused { r.response.scroll_to_me(Some(egui::Align::Center)); }
            }
        } else if self.options.audio.enabled {
            ui.label("No audio devices found. Click 'Refresh Audio Devices' to scan.");
        }
    }
}
