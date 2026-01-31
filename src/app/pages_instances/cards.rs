//! Main instance cards display - the core of the instance setup page

use super::focus::{element_focus_stroke, is_element_focused};
use super::types::{AudioOverrideAction, AudioPrefAction, GptokeybAction};
use crate::app::app::{ActiveDropdown, InstanceFocus, Splitux};
use crate::config::save_cfg;
use crate::ui::theme;
use crate::gptokeyb::{list_builtin_profiles, list_user_profiles};
use crate::profile_prefs::ProfilePreferences;
use crate::ui::components::dropdown::{render_gamepad_dropdown, DropdownItem};
use crate::ui::focus::types::InstanceCardFocus;
use crate::ui::responsive::{combo_width, LayoutMode};
use eframe::egui::{self, RichText, Ui};
use egui_phosphor::fill as icons_fill;
use egui_phosphor::regular as icons;

/// Player colors for visual distinction
const PLAYER_COLORS: [egui::Color32; 4] = [
    egui::Color32::from_rgb(80, 180, 255),  // P1: Blue
    egui::Color32::from_rgb(255, 100, 100), // P2: Red
    egui::Color32::from_rgb(100, 220, 100), // P3: Green
    egui::Color32::from_rgb(255, 200, 80),  // P4: Yellow
];

/// Render a monitor dropdown and return the selected index (if any) and whether it was toggled.
fn monitor_dropdown(
    ui: &mut Ui,
    id_salt: &str,
    monitors: &[crate::monitor::Monitor],
    current_idx: usize,
    width: f32,
    is_focused: bool,
    is_open: bool,
    selection_idx: usize,
    activate: bool,
) -> (Option<usize>, bool) {
    let items: Vec<DropdownItem<usize>> = monitors
        .iter()
        .enumerate()
        .map(|(idx, mon)| DropdownItem::new(idx, &mon.display_name(), idx == current_idx))
        .collect();

    let current_name = monitors
        .get(current_idx)
        .map(|m| m.display_name())
        .unwrap_or_else(|| "Select".to_string());

    let resp = render_gamepad_dropdown(
        ui, id_salt, &current_name, width, &items,
        is_focused, is_open, selection_idx, activate,
    );

    (resp.selected, resp.toggled || (is_focused && activate))
}

impl Splitux {
    pub fn display_page_instances(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.heading("Instance Setup");
        ui.add_space(4.0);
        ui.label("Connect your controllers and assign them to player instances");
        ui.add_space(8.0);
        ui.separator();

        self.display_instance_help_bar(ui);
        self.display_instance_warnings(ui);

        // Ensure prev_profile_selections matches instances count
        while self.prev_profile_selections.len() < self.instances.len() {
            self.prev_profile_selections.push(usize::MAX);
        }
        self.prev_profile_selections.truncate(self.instances.len());

        let mut devices_to_remove: Vec<(usize, usize)> = Vec::new();
        let mut profile_changes: Vec<(usize, usize)> = Vec::new();

        // Pre-compute state before mutable iteration
        let audio_conflicts = self.detect_audio_conflicts();
        let effective_audio: Vec<Option<(String, String, bool)>> = (0..self.instances.len())
            .map(|i| self.get_effective_audio(i))
            .collect();

        if self.instances.is_empty() {
            ui.add_space(16.0);
            ui.label(RichText::new("No instances yet").italics());
            ui.add_space(4.0);
            ui.label("Press A or Right-click on a controller to create a player instance");
        }

        let current_focus = self.instance_focus.clone();
        let activate_focused = self.activate_focused;
        let display_names = self.device_display_names.clone();

        // ── Render instance cards ──────────────────────────────────────────
        for (i, instance) in &mut self.instances.iter_mut().enumerate() {
            let player_color = PLAYER_COLORS.get(i).copied().unwrap_or(theme::colors::ACCENT);
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
                    // ── Row 1: Player + Profile + Master + (wide: Monitor + Invite) ──
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("P{}", i + 1)).strong().size(18.0).color(player_color));
                        ui.add_space(8.0);

                        // Profile dropdown
                        if !card_mode.is_narrow() {
                            ui.label("Profile:");
                        }
                        let profile_focused = is_element_focused(&current_focus, i, InstanceCardFocus::Profile);
                        let profile_open = self.active_dropdown == Some(ActiveDropdown::InstanceProfile(i));

                        let profile_items: Vec<DropdownItem<usize>> = self.profiles.iter()
                            .enumerate()
                            .map(|(idx, name)| DropdownItem::new(idx, name.clone(), idx == instance.profselection))
                            .collect();

                        let current_profile = self.profiles.get(instance.profselection)
                            .cloned()
                            .unwrap_or_else(|| "Select".to_string());

                        let profile_response = render_gamepad_dropdown(
                            ui,
                            &format!("profile_{i}"),
                            &current_profile,
                            profile_width,
                            &profile_items,
                            profile_focused,
                            profile_open,
                            self.dropdown_selection_idx,
                            profile_focused && activate_focused,
                        );

                        if let Some(new_idx) = profile_response.selected {
                            instance.profselection = new_idx;
                            self.active_dropdown = None;
                        } else if profile_response.toggled || (profile_focused && activate_focused) {
                            if profile_open {
                                self.active_dropdown = None;
                            } else {
                                self.active_dropdown = Some(ActiveDropdown::InstanceProfile(i));
                                self.dropdown_selection_idx = instance.profselection;
                            }
                        }

                        if instance.profselection != self.prev_profile_selections.get(i).copied().unwrap_or(usize::MAX) {
                            profile_changes.push((i, instance.profselection));
                        }

                        // Master profile indicator
                        if instance.profselection > 0 && instance.profselection < self.profiles.len() {
                            let prof_name = &self.profiles[instance.profselection];
                            let is_named = !prof_name.starts_with('.') && prof_name != "Guest";
                            let is_master = self.options.master_profile.as_ref() == Some(prof_name);

                            if is_master {
                                ui.label(RichText::new(icons_fill::CROWN).size(16.0).color(egui::Color32::GOLD))
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

                        // Wide mode: monitor + invite on same row
                        if !card_mode.is_narrow() {
                            if self.options.gamescope_sdl_backend {
                                ui.add_space(8.0);
                                ui.label("Monitor:");
                                let monitor_focused = is_element_focused(&current_focus, i, InstanceCardFocus::Monitor);
                                let monitor_open = self.active_dropdown == Some(ActiveDropdown::InstanceMonitor(i));

                                let (selected, toggled) = monitor_dropdown(
                                    ui, &format!("monitor_{i}"), &self.monitors,
                                    instance.monitor, monitor_width, monitor_focused, monitor_open,
                                    self.dropdown_selection_idx, monitor_focused && activate_focused,
                                );

                                if let Some(new_idx) = selected {
                                    instance.monitor = new_idx;
                                    self.active_dropdown = None;
                                } else if toggled {
                                    if monitor_open {
                                        self.active_dropdown = None;
                                    } else {
                                        self.active_dropdown = Some(ActiveDropdown::InstanceMonitor(i));
                                        self.dropdown_selection_idx = instance.monitor;
                                    }
                                }
                            }

                            ui.add_space(8.0);
                            if self.instance_add_dev.is_none() {
                                let invite_focused = is_element_focused(&current_focus, i, InstanceCardFocus::InviteDevice);
                                let invite_text = if card_mode == LayoutMode::Medium { " +Dev" } else { " Invite Device" };
                                let invite_btn = egui::Button::image_and_text(
                                    egui::Image::new(egui::include_image!("../../../assets/BTN_Y.png"))
                                        .fit_to_exact_size(egui::vec2(18.0, 18.0)),
                                    invite_text,
                                )
                                .min_size(egui::vec2(0.0, 26.0))
                                .stroke(element_focus_stroke(&current_focus, i, InstanceCardFocus::InviteDevice));
                                if ui.add(invite_btn).clicked() || (invite_focused && activate_focused) {
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

                    // ── Row 2: Monitor + Invite (narrow mode only) ──
                    if card_mode.is_narrow() {
                        ui.horizontal(|ui| {
                            if self.options.gamescope_sdl_backend {
                                ui.label("Mon:");
                                let monitor_focused = is_element_focused(&current_focus, i, InstanceCardFocus::Monitor);
                                let monitor_open = self.active_dropdown == Some(ActiveDropdown::InstanceMonitor(i));

                                let (selected, toggled) = monitor_dropdown(
                                    ui, &format!("monitor_narrow_{i}"), &self.monitors,
                                    instance.monitor, monitor_width, monitor_focused, monitor_open,
                                    self.dropdown_selection_idx, monitor_focused && activate_focused,
                                );

                                if let Some(new_idx) = selected {
                                    instance.monitor = new_idx;
                                    self.active_dropdown = None;
                                } else if toggled {
                                    if monitor_open {
                                        self.active_dropdown = None;
                                    } else {
                                        self.active_dropdown = Some(ActiveDropdown::InstanceMonitor(i));
                                        self.dropdown_selection_idx = instance.monitor;
                                    }
                                }
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

                    // ── Device list ──
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

                            // Preferred controller star for named profiles
                            if is_named_profile {
                                let dev_uniq = self.input_devices[dev].uniq();
                                if !dev_uniq.is_empty() {
                                    if let Some(ref prof_name) = profile_name {
                                        let prefs = ProfilePreferences::load(prof_name);
                                        let is_preferred = prefs.preferred_controller.as_ref() == Some(&dev_uniq.to_string());

                                        if is_preferred {
                                            ui.label(RichText::new(icons_fill::STAR).color(egui::Color32::GOLD))
                                                .on_hover_text("This is the preferred controller for this profile");
                                        } else {
                                            let mut set_pref_btn = egui::Button::new(icons::STAR).min_size(egui::vec2(24.0, 24.0));
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

                            let remove_text = if card_mode.is_narrow() { icons::X } else { "Remove" };
                            let mut remove_btn = egui::Button::new(remove_text).min_size(egui::vec2(24.0, 24.0));
                            if device_focused {
                                remove_btn = remove_btn.stroke(theme::focus_stroke());
                            }
                            if ui.add(remove_btn).on_hover_text("Remove device").clicked() {
                                devices_to_remove.push((i, dev));
                            }
                        });
                    }

                    // ── Audio section ──
                    if !self.audio_devices.is_empty() && self.options.audio.enabled {
                        ui.add_space(4.0);
                        let has_conflict = audio_conflicts.contains(&i);
                        let effective = effective_audio.get(i).cloned().flatten();
                        let audio_combo_width = combo_width(ui, 80.0, 50.0);

                        ui.horizontal(|ui| {
                            if has_conflict {
                                ui.label(RichText::new(icons::WARNING).size(14.0).color(egui::Color32::YELLOW))
                                    .on_hover_text("Audio conflict: multiple players using same device");
                            }

                            ui.label(icons::SPEAKER_HIGH);

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
                            let audio_override_focused = is_element_focused(&current_focus, i, InstanceCardFocus::AudioOverride);
                            let audio_override_open = self.active_dropdown == Some(ActiveDropdown::InstanceAudioOverride(i));
                            let has_override = self.audio_session_overrides.contains_key(&i);

                            let is_muted = effective.as_ref().map_or(false, |(s, _, _)| s.is_empty());
                            let mut items: Vec<DropdownItem<AudioOverrideAction>> = self.audio_devices.iter()
                                .map(|sink| {
                                    let is_current = effective.as_ref().map_or(false, |(s, _, _)| s == &sink.name);
                                    DropdownItem::new(AudioOverrideAction::SetDevice(sink.name.clone()), &sink.description, is_current)
                                })
                                .collect();
                            items.push(DropdownItem::new(AudioOverrideAction::Mute, &format!("{} None (mute)", icons::SPEAKER_SLASH), is_muted));
                            if has_override {
                                items.push(DropdownItem::new(AudioOverrideAction::Reset, "↩ Reset to profile", false));
                            }

                            let button_text = if card_mode.is_narrow() {
                                ""
                            } else if has_override {
                                "Override"
                            } else {
                                "Change"
                            };

                            let audio_response = render_gamepad_dropdown(
                                ui, &format!("audio_override_{i}"), button_text, audio_combo_width,
                                &items, audio_override_focused, audio_override_open,
                                self.dropdown_selection_idx, audio_override_focused && activate_focused,
                            );

                            if let Some(action) = audio_response.selected {
                                match action {
                                    AudioOverrideAction::SetDevice(name) => { self.audio_session_overrides.insert(i, Some(name)); }
                                    AudioOverrideAction::Mute => { self.audio_session_overrides.insert(i, None); }
                                    AudioOverrideAction::Reset => { self.audio_session_overrides.remove(&i); }
                                }
                                self.active_dropdown = None;
                            } else if audio_response.toggled || (audio_override_focused && activate_focused) {
                                if audio_override_open {
                                    self.active_dropdown = None;
                                } else {
                                    self.active_dropdown = Some(ActiveDropdown::InstanceAudioOverride(i));
                                    self.dropdown_selection_idx = 0;
                                }
                            }

                            // Audio preference for named profiles
                            if is_named_profile {
                                if let Some(ref prof_name) = profile_name {
                                    let prefs = ProfilePreferences::load(prof_name);
                                    let pref_text = if card_mode.is_narrow() { icons_fill::STAR } else { "Pref..." };
                                    let pref_width = combo_width(ui, 60.0, 35.0);

                                    let audio_pref_focused = is_element_focused(&current_focus, i, InstanceCardFocus::AudioPreference);
                                    let audio_pref_open = self.active_dropdown == Some(ActiveDropdown::InstanceAudioPreference(i));

                                    let mut items: Vec<DropdownItem<AudioPrefAction>> = self.audio_devices.iter()
                                        .map(|sink| DropdownItem::new(
                                            AudioPrefAction::SetDevice(sink.name.clone(), sink.description.clone()),
                                            &sink.description, false,
                                        ))
                                        .collect();
                                    if prefs.has_audio() {
                                        items.push(DropdownItem::new(AudioPrefAction::Clear, "Clear preference", false));
                                    }

                                    let prof_name_owned = prof_name.clone();

                                    let pref_response = render_gamepad_dropdown(
                                        ui, &format!("audio_pref_{i}"), pref_text, pref_width,
                                        &items, audio_pref_focused, audio_pref_open,
                                        self.dropdown_selection_idx, audio_pref_focused && activate_focused,
                                    );

                                    if let Some(action) = pref_response.selected {
                                        match action {
                                            AudioPrefAction::SetDevice(name, desc) => {
                                                let mut new_prefs = ProfilePreferences::load(&prof_name_owned);
                                                new_prefs.set_audio(&name, &desc);
                                                if let Err(e) = new_prefs.save(&prof_name_owned) {
                                                    eprintln!("[splitux] Failed to save audio preference: {}", e);
                                                } else {
                                                    self.profile_audio_prefs.insert(i, name);
                                                    self.audio_session_overrides.remove(&i);
                                                }
                                            }
                                            AudioPrefAction::Clear => {
                                                let mut new_prefs = ProfilePreferences::load(&prof_name_owned);
                                                new_prefs.clear_audio();
                                                if let Err(e) = new_prefs.save(&prof_name_owned) {
                                                    eprintln!("[splitux] Failed to clear audio preference: {}", e);
                                                } else {
                                                    self.profile_audio_prefs.remove(&i);
                                                }
                                            }
                                        }
                                        self.active_dropdown = None;
                                    } else if pref_response.toggled || (audio_pref_focused && activate_focused) {
                                        if audio_pref_open {
                                            self.active_dropdown = None;
                                        } else {
                                            self.active_dropdown = Some(ActiveDropdown::InstanceAudioPreference(i));
                                            self.dropdown_selection_idx = 0;
                                        }
                                    }
                                }
                            }
                        });
                    }

                    // ── gptokeyb KB/Mouse section ──
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(icons::KEYBOARD);
                        if !card_mode.is_narrow() {
                            ui.label(RichText::new("KB/Mouse:").small());
                        }

                        let gptokeyb_focused = is_element_focused(&current_focus, i, InstanceCardFocus::GptokeybProfile);
                        let gptokeyb_open = self.active_dropdown == Some(ActiveDropdown::InstanceGptokeyb(i));

                        let has_override = self.gptokeyb_instance_overrides.contains_key(&i);
                        let current_gptokeyb = self.gptokeyb_instance_overrides.get(&i);

                        let mut items: Vec<DropdownItem<GptokeybAction>> = Vec::new();
                        items.push(DropdownItem::new(GptokeybAction::Default, "Default (handler)", !has_override));
                        items.push(DropdownItem::new(GptokeybAction::Disabled, format!("{} Disabled", icons::PROHIBIT), current_gptokeyb == Some(&String::new())));

                        for profile in list_builtin_profiles() {
                            items.push(DropdownItem::new(
                                GptokeybAction::Profile(profile.to_string()),
                                format!("{} {} (built-in)", icons::GAME_CONTROLLER, profile),
                                current_gptokeyb == Some(&profile.to_string()),
                            ));
                        }
                        for profile in list_user_profiles() {
                            items.push(DropdownItem::new(
                                GptokeybAction::Profile(profile.clone()),
                                format!("{} {} (custom)", icons::USER, profile),
                                current_gptokeyb == Some(&profile),
                            ));
                        }

                        let button_text = if card_mode.is_narrow() {
                            ""
                        } else if has_override {
                            current_gptokeyb
                                .map(|p| if p.is_empty() { "Disabled" } else { p.as_str() })
                                .unwrap_or("Default")
                        } else {
                            "Default"
                        };

                        let gptokeyb_width = combo_width(ui, 100.0, 50.0);

                        let gptokeyb_response = render_gamepad_dropdown(
                            ui, &format!("gptokeyb_{i}"), button_text, gptokeyb_width,
                            &items, gptokeyb_focused, gptokeyb_open,
                            self.dropdown_selection_idx, gptokeyb_focused && activate_focused,
                        );

                        if let Some(action) = gptokeyb_response.selected {
                            match action {
                                GptokeybAction::Default => { self.gptokeyb_instance_overrides.remove(&i); }
                                GptokeybAction::Disabled => { self.gptokeyb_instance_overrides.insert(i, String::new()); }
                                GptokeybAction::Profile(name) => { self.gptokeyb_instance_overrides.insert(i, name); }
                            }
                            self.active_dropdown = None;
                        } else if gptokeyb_response.toggled || (gptokeyb_focused && activate_focused) {
                            if gptokeyb_open {
                                self.active_dropdown = None;
                            } else {
                                self.active_dropdown = Some(ActiveDropdown::InstanceGptokeyb(i));
                                self.dropdown_selection_idx = 0;
                            }
                        }
                    });
                });
            ui.add_space(4.0);
        }

        // Post-processing
        for (i, d) in devices_to_remove {
            self.remove_device_instance(i, d);
        }
        self.handle_profile_changes(profile_changes);
        self.display_launch_options(ui);
    }

}
