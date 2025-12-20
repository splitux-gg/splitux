//! Profiles settings section (options 20+)

use crate::app::app::{ActiveDropdown, Splitux};
use crate::app::theme;
use crate::profile_prefs::ProfilePreferences;
use crate::profiles::{delete_profile, rename_profile, scan_profiles};
use crate::ui::components::dropdown::{render_gamepad_dropdown, DropdownItem};
use crate::util::{msg, yesno};
use eframe::egui::{self, RichText, Ui};

/// Controller preference dropdown action
#[derive(Clone)]
enum ControllerAction {
    Clear,
    SetDevice { uniq: String, name: String },
}

/// Audio preference dropdown action
#[derive(Clone)]
enum AudioAction {
    Clear,
    SetDevice { name: String, description: String },
}

impl Splitux {
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
                                    self.active_dropdown = None;
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

                            // Controller preference (sub_focus = 1)
                            let ctrl_focused = is_focused && sub_focus == 1;
                            let ctrl_combo_open = self.active_dropdown == Some(ActiveDropdown::ProfileController(i));

                            ui.horizontal(|ui| {
                                ui.label("🎮 Controller:");

                                // Build items list (filtered to devices with uniq)
                                let mut ctrl_items: Vec<DropdownItem<ControllerAction>> = vec![
                                    DropdownItem::new(ControllerAction::Clear, "None", !prefs.has_controller())
                                ];
                                for (dev_idx, device) in self.input_devices.iter().enumerate() {
                                    let uniq = device.uniq();
                                    if !uniq.is_empty() {
                                        let display_name = self.device_display_name(dev_idx);
                                        let is_selected = prefs.preferred_controller.as_ref() == Some(&uniq.to_string());
                                        ctrl_items.push(DropdownItem::new(
                                            ControllerAction::SetDevice { uniq: uniq.to_string(), name: display_name.to_string() },
                                            display_name,
                                            is_selected,
                                        ));
                                    }
                                }

                                // Button text with offline indicator
                                let ctrl_text = prefs.preferred_controller_name
                                    .as_ref()
                                    .map(|n| {
                                        let connected = self.input_devices.iter()
                                            .any(|d| prefs.preferred_controller.as_ref() == Some(&d.uniq().to_string()));
                                        if connected { n.clone() } else { format!("{} (offline)", n) }
                                    })
                                    .unwrap_or_else(|| "None".to_string());

                                let ctrl_response = render_gamepad_dropdown(
                                    ui, &format!("profile_ctrl_{}", i), &ctrl_text, 180.0,
                                    &ctrl_items, ctrl_focused, ctrl_combo_open,
                                    self.dropdown_selection_idx, ctrl_focused && activate,
                                );

                                // Handle response
                                if let Some(action) = ctrl_response.selected {
                                    let mut new_prefs = ProfilePreferences::load(profile_name);
                                    match action {
                                        ControllerAction::Clear => new_prefs.clear_controller(),
                                        ControllerAction::SetDevice { uniq, name } => new_prefs.set_controller(&uniq, &name),
                                    }
                                    let _ = new_prefs.save(profile_name);
                                    self.active_dropdown = None;
                                } else if ctrl_response.toggled || (ctrl_focused && activate && !ctrl_combo_open) {
                                    self.active_dropdown = if ctrl_combo_open { None } else { Some(ActiveDropdown::ProfileController(i)) };
                                    if !ctrl_combo_open { self.dropdown_selection_idx = 0; }
                                }
                            });

                            ui.add_space(2.0);

                            // Audio preference (sub_focus = 2)
                            let audio_focused = is_focused && sub_focus == 2;
                            let audio_combo_open = self.active_dropdown == Some(ActiveDropdown::ProfileAudio(i));

                            ui.horizontal(|ui| {
                                ui.label("🔊 Audio:");

                                // Build items list
                                let mut audio_items: Vec<DropdownItem<AudioAction>> = vec![
                                    DropdownItem::new(AudioAction::Clear, "None", prefs.preferred_audio.is_none())
                                ];
                                for device in self.audio_devices.iter() {
                                    let is_selected = prefs.preferred_audio.as_ref() == Some(&device.name);
                                    audio_items.push(DropdownItem::new(
                                        AudioAction::SetDevice { name: device.name.clone(), description: device.description.clone() },
                                        &device.description,
                                        is_selected,
                                    ));
                                }

                                // Button text with offline indicator
                                let audio_text = prefs.preferred_audio_name
                                    .as_ref()
                                    .map(|n| {
                                        let connected = self.audio_devices.iter()
                                            .any(|d| prefs.preferred_audio.as_ref() == Some(&d.name));
                                        if connected { n.clone() } else { format!("{} (offline)", n) }
                                    })
                                    .unwrap_or_else(|| "None".to_string());

                                let audio_response = render_gamepad_dropdown(
                                    ui, &format!("profile_audio_{}", i), &audio_text, 180.0,
                                    &audio_items, audio_focused, audio_combo_open,
                                    self.dropdown_selection_idx, audio_focused && activate,
                                );

                                // Handle response
                                if let Some(action) = audio_response.selected {
                                    let mut new_prefs = ProfilePreferences::load(profile_name);
                                    match action {
                                        AudioAction::Clear => new_prefs.clear_audio(),
                                        AudioAction::SetDevice { name, description } => new_prefs.set_audio(&name, &description),
                                    }
                                    let _ = new_prefs.save(profile_name);
                                    self.active_dropdown = None;
                                } else if audio_response.toggled || (audio_focused && activate && !audio_combo_open) {
                                    self.active_dropdown = if audio_combo_open { None } else { Some(ActiveDropdown::ProfileAudio(i)) };
                                    if !audio_combo_open { self.dropdown_selection_idx = 0; }
                                }
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
}
