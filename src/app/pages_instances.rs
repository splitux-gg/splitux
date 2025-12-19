//! Instance setup page display functions
//!
//! This module is split into submodules:
//! - `audio` - Audio conflict detection and effective audio resolution
//! - `help_bar` - Controls help bar UI
//! - `launch_options` - Bottom bar with start button and launch options

mod audio;
mod help_bar;
mod launch_options;

use super::app::{InstanceFocus, Splitux};
use super::config::save_cfg;
use super::theme;
use crate::input::{find_device_by_uniq, is_device_assigned};
use crate::profile_prefs::ProfilePreferences;
use crate::ui::focus::types::InstanceCardFocus;
use crate::ui::responsive::{combo_width, LayoutMode};
use eframe::egui::{self, RichText, Ui};

/// Check if a specific element in an instance card is focused (pure function)
fn is_element_focused(focus: &InstanceFocus, instance_idx: usize, element: InstanceCardFocus) -> bool {
    matches!(focus, InstanceFocus::InstanceCard(i, e) if *i == instance_idx && *e == element)
}

/// Get focus stroke for an element (accent color if focused)
fn element_focus_stroke(focus: &InstanceFocus, instance_idx: usize, element: InstanceCardFocus) -> egui::Stroke {
    if is_element_focused(focus, instance_idx, element) {
        theme::focus_stroke()
    } else {
        egui::Stroke::NONE
    }
}

impl Splitux {

    pub fn display_page_instances(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.heading("Instance Setup");
        ui.add_space(4.0);
        ui.label("Connect your controllers and assign them to player instances");
        ui.add_space(8.0);
        ui.separator();

        // Controls help bar (extracted)
        self.display_instance_help_bar(ui);

        // Display warnings
        self.display_instance_warnings(ui);

        // Ensure prev_profile_selections matches instances count
        while self.prev_profile_selections.len() < self.instances.len() {
            self.prev_profile_selections.push(usize::MAX);
        }
        self.prev_profile_selections.truncate(self.instances.len());

        let mut devices_to_remove: Vec<(usize, usize)> = Vec::new();
        let mut profile_changes: Vec<(usize, usize)> = Vec::new();

        // Pre-compute audio conflicts before the mutable borrow of instances
        let audio_conflicts = self.detect_audio_conflicts();

        // Pre-compute effective audio for each instance
        let effective_audio: Vec<Option<(String, String, bool)>> = (0..self.instances.len())
            .map(|i| self.get_effective_audio(i))
            .collect();

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

        // Pre-compute focus state before iterating
        let current_focus = self.instance_focus.clone();
        let activate_focused = self.activate_focused;
        let display_names = self.device_display_names.clone();

        // Render instance cards
        for (i, instance) in &mut self.instances.iter_mut().enumerate() {
            let player_color = player_colors.get(i).copied().unwrap_or(theme::colors::ACCENT);
            let card_focused = matches!(&current_focus, InstanceFocus::InstanceCard(idx, _) if *idx == i);
            let card_stroke = if card_focused {
                egui::Stroke::new(3.0, theme::colors::ACCENT)
            } else {
                egui::Stroke::new(2.0, player_color)
            };

            let card_mode = LayoutMode::from_ui(ui);
            let profile_width = combo_width(ui, 120.0, 70.0);
            let monitor_width = combo_width(ui, 100.0, 60.0);

            theme::card_frame()
                .stroke(card_stroke)
                .show(ui, |ui| {
                    // Row 1: Player label + Profile dropdown + Master indicator
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("P{}", i + 1)).strong().size(18.0).color(player_color));
                        ui.add_space(8.0);

                        // Profile dropdown with focus indicator
                        if !card_mode.is_narrow() {
                            ui.label("Profile:");
                        }
                        egui::Frame::NONE
                            .inner_margin(2.0)
                            .stroke(element_focus_stroke(&current_focus, i, InstanceCardFocus::Profile))
                            .show(ui, |ui| {
                            egui::ComboBox::from_id_salt(format!("{i}"))
                                .width(profile_width)
                                .show_index(
                                    ui,
                                    &mut instance.profselection,
                                    self.profiles.len(),
                                    |j| self.profiles[j].clone(),
                                );
                            if instance.profselection != self.prev_profile_selections.get(i).copied().unwrap_or(usize::MAX) {
                                profile_changes.push((i, instance.profselection));
                            }
                        });

                        // Master profile indicator and toggle
                        if instance.profselection > 0 && instance.profselection < self.profiles.len() {
                            let prof_name = &self.profiles[instance.profselection];
                            let is_named = !prof_name.starts_with('.') && prof_name != "Guest";
                            let is_master = self.options.master_profile.as_ref() == Some(prof_name);

                            if is_master {
                                ui.label(RichText::new("👑").size(16.0))
                                    .on_hover_text("Master profile - saves sync to/from original location");
                            } else if is_named && !card_mode.is_narrow() {
                                let set_master_focused = is_element_focused(&current_focus, i, InstanceCardFocus::SetMaster);
                                let btn_text = if card_mode == LayoutMode::Medium { "Master" } else { "Set Master" };
                                let btn = egui::Button::new(btn_text)
                                    .stroke(element_focus_stroke(&current_focus, i, InstanceCardFocus::SetMaster));
                                if ui.add(btn)
                                    .on_hover_text("Set as master profile (saves sync to/from original location)")
                                    .clicked() || (set_master_focused && activate_focused)
                                {
                                    self.options.master_profile = Some(prof_name.clone());
                                    let _ = save_cfg(&self.options);
                                }
                            }
                        }

                        // In wide mode, include Monitor and Invite on same row
                        if !card_mode.is_narrow() {
                            if self.options.gamescope_sdl_backend {
                                ui.add_space(8.0);
                                ui.label("Monitor:");
                                egui::Frame::NONE
                                    .inner_margin(2.0)
                                    .stroke(element_focus_stroke(&current_focus, i, InstanceCardFocus::Monitor))
                                    .show(ui, |ui| {
                                        egui::ComboBox::from_id_salt(format!("monitors{i}"))
                                            .width(monitor_width)
                                            .show_index(
                                                ui,
                                                &mut instance.monitor,
                                                self.monitors.len(),
                                                |j| self.monitors[j].name(),
                                            );
                                    });
                            }

                            ui.add_space(8.0);
                            if self.instance_add_dev.is_none() {
                                let invite_focused = is_element_focused(&current_focus, i, InstanceCardFocus::InviteDevice);
                                let invite_text = if card_mode == LayoutMode::Medium { " +Dev" } else { " Invite Device" };
                                let invite_btn = egui::Button::image_and_text(
                                    egui::Image::new(egui::include_image!("../../res/BTN_Y.png"))
                                        .fit_to_exact_size(egui::vec2(18.0, 18.0)),
                                    invite_text,
                                )
                                .min_size(egui::vec2(0.0, 26.0))
                                .stroke(element_focus_stroke(&current_focus, i, InstanceCardFocus::InviteDevice));
                                let invitebtn = ui.add(invite_btn);
                                if invitebtn.clicked() || (invite_focused && activate_focused) {
                                    self.instance_add_dev = Some(i);
                                }
                            } else if self.instance_add_dev == Some(i) {
                                ui.label(RichText::new("Waiting...").italics());
                                if ui.add(egui::Button::new("x").min_size(egui::vec2(26.0, 26.0))).clicked() {
                                    self.instance_add_dev = None;
                                }
                            }
                        }
                    });

                    // Row 2: Monitor + Invite (only in narrow mode)
                    if card_mode.is_narrow() {
                        ui.horizontal(|ui| {
                            if self.options.gamescope_sdl_backend {
                                ui.label("Mon:");
                                egui::Frame::NONE
                                    .inner_margin(2.0)
                                    .stroke(element_focus_stroke(&current_focus, i, InstanceCardFocus::Monitor))
                                    .show(ui, |ui| {
                                        egui::ComboBox::from_id_salt(format!("monitors{i}"))
                                            .width(monitor_width)
                                            .show_index(
                                                ui,
                                                &mut instance.monitor,
                                                self.monitors.len(),
                                                |j| self.monitors[j].name(),
                                            );
                                    });
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if self.instance_add_dev.is_none() {
                                    let invite_focused = is_element_focused(&current_focus, i, InstanceCardFocus::InviteDevice);
                                    let invite_btn = egui::Button::new("+")
                                        .min_size(egui::vec2(26.0, 26.0))
                                        .stroke(element_focus_stroke(&current_focus, i, InstanceCardFocus::InviteDevice));
                                    if ui.add(invite_btn).on_hover_text("Invite Device").clicked() || (invite_focused && activate_focused) {
                                        self.instance_add_dev = Some(i);
                                    }
                                } else if self.instance_add_dev == Some(i) {
                                    ui.label(RichText::new("...").italics());
                                    if ui.add(egui::Button::new("x").min_size(egui::vec2(26.0, 26.0))).clicked() {
                                        self.instance_add_dev = None;
                                    }
                                }
                            });
                        });
                    }

                    // Device list
                    let profile_name = if instance.profselection > 0 && instance.profselection < self.profiles.len() {
                        Some(self.profiles[instance.profselection].clone())
                    } else {
                        None
                    };
                    let is_named_profile = profile_name.as_ref().map_or(false, |n| !n.starts_with('.') && n != "Guest");

                    for (dev_idx, &dev) in instance.devices.iter().enumerate() {
                        let device_focused = is_element_focused(&current_focus, i, InstanceCardFocus::Device(dev_idx));

                        let dev_display_name = display_names.get(dev)
                            .map(|s| s.as_str())
                            .unwrap_or_else(|| self.input_devices.get(dev).map(|d| d.fancyname()).unwrap_or("Unknown"));
                        let mut dev_text = RichText::new(format!(
                            "   {} {}",
                            self.input_devices[dev].emoji(),
                            dev_display_name
                        ));

                        if self.input_devices[dev].has_button_held() {
                            dev_text = dev_text.strong();
                        }
                        if device_focused {
                            dev_text = dev_text.color(theme::colors::ACCENT);
                        }

                        ui.horizontal(|ui| {
                            ui.label(dev_text);

                            // "Set as preferred" button for named profiles
                            if is_named_profile {
                                let dev_uniq = self.input_devices[dev].uniq();
                                if !dev_uniq.is_empty() {
                                    if let Some(ref prof_name) = profile_name {
                                        let prefs = ProfilePreferences::load(prof_name);
                                        let is_preferred = prefs.preferred_controller.as_ref() == Some(&dev_uniq.to_string());

                                        if is_preferred {
                                            ui.label(RichText::new("★").color(egui::Color32::GOLD))
                                                .on_hover_text("This is the preferred controller for this profile");
                                        } else {
                                            let pref_btn_text = if card_mode.is_narrow() { "☆" } else { "☆ Set Preferred" };
                                            let mut set_pref_btn = egui::Button::new(pref_btn_text).min_size(egui::vec2(24.0, 24.0));
                                            if device_focused {
                                                set_pref_btn = set_pref_btn.stroke(theme::focus_stroke());
                                            }
                                            if ui.add(set_pref_btn)
                                                .on_hover_text(format!("Set as {}'s preferred controller", prof_name))
                                                .clicked() || (device_focused && activate_focused)
                                            {
                                                let mut prefs = ProfilePreferences::load(prof_name);
                                                prefs.set_controller(dev_uniq, self.input_devices[dev].fancyname());
                                                if let Err(e) = prefs.save(prof_name) {
                                                    eprintln!("[splitux] Failed to save profile preferences: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            let remove_text = if card_mode.is_narrow() { "×" } else { "Remove" };
                            let mut remove_btn = egui::Button::new(remove_text).min_size(egui::vec2(24.0, 24.0));
                            if device_focused {
                                remove_btn = remove_btn.stroke(theme::focus_stroke());
                            }
                            if ui.add(remove_btn).on_hover_text("Remove device").clicked() {
                                devices_to_remove.push((i, dev));
                            }
                        });
                    }

                    // Audio section
                    if !self.audio_devices.is_empty() && self.options.audio.enabled {
                        ui.add_space(4.0);
                        let has_conflict = audio_conflicts.contains(&i);
                        let effective = effective_audio.get(i).cloned().flatten();
                        let audio_combo_width = combo_width(ui, 80.0, 50.0);

                        ui.horizontal(|ui| {
                            if has_conflict {
                                ui.label(RichText::new("⚠").size(14.0).color(egui::Color32::YELLOW))
                                    .on_hover_text("Audio conflict: multiple players using same device");
                            }

                            ui.label("🔊");

                            if !card_mode.is_narrow() {
                                match &effective {
                                    Some((_, name, is_override)) => {
                                        let text = if *is_override {
                                            RichText::new(format!("{} (override)", name)).color(egui::Color32::from_rgb(150, 200, 255))
                                        } else {
                                            RichText::new(name).color(theme::colors::TEXT_MUTED)
                                        };
                                        ui.label(text);
                                    }
                                    None => {
                                        ui.label(RichText::new("System default").color(theme::colors::TEXT_MUTED).italics());
                                    }
                                }
                            }

                            // Session override dropdown
                            let override_text = if card_mode.is_narrow() {
                                "▼"
                            } else if self.audio_session_overrides.contains_key(&i) {
                                "Override ▼"
                            } else {
                                "Change ▼"
                            };

                            egui::Frame::NONE
                                .inner_margin(2.0)
                                .stroke(element_focus_stroke(&current_focus, i, InstanceCardFocus::AudioOverride))
                                .show(ui, |ui| {
                                egui::ComboBox::from_id_salt(format!("audio_override_{i}"))
                                    .selected_text(override_text)
                                    .width(audio_combo_width)
                                    .show_ui(ui, |ui| {
                                        for sink in &self.audio_devices {
                                            let is_current = effective.as_ref().map_or(false, |(s, _, _)| s == &sink.name);
                                            if ui.selectable_label(is_current, &sink.description).clicked() {
                                                self.audio_session_overrides.insert(i, Some(sink.name.clone()));
                                            }
                                        }
                                        ui.separator();
                                        let is_muted = effective.as_ref().map_or(false, |(s, _, _)| s.is_empty());
                                        if ui.selectable_label(is_muted, "🔇 None (mute)").clicked() {
                                            self.audio_session_overrides.insert(i, None);
                                        }
                                        if self.audio_session_overrides.contains_key(&i) {
                                            if ui.selectable_label(false, "↩ Reset to profile").clicked() {
                                                self.audio_session_overrides.remove(&i);
                                            }
                                        }
                                    });
                            });

                            // Profile preference management for named profiles
                            if is_named_profile {
                                if let Some(ref prof_name) = profile_name {
                                    let prefs = ProfilePreferences::load(prof_name);
                                    let pref_text = if card_mode.is_narrow() { "★" } else { "Pref..." };
                                    let pref_width = combo_width(ui, 60.0, 35.0);

                                    egui::Frame::NONE
                                        .inner_margin(2.0)
                                        .stroke(element_focus_stroke(&current_focus, i, InstanceCardFocus::AudioPreference))
                                        .show(ui, |ui| {
                                        egui::ComboBox::from_id_salt(format!("audio_pref_{i}"))
                                            .selected_text(pref_text)
                                            .width(pref_width)
                                            .show_ui(ui, |ui| {
                                                ui.label(RichText::new("Set profile preference:").small());
                                                for sink in &self.audio_devices {
                                                    if ui.selectable_label(false, &sink.description).clicked() {
                                                        let mut new_prefs = ProfilePreferences::load(prof_name);
                                                        new_prefs.set_audio(&sink.name, &sink.description);
                                                        if let Err(e) = new_prefs.save(prof_name) {
                                                            eprintln!("[splitux] Failed to save audio preference: {}", e);
                                                        } else {
                                                            self.profile_audio_prefs.insert(i, sink.name.clone());
                                                            self.audio_session_overrides.remove(&i);
                                                        }
                                                    }
                                                }
                                                ui.separator();
                                                if prefs.has_audio() {
                                                    if ui.selectable_label(false, "Clear preference").clicked() {
                                                        let mut new_prefs = ProfilePreferences::load(prof_name);
                                                        new_prefs.clear_audio();
                                                        if let Err(e) = new_prefs.save(prof_name) {
                                                            eprintln!("[splitux] Failed to clear audio preference: {}", e);
                                                        } else {
                                                            self.profile_audio_prefs.remove(&i);
                                                        }
                                                    }
                                                }
                                            });
                                    });
                                }
                            }
                        });
                    }
                });
            ui.add_space(4.0);
        }

        // Process device removals
        for (i, d) in devices_to_remove {
            self.remove_device_instance(i, d);
        }

        // Handle profile changes
        self.handle_profile_changes(profile_changes);

        // Launch options bar (extracted)
        self.display_launch_options(ui);
    }

    /// Display controller and audio warnings
    fn display_instance_warnings(&self, ui: &mut Ui) {
        if !self.controller_warnings.is_empty() {
            theme::card_frame()
                .fill(egui::Color32::from_rgb(80, 60, 20))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("⚠").size(16.0));
                        ui.label(RichText::new("Missing preferred controllers:").strong());
                    });
                    for warning in &self.controller_warnings {
                        ui.label(format!("  • {}", warning));
                    }
                });
            ui.add_space(4.0);
        }

        if !self.audio_warnings.is_empty() {
            theme::card_frame()
                .fill(egui::Color32::from_rgb(80, 60, 20))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔇").size(16.0));
                        ui.label(RichText::new("Missing preferred audio devices:").strong());
                    });
                    for warning in &self.audio_warnings {
                        ui.label(format!("  • {}", warning));
                    }
                });
            ui.add_space(4.0);
        }
    }

    /// Handle profile selection changes - auto-assign preferred controllers and audio
    fn handle_profile_changes(&mut self, profile_changes: Vec<(usize, usize)>) {
        self.controller_warnings.clear();
        self.audio_warnings.clear();

        for (instance_idx, new_selection) in profile_changes {
            if instance_idx < self.prev_profile_selections.len() {
                self.prev_profile_selections[instance_idx] = new_selection;
            }

            if new_selection == 0 || new_selection >= self.profiles.len() {
                continue;
            }

            let profile_name = &self.profiles[new_selection];
            if profile_name.starts_with('.') {
                continue;
            }

            let prefs = ProfilePreferences::load(profile_name);

            // Try to auto-assign preferred controller
            if let Some(ref preferred_uniq) = prefs.preferred_controller {
                if let Some(dev_idx) = find_device_by_uniq(&self.input_devices, preferred_uniq) {
                    if !is_device_assigned(dev_idx, &self.instances) {
                        if instance_idx < self.instances.len() {
                            if !self.instances[instance_idx].devices.contains(&dev_idx) {
                                self.instances[instance_idx].devices.push(dev_idx);
                                println!(
                                    "[splitux] Auto-assigned {} to profile '{}'",
                                    self.device_display_name(dev_idx),
                                    profile_name
                                );
                            }
                        }
                    } else {
                        self.controller_warnings.push(format!(
                            "{}'s controller ({}) is assigned to another player",
                            profile_name,
                            prefs.preferred_controller_name.as_deref().unwrap_or("unknown")
                        ));
                    }
                } else {
                    self.controller_warnings.push(format!(
                        "{}'s preferred controller ({}) is not connected",
                        profile_name,
                        prefs.preferred_controller_name.as_deref().unwrap_or("unknown")
                    ));
                }
            }

            // Store audio preference for use at launch
            if let Some(ref preferred_audio) = prefs.preferred_audio {
                self.profile_audio_prefs.insert(instance_idx, preferred_audio.clone());

                let sink_available = self
                    .audio_devices
                    .iter()
                    .any(|d| d.name == *preferred_audio);

                if !sink_available {
                    self.audio_warnings.push(format!(
                        "{}'s preferred audio ({}) is not available",
                        profile_name,
                        prefs.preferred_audio_name.as_deref().unwrap_or("unknown")
                    ));
                }
            }
        }
    }
}
