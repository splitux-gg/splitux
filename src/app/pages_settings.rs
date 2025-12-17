// Settings page display functions

use super::app::{SettingsFocus, Splitux};
use super::config::{save_cfg, PartyConfig, PadFilterType, WindowManagerType};
use super::theme;
use crate::audio::{resolve_audio_system, scan_sinks, AudioSystemPreference};
use crate::input::scan_input_devices;
use crate::paths::PATH_PARTY;
use crate::profile_prefs::ProfilePreferences;
use crate::profiles::{delete_profile, rename_profile, scan_profiles};
use crate::ui::responsive::LayoutMode;
use crate::util::{msg, yesno};
use eframe::egui::{self, RichText, Ui};

impl Splitux {
    /// Check if a settings option is currently focused
    fn is_settings_option_focused(&self, index: usize) -> bool {
        self.settings_focus == SettingsFocus::Options && self.settings_option_index == index
    }

    /// Scroll to focused option only when focus changed (clears the flag after scrolling)
    fn scroll_to_settings_option_if_needed(&mut self, index: usize, response: &egui::Response) {
        if self.settings_scroll_to_focus && self.is_settings_option_focused(index) {
            response.scroll_to_me(Some(egui::Align::Center));
            self.settings_scroll_to_focus = false;
        }
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

                ui.add_space(16.0);
                ui.separator();

                // === Profiles Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("Profiles").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_profiles(ui);

                ui.add_space(16.0);
                ui.separator();

                // === Devices Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("Controllers").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_devices(ui);

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
                    self.refresh_device_display_names();
                }
            });
            ui.separator();
        });
    }

    pub fn display_settings_general(&mut self, ui: &mut Ui) {
        let layout_mode = LayoutMode::from_ui(ui);
        let is_narrow = layout_mode.is_narrow();

        // Option 0: Window Manager
        let frame_resp = self.settings_option_frame(0).show(ui, |ui| {
            let show_radios = |ui: &mut Ui| -> (egui::Response, egui::Response, egui::Response, egui::Response) {
                let r1 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::Auto, "Auto");
                let r2 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::KWin, "KWin");
                let r3 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::Hyprland, "Hyprland");
                let r4 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::GamescopeOnly, "None");
                (r1, r2, r3, r4)
            };

            let wm_label = ui.label("Window Manager");
            let (r1, r2, r3, r4) = if is_narrow {
                ui.horizontal_wrapped(show_radios)
            } else {
                ui.horizontal(show_radios)
            }.inner;

            if wm_label.hovered() || r1.hovered() || r2.hovered() || r3.hovered() || r4.hovered() || self.is_settings_option_focused(0) {
                self.infotext = "DEFAULT: Auto\n\nSelect window manager for positioning game windows. Auto detects your WM. Use 'None' for manual positioning or Gamescope-only mode.".to_string();
            }
        });
        self.scroll_to_settings_option_if_needed(0, &frame_resp.response);

        // Option 1: Controller filter
        let r = self.settings_option_frame(1).show(ui, |ui| {
            let filter_label = ui.label("Controller filter");
            let (r1, r2, r3) = if is_narrow {
                ui.horizontal_wrapped(|ui| {
                    let r1 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::All, "All");
                    let r2 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::NoSteamInput, "No Steam");
                    let r3 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::OnlySteamInput, "Steam Only");
                    (r1, r2, r3)
                }).inner
            } else {
                ui.horizontal(|ui| {
                    let r1 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::All, "All controllers");
                    let r2 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::NoSteamInput, "No Steam Input");
                    let r3 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::OnlySteamInput, "Only Steam Input");
                    (r1, r2, r3)
                }).inner
            };

            if filter_label.hovered() || r1.hovered() || r2.hovered() || r3.hovered() || self.is_settings_option_focused(1) {
                self.infotext = "DEFAULT: No Steam Input\n\nSelect which controllers to filter out.".to_string();
            }

            if r1.clicked() || r2.clicked() || r3.clicked() {
                self.input_devices = scan_input_devices(&self.options.pad_filter_type);
                self.refresh_device_display_names();
            }
        });
        self.scroll_to_settings_option_if_needed(1, &r.response);

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
        self.scroll_to_settings_option_if_needed(2, &r.response);

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
        self.scroll_to_settings_option_if_needed(3, &r.response);

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
        self.scroll_to_settings_option_if_needed(4, &r.response);

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
        self.scroll_to_settings_option_if_needed(5, &r.response);

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
        self.scroll_to_settings_option_if_needed(6, &r.response);

        // Option 7: Open Splitux Data Folder
        let r = self.settings_option_frame(7).show(ui, |ui| {
            let btn = ui.button("Open Splitux Data Folder");
            if btn.clicked() || (self.is_settings_option_focused(7) && self.activate_focused) {
                let _ = std::process::Command::new("xdg-open").arg(PATH_PARTY.clone()).status();
            }
        });
        self.scroll_to_settings_option_if_needed(7, &r.response);

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
        self.scroll_to_settings_option_if_needed(8, &r.response);

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
        self.scroll_to_settings_option_if_needed(9, &r.response);

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
        self.scroll_to_settings_option_if_needed(10, &r.response);

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
        self.scroll_to_settings_option_if_needed(11, &r.response);

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
        self.scroll_to_settings_option_if_needed(12, &r.response);
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
                ui.label(format!("{} devices found", self.audio_devices.len()));
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

    pub fn display_settings_profiles(&mut self, ui: &mut Ui) {
        ui.label("Manage player profiles for split-screen gaming.");
        ui.add_space(8.0);

        // Option 20: New Profile button
        let r = self.settings_option_frame(20).show(ui, |ui| {
            let is_focused = self.is_settings_option_focused(20);
            let mut btn = egui::Button::new("+ New Profile");
            if is_focused {
                btn = btn.stroke(theme::focus_stroke());
            }
            let response = ui.add(btn);
            if response.clicked() || (is_focused && self.activate_focused) {
                self.show_new_profile_dialog = true;
            }
        });
        self.scroll_to_settings_option_if_needed(20, &r.response);

        ui.add_space(8.0);

        // Profile list (options 21+)
        if self.profiles.is_empty() {
            ui.label(RichText::new("No profiles created yet.").weak());
        } else {
            // Clone profiles to avoid borrow issues
            let profiles_list = self.profiles.clone();
            let master_profile = self.options.master_profile.clone();

            for (i, profile_name) in profiles_list.iter().enumerate() {
                let option_index = 21 + i;
                let is_focused = self.is_settings_option_focused(option_index);
                let is_master = master_profile.as_ref() == Some(profile_name);
                let is_renaming = self.profile_edit_index == Some(i);
                let is_expanded = self.profile_prefs_expanded == Some(i);

                let r = self.settings_option_frame(option_index).show(ui, |ui| {
                    // Main profile row
                    ui.horizontal(|ui| {
                        if is_renaming {
                            // Rename mode: show text input
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut self.profile_rename_buffer)
                                    .desired_width(150.0)
                                    .hint_text("New name"),
                            );

                            // Auto-focus the text field
                            edit.request_focus();

                            if ui.button("Save").clicked()
                                || (edit.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            {
                                // Apply rename
                                let new_name = self.profile_rename_buffer.trim().to_string();
                                if !new_name.is_empty() && new_name != *profile_name {
                                    match rename_profile(profile_name, &new_name) {
                                        Ok(()) => {
                                            // Update master profile if renamed
                                            if self.options.master_profile.as_ref()
                                                == Some(profile_name)
                                            {
                                                self.options.master_profile = Some(new_name);
                                            }
                                            self.profiles = scan_profiles(false);
                                        }
                                        Err(e) => {
                                            msg("Rename Failed", &e.to_string());
                                        }
                                    }
                                }
                                self.profile_edit_index = None;
                                self.profile_rename_buffer.clear();
                            }

                            if ui.button("Cancel").clicked()
                                || ui.input(|i| i.key_pressed(egui::Key::Escape))
                            {
                                self.profile_edit_index = None;
                                self.profile_rename_buffer.clear();
                            }
                        } else {
                            // Expand/collapse toggle
                            // Only activate via gamepad when sub_focus == 0 (on the header row)
                            let expand_icon = if is_expanded { "▼" } else { "▶" };
                            let gamepad_activate = is_focused && self.activate_focused && self.profile_prefs_focus == 0;
                            if ui.button(expand_icon)
                                .on_hover_text(if is_expanded { "Collapse preferences" } else { "Edit preferences" })
                                .clicked()
                                || gamepad_activate
                            {
                                self.profile_prefs_expanded = if is_expanded { None } else { Some(i) };
                                // Close any open dropdowns when collapsing
                                if is_expanded {
                                    self.profile_ctrl_combo_open = None;
                                    self.profile_audio_combo_open = None;
                                }
                                // Reset sub-focus when expanding
                                self.profile_prefs_focus = 0;
                            }

                            // Profile name with master indicator
                            let master_icon = if is_master { "★ " } else { "" };
                            ui.label(format!("{}{}", master_icon, profile_name));

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Always show action buttons (visible for both mouse and gamepad users)
                                    // Delete button (X on gamepad)
                                    if ui.button("Delete").clicked() {
                                        self.profile_delete_confirm = Some(i);
                                    }

                                    // Rename button (Y on gamepad)
                                    if ui.button("Rename").clicked() {
                                        self.profile_edit_index = Some(i);
                                        self.profile_rename_buffer = profile_name.clone();
                                    }

                                    // Set as master toggle
                                    if is_master {
                                        if ui.button("Unset Master").clicked() {
                                            self.options.master_profile = None;
                                        }
                                    } else if ui.button("Set Master").clicked() {
                                        self.options.master_profile =
                                            Some(profile_name.clone());
                                    }
                                },
                            );
                        }
                    });

                    // Expanded preferences section
                    if is_expanded && !is_renaming {
                        ui.add_space(4.0);
                        ui.indent("profile_prefs", |ui| {
                            let prefs = ProfilePreferences::load(profile_name);
                            let sub_focus = self.profile_prefs_focus;
                            let activate = self.activate_focused;
                            let focus_stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 255));

                            // Controller preference (sub_focus = 1)
                            let ctrl_focused = is_focused && sub_focus == 1;
                            let ctrl_combo_open = self.profile_ctrl_combo_open == Some(i);

                            // Toggle combo open state when A pressed
                            if ctrl_focused && activate && !ctrl_combo_open {
                                self.profile_ctrl_combo_open = Some(i);
                                self.profile_audio_combo_open = None; // Close other combo
                                self.profile_dropdown_selection_idx = 0; // Reset selection
                            }

                            let ctrl_frame = if ctrl_focused {
                                egui::Frame::NONE
                                    .stroke(focus_stroke)
                                    .inner_margin(4.0)
                                    .rounding(4.0)
                            } else {
                                egui::Frame::NONE.inner_margin(4.0)
                            };

                            ctrl_frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("🎮 Controller:");

                                    let ctrl_text = prefs.preferred_controller_name
                                        .as_ref()
                                        .map(|n| {
                                            let connected = self.input_devices.iter()
                                                .any(|d| prefs.preferred_controller.as_ref() == Some(&d.uniq().to_string()));
                                            if connected { n.clone() } else { format!("{} (offline)", n) }
                                        })
                                        .unwrap_or_else(|| "None".to_string());

                                    // Button that shows current selection and opens popup
                                    let btn = ui.add_sized(
                                        [180.0, 24.0],
                                        egui::Button::new(format!("{} ▼", ctrl_text))
                                    );

                                    if btn.clicked() {
                                        self.profile_ctrl_combo_open = if ctrl_combo_open { None } else { Some(i) };
                                        self.profile_audio_combo_open = None;
                                    }

                                    // Show popup if open
                                    if self.profile_ctrl_combo_open == Some(i) {
                                        let popup_id = ui.make_persistent_id(format!("ctrl_popup_{}", i));
                                        let selection_idx = self.profile_dropdown_selection_idx;
                                        let activate_selection = ctrl_focused && activate;

                                        egui::popup_below_widget(ui, popup_id, &btn, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                                            ui.set_min_width(180.0);

                                            // "None" option (index 0)
                                            let none_highlighted = selection_idx == 0;
                                            let none_response = ui.selectable_label(
                                                prefs.preferred_controller.is_none() || none_highlighted,
                                                if none_highlighted { "▶ None" } else { "  None" }
                                            );
                                            if none_response.clicked() || (activate_selection && none_highlighted) {
                                                let mut new_prefs = ProfilePreferences::load(profile_name);
                                                new_prefs.clear_controller();
                                                let _ = new_prefs.save(profile_name);
                                                self.profile_ctrl_combo_open = None;
                                            }

                                            ui.separator();

                                            // Device options (index 1+)
                                            let mut device_index = 1usize;
                                            for (dev_idx, device) in self.input_devices.iter().enumerate() {
                                                let uniq = device.uniq();
                                                if !uniq.is_empty() {
                                                    let display_name = self.device_display_name(dev_idx);
                                                    let is_selected = prefs.preferred_controller.as_ref() == Some(&uniq.to_string());
                                                    let is_highlighted = selection_idx == device_index;
                                                    let label = if is_highlighted {
                                                        format!("▶ {}", display_name)
                                                    } else {
                                                        format!("  {}", display_name)
                                                    };

                                                    let response = ui.selectable_label(is_selected || is_highlighted, label);
                                                    if response.clicked() || (activate_selection && is_highlighted) {
                                                        let mut new_prefs = ProfilePreferences::load(profile_name);
                                                        new_prefs.set_controller(uniq, display_name);
                                                        let _ = new_prefs.save(profile_name);
                                                        self.profile_ctrl_combo_open = None;
                                                    }
                                                    device_index += 1;
                                                }
                                            }
                                        });
                                        // Keep popup open
                                        ui.memory_mut(|mem| mem.open_popup(popup_id));
                                    }
                                });
                            });

                            ui.add_space(2.0);

                            // Audio preference (sub_focus = 2)
                            let audio_focused = is_focused && sub_focus == 2;
                            let audio_combo_open = self.profile_audio_combo_open == Some(i);

                            // Toggle combo open state when A pressed
                            if audio_focused && activate && !audio_combo_open {
                                self.profile_audio_combo_open = Some(i);
                                self.profile_ctrl_combo_open = None; // Close other combo
                                self.profile_dropdown_selection_idx = 0; // Reset selection
                            }

                            let audio_frame = if audio_focused {
                                egui::Frame::NONE
                                    .stroke(focus_stroke)
                                    .inner_margin(4.0)
                                    .rounding(4.0)
                            } else {
                                egui::Frame::NONE.inner_margin(4.0)
                            };

                            audio_frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("🔊 Audio:");

                                    let audio_text = prefs.preferred_audio_name
                                        .as_ref()
                                        .map(|n| {
                                            let connected = self.audio_devices.iter()
                                                .any(|d| prefs.preferred_audio.as_ref() == Some(&d.name));
                                            if connected { n.clone() } else { format!("{} (offline)", n) }
                                        })
                                        .unwrap_or_else(|| "None".to_string());

                                    // Button that shows current selection and opens popup
                                    let btn = ui.add_sized(
                                        [180.0, 24.0],
                                        egui::Button::new(format!("{} ▼", audio_text))
                                    );

                                    if btn.clicked() {
                                        self.profile_audio_combo_open = if audio_combo_open { None } else { Some(i) };
                                        self.profile_ctrl_combo_open = None;
                                    }

                                    // Show popup if open
                                    if self.profile_audio_combo_open == Some(i) {
                                        let popup_id = ui.make_persistent_id(format!("audio_popup_{}", i));
                                        let selection_idx = self.profile_dropdown_selection_idx;
                                        let activate_selection = audio_focused && activate;

                                        egui::popup_below_widget(ui, popup_id, &btn, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                                            ui.set_min_width(180.0);

                                            // "None" option (index 0)
                                            let none_highlighted = selection_idx == 0;
                                            let none_response = ui.selectable_label(
                                                prefs.preferred_audio.is_none() || none_highlighted,
                                                if none_highlighted { "▶ None" } else { "  None" }
                                            );
                                            if none_response.clicked() || (activate_selection && none_highlighted) {
                                                let mut new_prefs = ProfilePreferences::load(profile_name);
                                                new_prefs.clear_audio();
                                                let _ = new_prefs.save(profile_name);
                                                self.profile_audio_combo_open = None;
                                            }

                                            ui.separator();

                                            // Audio device options (index 1+)
                                            for (device_idx, device) in self.audio_devices.iter().enumerate() {
                                                let is_selected = prefs.preferred_audio.as_ref() == Some(&device.name);
                                                let is_highlighted = selection_idx == device_idx + 1;
                                                let label = if is_highlighted {
                                                    format!("▶ {}", device.description)
                                                } else {
                                                    format!("  {}", device.description)
                                                };

                                                let response = ui.selectable_label(is_selected || is_highlighted, label);
                                                if response.clicked() || (activate_selection && is_highlighted) {
                                                    let mut new_prefs = ProfilePreferences::load(profile_name);
                                                    new_prefs.set_audio(&device.name, &device.description);
                                                    let _ = new_prefs.save(profile_name);
                                                    self.profile_audio_combo_open = None;
                                                }
                                            }
                                        });
                                        // Keep popup open
                                        ui.memory_mut(|mem| mem.open_popup(popup_id));
                                    }
                                });
                            });
                        });
                    }
                });
                self.scroll_to_settings_option_if_needed(option_index, &r.response);
            }

            // Handle delete confirmation
            if let Some(delete_idx) = self.profile_delete_confirm {
                if let Some(profile_to_delete) = profiles_list.get(delete_idx) {
                    let is_master = master_profile.as_ref() == Some(profile_to_delete);
                    let warning = if is_master {
                        format!(
                            "Are you sure you want to delete '{}'?\n\nThis is your MASTER profile - save sync will be disabled!",
                            profile_to_delete
                        )
                    } else {
                        format!(
                            "Are you sure you want to delete '{}'?\n\nAll saves for this profile will be lost.",
                            profile_to_delete
                        )
                    };

                    if yesno("Delete Profile?", &warning) {
                        match delete_profile(profile_to_delete) {
                            Ok(()) => {
                                if is_master {
                                    self.options.master_profile = None;
                                }
                                self.profiles = scan_profiles(false);
                            }
                            Err(e) => {
                                msg("Delete Failed", &e.to_string());
                            }
                        }
                    }
                }
                self.profile_delete_confirm = None;
            }
        }

        ui.add_space(8.0);
        ui.label(
            RichText::new("Tip: Set a Master profile to sync saves with your main game installation.")
                .weak()
                .small(),
        );
    }

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
            .map(|(idx, d)| GamepadInfo {
                idx: Some(idx),
                uniq: d.uniq().to_string(),
                emoji: d.emoji().to_string(),
                hw_name: d.fancyname().to_string(),
                is_online: true,
            })
            .collect();

        // Collect unique IDs of connected devices
        let connected_uniqs: std::collections::HashSet<String> =
            gamepads.iter().map(|g| g.uniq.clone()).collect();

        // Add offline devices that have saved aliases
        for (uniq, alias) in &self.options.device_aliases {
            if !connected_uniqs.contains(uniq) {
                gamepads.push(GamepadInfo {
                    idx: None,
                    uniq: uniq.clone(),
                    emoji: "🎮".to_string(),
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
