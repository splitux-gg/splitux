// Instance setup page display functions

use super::app::{InstanceFocus, Splitux};
use super::config::save_cfg;
use super::theme;
use crate::input::{find_device_by_uniq, is_device_assigned};
use crate::profile_prefs::ProfilePreferences;
use crate::ui::focus::types::InstanceCardFocus;
use crate::ui::responsive::{combo_width, LayoutMode};
use eframe::egui::{self, RichText, Ui};

impl Splitux {
    /// Check if a specific element in an instance card is focused
    fn is_instance_element_focused(&self, instance_idx: usize, element: InstanceCardFocus) -> bool {
        matches!(&self.instance_focus, InstanceFocus::InstanceCard(i, e) if *i == instance_idx && *e == element)
    }

    /// Check if any element in an instance card is focused
    fn is_instance_card_focused(&self, instance_idx: usize) -> bool {
        matches!(&self.instance_focus, InstanceFocus::InstanceCard(i, _) if *i == instance_idx)
    }

    /// Get focus stroke for an element (accent color if focused)
    fn element_focus_stroke(&self, instance_idx: usize, element: InstanceCardFocus) -> egui::Stroke {
        if self.is_instance_element_focused(instance_idx, element) {
            theme::focus_stroke()
        } else {
            egui::Stroke::NONE
        }
    }

    pub fn display_page_instances(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.heading("Instance Setup");
        ui.add_space(4.0);
        ui.label("Connect your controllers and assign them to player instances");
        ui.add_space(8.0);
        ui.separator();

        // Controls help bar (responsive)
        ui.add_space(8.0);
        let help_mode = LayoutMode::from_ui(ui);
        theme::card_frame()
            .fill(theme::colors::BG_DARK)
            .show(ui, |ui| {
                if help_mode.is_narrow() {
                    // Compact mode: icons only with tooltips, wrapped
                    ui.horizontal_wrapped(|ui| {
                        let add_tip = match self.instance_add_dev {
                            None => "A / Z / Right-Click: Add Instance",
                            Some(i) => &format!("A / Z / Right-Click: Add to P{}", i + 1),
                        };
                        ui.add(egui::Image::new(egui::include_image!("../../res/BTN_A.png")).max_height(16.0))
                            .on_hover_text(add_tip);

                        let remove_tip = match self.instance_add_dev {
                            None => "B / X: Remove",
                            Some(_) => "B / X: Cancel",
                        };
                        ui.add(egui::Image::new(egui::include_image!("../../res/BTN_B.png")).max_height(16.0))
                            .on_hover_text(remove_tip);

                        ui.add(egui::Image::new(egui::include_image!("../../res/BTN_Y.png")).max_height(16.0))
                            .on_hover_text("Y / A: Invite Device");

                        ui.add(egui::Image::new(egui::include_image!("../../res/BTN_DPAD.png")).max_height(16.0))
                            .on_hover_text("D-pad / Left Stick: Navigate");

                        ui.add(egui::Image::new(egui::include_image!("../../res/BTN_STICK_R.png")).max_height(16.0))
                            .on_hover_text("Right Stick: Scroll");
                    });
                } else {
                    // Full mode: icons with labels
                    ui.horizontal(|ui| {
                        // Add instance control
                        ui.add(egui::Image::new(egui::include_image!("../../res/BTN_A.png")).max_height(16.0));
                        ui.label(" / Z / Right-Click:");
                        let add_text = match self.instance_add_dev {
                            None => "Add Instance",
                            Some(i) => &format!("Add to P{}", i + 1),
                        };
                        ui.label(RichText::new(add_text).strong());

                        ui.add_space(16.0);
                        ui.add(egui::Separator::default().vertical());
                        ui.add_space(16.0);

                        // Remove/Cancel control
                        ui.add(egui::Image::new(egui::include_image!("../../res/BTN_B.png")).max_height(16.0));
                        ui.label(" / X:");
                        let remove_text = match self.instance_add_dev {
                            None => "Remove",
                            Some(_) => "Cancel",
                        };
                        ui.label(RichText::new(remove_text).strong());

                        ui.add_space(16.0);
                        ui.add(egui::Separator::default().vertical());
                        ui.add_space(16.0);

                        // Invite control
                        ui.add(egui::Image::new(egui::include_image!("../../res/BTN_Y.png")).max_height(16.0));
                        ui.label(" / A:");
                        ui.label(RichText::new("Invite Device").strong());

                        ui.add_space(16.0);
                        ui.add(egui::Separator::default().vertical());
                        ui.add_space(16.0);

                        // Navigation hints
                        ui.add(egui::Image::new(egui::include_image!("../../res/BTN_DPAD.png")).max_height(16.0));
                        ui.add(egui::Image::new(egui::include_image!("../../res/BTN_STICK_L.png")).max_height(16.0));
                        ui.label(RichText::new("Navigate").strong());

                        ui.add_space(8.0);

                        ui.add(egui::Image::new(egui::include_image!("../../res/BTN_STICK_R.png")).max_height(16.0));
                        ui.label(RichText::new("Scroll").strong());
                    });
                }
            });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Display controller warnings if any
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

        // Ensure prev_profile_selections matches instances count
        while self.prev_profile_selections.len() < self.instances.len() {
            self.prev_profile_selections.push(usize::MAX); // Use MAX as "uninitialized"
        }
        self.prev_profile_selections.truncate(self.instances.len());

        let mut devices_to_remove: Vec<(usize, usize)> = Vec::new();
        let mut profile_changes: Vec<(usize, usize)> = Vec::new(); // (instance_idx, new_profile_selection)

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

        for (i, instance) in &mut self.instances.iter_mut().enumerate() {
            let player_color = player_colors.get(i).copied().unwrap_or(theme::colors::ACCENT);

            // Check if this card is focused
            let card_focused = matches!(&current_focus, InstanceFocus::InstanceCard(idx, _) if *idx == i);

            // Card frame - thicker stroke when card has focus
            let card_stroke = if card_focused {
                egui::Stroke::new(3.0, theme::colors::ACCENT)
            } else {
                egui::Stroke::new(2.0, player_color)
            };

            // Responsive layout mode for this card
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
                        let profile_focused = matches!(&current_focus, InstanceFocus::InstanceCard(idx, InstanceCardFocus::Profile) if *idx == i);
                        let mut profile_frame = egui::Frame::NONE.inner_margin(2.0);
                        if profile_focused {
                            profile_frame = profile_frame.stroke(theme::focus_stroke());
                        }
                        profile_frame.show(ui, |ui| {
                            egui::ComboBox::from_id_salt(format!("{i}"))
                                .width(profile_width)
                                .show_index(
                                    ui,
                                    &mut instance.profselection,
                                    self.profiles.len(),
                                    |j| self.profiles[j].clone(),
                                );
                            // Detect profile selection changes
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
                                // Only show Set Master button in wide/medium mode
                                let set_master_focused = matches!(&current_focus, InstanceFocus::InstanceCard(idx, InstanceCardFocus::SetMaster) if *idx == i);
                                let btn_text = if card_mode == LayoutMode::Medium { "Master" } else { "Set Master" };
                                let mut btn = egui::Button::new(btn_text);
                                if set_master_focused {
                                    btn = btn.stroke(theme::focus_stroke());
                                }
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
                            // Monitor dropdown with focus indicator
                            if self.options.gamescope_sdl_backend {
                                ui.add_space(8.0);
                                ui.label("Monitor:");
                                let monitor_focused = matches!(&current_focus, InstanceFocus::InstanceCard(idx, InstanceCardFocus::Monitor) if *idx == i);
                                let mut monitor_frame = egui::Frame::NONE.inner_margin(2.0);
                                if monitor_focused {
                                    monitor_frame = monitor_frame.stroke(theme::focus_stroke());
                                }
                                monitor_frame.show(ui, |ui| {
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
                            if self.instance_add_dev == None {
                                let invite_focused = matches!(&current_focus, InstanceFocus::InstanceCard(idx, InstanceCardFocus::InviteDevice) if *idx == i);
                                let invite_text = if card_mode == LayoutMode::Medium { " +Dev" } else { " Invite Device" };
                                let mut invite_btn = egui::Button::image_and_text(
                                    egui::Image::new(egui::include_image!("../../res/BTN_Y.png"))
                                        .fit_to_exact_size(egui::vec2(18.0, 18.0)),
                                    invite_text,
                                ).min_size(egui::vec2(0.0, 26.0));
                                if invite_focused {
                                    invite_btn = invite_btn.stroke(theme::focus_stroke());
                                }
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
                            // Monitor dropdown
                            if self.options.gamescope_sdl_backend {
                                ui.label("Mon:");
                                let monitor_focused = matches!(&current_focus, InstanceFocus::InstanceCard(idx, InstanceCardFocus::Monitor) if *idx == i);
                                let mut monitor_frame = egui::Frame::NONE.inner_margin(2.0);
                                if monitor_focused {
                                    monitor_frame = monitor_frame.stroke(theme::focus_stroke());
                                }
                                monitor_frame.show(ui, |ui| {
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
                                if self.instance_add_dev == None {
                                    let invite_focused = matches!(&current_focus, InstanceFocus::InstanceCard(idx, InstanceCardFocus::InviteDevice) if *idx == i);
                                    let mut invite_btn = egui::Button::new("+").min_size(egui::vec2(26.0, 26.0));
                                    if invite_focused {
                                        invite_btn = invite_btn.stroke(theme::focus_stroke());
                                    }
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
                        let device_focused = matches!(&current_focus, InstanceFocus::InstanceCard(idx, InstanceCardFocus::Device(d)) if *idx == i && *d == dev_idx);

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

                    // Audio section - show for all profiles when audio is enabled
                    if !self.audio_devices.is_empty() && self.options.audio.enabled {
                        ui.add_space(4.0);

                        // Use pre-computed conflict and effective audio data
                        let has_conflict = audio_conflicts.contains(&i);
                        let effective = effective_audio.get(i).cloned().flatten();

                        // Responsive audio dropdown widths
                        let audio_combo_width = combo_width(ui, 80.0, 50.0);

                        ui.horizontal(|ui| {
                            // Conflict warning
                            if has_conflict {
                                ui.label(RichText::new("⚠").size(14.0).color(egui::Color32::YELLOW))
                                    .on_hover_text("Audio conflict: multiple players using same device");
                            }

                            ui.label("🔊");

                            // Show effective audio status (truncate in narrow mode)
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

                            // Session override dropdown (available for all profiles)
                            let audio_override_focused = matches!(&current_focus, InstanceFocus::InstanceCard(idx, InstanceCardFocus::AudioOverride) if *idx == i);
                            let override_text = if card_mode.is_narrow() {
                                "▼"
                            } else if self.audio_session_overrides.contains_key(&i) {
                                "Override ▼"
                            } else {
                                "Change ▼"
                            };

                            let mut audio_frame = egui::Frame::NONE.inner_margin(2.0);
                            if audio_override_focused {
                                audio_frame = audio_frame.stroke(theme::focus_stroke());
                            }
                            audio_frame.show(ui, |ui| {
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
                                        // "None" option to mute
                                        let is_muted = effective.as_ref().map_or(false, |(s, _, _)| s.is_empty());
                                        if ui.selectable_label(is_muted, "🔇 None (mute)").clicked() {
                                            self.audio_session_overrides.insert(i, None);
                                        }
                                        // Reset to profile preference
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
                                    let audio_pref_focused = matches!(&current_focus, InstanceFocus::InstanceCard(idx, InstanceCardFocus::AudioPreference) if *idx == i);
                                    let prefs = ProfilePreferences::load(prof_name);
                                    let pref_text = if card_mode.is_narrow() { "★" } else { "Pref..." };
                                    let pref_width = combo_width(ui, 60.0, 35.0);

                                    let mut pref_frame = egui::Frame::NONE.inner_margin(2.0);
                                    if audio_pref_focused {
                                        pref_frame = pref_frame.stroke(theme::focus_stroke());
                                    }
                                    pref_frame.show(ui, |ui| {
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
                                                            // Clear any session override since preference changed
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

        for (i, d) in devices_to_remove {
            self.remove_device_instance(i, d);
        }

        // Handle profile changes - auto-assign preferred controllers
        self.controller_warnings.clear();
        for (instance_idx, new_selection) in profile_changes {
            // Update tracking
            if instance_idx < self.prev_profile_selections.len() {
                self.prev_profile_selections[instance_idx] = new_selection;
            }

            // Skip Guest profiles (selection 0)
            if new_selection == 0 || new_selection >= self.profiles.len() {
                continue;
            }

            let profile_name = &self.profiles[new_selection];
            if profile_name.starts_with('.') {
                continue; // Guest profile
            }

            let prefs = ProfilePreferences::load(profile_name);

            // Try to auto-assign preferred controller
            if let Some(ref preferred_uniq) = prefs.preferred_controller {
                if let Some(dev_idx) = find_device_by_uniq(&self.input_devices, preferred_uniq) {
                    // Check if device is already assigned to another instance
                    if !is_device_assigned(dev_idx, &self.instances) {
                        // Auto-assign the device to this instance
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
                        // Device is assigned elsewhere
                        self.controller_warnings.push(format!(
                            "{}'s controller ({}) is assigned to another player",
                            profile_name,
                            prefs.preferred_controller_name.as_deref().unwrap_or("unknown")
                        ));
                    }
                } else {
                    // Preferred controller not connected
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
            }
        }

        if self.instances.len() > 0 {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                let start_btn = ui.add(
                    egui::Button::image_and_text(
                        egui::Image::new(egui::include_image!("../../res/BTN_START_NEW.png"))
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
                                let split_focused = is_launch_options_focused && self.launch_option_index == option_idx;

                                ui.label("Split:");

                                // Horizontal option
                                let h_selected = !self.options.vertical_two_player;
                                let h_text = if split_focused && h_selected {
                                    RichText::new("▶ Horizontal").strong().color(theme::colors::ACCENT)
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
                                    RichText::new("▶ Vertical").strong().color(theme::colors::ACCENT)
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
                            let kb_focused = is_launch_options_focused && self.launch_option_index == option_idx;
                            let kb_text = if kb_focused {
                                RichText::new("Keyboard/mouse support").color(theme::colors::ACCENT)
                            } else {
                                RichText::new("Keyboard/mouse support")
                            };

                            let checkbox_response = ui.checkbox(
                                &mut self.options.input_holding,
                                kb_text,
                            );

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

    /// Detect audio conflicts - returns set of instance indices that have conflicts
    fn detect_audio_conflicts(&self) -> std::collections::HashSet<usize> {
        use std::collections::{HashMap, HashSet};

        let mut sink_usage: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, instance) in self.instances.iter().enumerate() {
            // Check session override first
            if let Some(override_opt) = self.audio_session_overrides.get(&i) {
                if let Some(sink) = override_opt {
                    sink_usage.entry(sink.clone()).or_default().push(i);
                }
                continue; // Session override takes precedence
            }

            // Check profile_audio_prefs (populated from ProfilePreferences)
            if let Some(sink) = self.profile_audio_prefs.get(&i) {
                sink_usage.entry(sink.clone()).or_default().push(i);
            }
        }

        // Find instances with conflicts (same sink used by multiple instances)
        let mut conflicts = HashSet::new();
        for (_sink, instances) in sink_usage {
            if instances.len() > 1 {
                for idx in instances {
                    conflicts.insert(idx);
                }
            }
        }
        conflicts
    }

    /// Get the effective audio sink for an instance (session override or profile preference)
    fn get_effective_audio(&self, instance_idx: usize) -> Option<(String, String, bool)> {
        // Returns: (sink_name, display_name, is_override)

        // Check session override first
        if let Some(override_opt) = self.audio_session_overrides.get(&instance_idx) {
            return match override_opt {
                Some(sink) => {
                    let name = self.audio_devices.iter()
                        .find(|d| &d.name == sink)
                        .map(|d| d.description.clone())
                        .unwrap_or_else(|| sink.clone());
                    Some((sink.clone(), name, true))
                }
                None => Some(("".to_string(), "Muted".to_string(), true)), // Explicit mute
            };
        }

        // Check profile preference
        if let Some(sink) = self.profile_audio_prefs.get(&instance_idx) {
            let name = self.audio_devices.iter()
                .find(|d| &d.name == sink)
                .map(|d| d.description.clone())
                .unwrap_or_else(|| sink.clone());
            return Some((sink.clone(), name, false));
        }

        None
    }
}
