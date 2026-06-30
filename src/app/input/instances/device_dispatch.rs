//! Device dispatch polling loop for instance page

use crate::app::app::{ActiveDropdown, InstanceFocus, MenuPage, Splitux};
use crate::input::*;
use crate::ui::focus::types::InstanceCardFocus;
use eframe::egui;

impl Splitux {
    /// Keyboard + mouse are ONE logical I/O unit on a single-user machine — there
    /// is no game you'd play with a keyboard but no mouse (or vice versa). So when
    /// a kb/mouse device joins a seat, pull in every other still-unassigned
    /// keyboard/mouse so a single press binds the whole kb+mouse I/O to that
    /// player. This also makes duplicate firmware endpoints harmless: the seat owns
    /// them all, so whichever endpoint the keyboard/mouse actually emits on is
    /// bound regardless. Gamepads are never touched here — each pad is its own
    /// player; this is a no-op when the triggering device is a gamepad.
    pub(crate) fn join_kbm_partners(&mut self, inst: usize, trigger: usize) {
        let trigger_type = self.input_devices[trigger].device_type();
        if trigger_type != DeviceType::Keyboard && trigger_type != DeviceType::Mouse {
            return;
        }
        let debug_input = std::env::var_os("SPLITUX_DEBUG_INPUT").is_some();
        for j in 0..self.input_devices.len() {
            if j == trigger || !self.input_devices[j].enabled() {
                continue;
            }
            let dt = self.input_devices[j].device_type();
            if (dt == DeviceType::Keyboard || dt == DeviceType::Mouse)
                && !self.is_device_in_any_instance(j)
            {
                if debug_input {
                    eprintln!(
                        "[splitux/input]   -> pair kb/mouse partner {j} ({}) into instance {inst}",
                        self.input_devices[j].fancyname()
                    );
                }
                self.instances[inst].devices.push(j);
            }
        }
    }

    pub(crate) fn handle_devices_instance_menu(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.activate_focused = false;

        // Opt-in live tracing for the device-assignment path (run with
        // `SPLITUX_DEBUG_INPUT=1 splitux`). Prints which focus an A-press hit and
        // why a device was/ wasn't added — the fast way to diagnose "A doesn't
        // join my controller" without a unit test for gamepad nav.
        let debug_input = std::env::var_os("SPLITUX_DEBUG_INPUT").is_some();

        // Keyboard navigates this page the SAME as a gamepad: arrows move focus,
        // Enter activates, Escape goes back. egui delivers those key events; the
        // gamepad-page handler runs this, but the Instances page (which has its
        // own evdev poll loop below for pads + device-add keys) otherwise dropped
        // them. The evdev poll only maps letter keys (z/a/x) and pad d-pad, so
        // there's no overlap with the arrow/Enter/Escape handled here.
        let mut kb_key: Option<egui::Key> = None;
        let mut kb_page_changed = false;
        if self.process_keyboard_nav(raw_input, true, false, &mut kb_key, &mut kb_page_changed) {
            raw_input.events.retain(|event| {
                !matches!(event, egui::Event::Key { key: k, pressed: true, .. }
                    if matches!(k,
                        egui::Key::ArrowUp | egui::Key::ArrowDown
                        | egui::Key::ArrowLeft | egui::Key::ArrowRight
                        | egui::Key::Enter | egui::Key::Escape))
            });
        }

        let mut i = 0;
        while i < self.input_devices.len() {
            if !self.input_devices[i].enabled() {
                i += 1;
                continue;
            }
            match self.input_devices[i].poll() {
                PollResult::DeviceDisabled(reason) => {
                    eprintln!("[splitux] evdev: {}", reason);
                    i += 1;
                    continue;
                }
                PollResult::None => {
                    i += 1;
                    continue;
                }
                PollResult::Button(PadButton::ABtn) | PollResult::Button(PadButton::ZKey) | PollResult::Button(PadButton::RightClick) => {
                    // Handle custom layout mode first
                    if self.layout_custom_mode {
                        self.cycle_instance_in_region(self.instances.len());
                        i += 1;
                        continue;
                    }

                    if debug_input {
                        eprintln!(
                            "[splitux/input] A from device {i} ({}): focus={:?} input_holding={} in_any={}",
                            self.input_devices[i].fancyname(),
                            self.instance_focus,
                            self.options.input_holding,
                            self.is_device_in_any_instance(i),
                        );
                    }

                    match &self.instance_focus {
                        InstanceFocus::GamesSidebar => {
                            // A in the sidebar confirms the picked game and returns
                            // to the setup content (selection already changed live).
                            self.instance_focus = InstanceFocus::Devices;
                            i += 1;
                            continue;
                        }
                        InstanceFocus::LaunchOptions => {
                            // The carousel is the only launch option; A cycles it.
                            if self.show_layout_carousel() {
                                self.options.layout_presets.cycle_next(self.instances.len());
                            }
                            i += 1;
                            continue;
                        }
                        InstanceFocus::StartButton => {
                            if !self.instances.is_empty() {
                                self.prepare_game_launch();
                            }
                            i += 1;
                            continue;
                        }
                        InstanceFocus::InstanceCard(inst, _) => {
                            let inst = *inst;
                            // "Press A on a card to JOIN it with your controller":
                            // if the device that pressed A isn't assigned to any
                            // instance yet, add it to THIS instance instead of only
                            // toggling the focused widget. The already-assigned
                            // device you navigate the menu with stays in_any → falls
                            // through to the widget toggle, so dropdowns still work.
                            // This is also what restores joining when focus has moved
                            // onto a card (the device strip / Game-picker focus chain
                            // made that common). A keyboard/mouse joins just like a
                            // gamepad — pressing to join IS the intent; we flip on
                            // input holding so the launch binds it to this player.
                            if inst < self.instances.len()
                                && !self.is_device_in_any_instance(i)
                            {
                                if self.input_devices[i].device_type() != DeviceType::Gamepad {
                                    self.options.input_holding = true;
                                }
                                if debug_input {
                                    eprintln!(
                                        "[splitux/input]   -> joining device {i} to card {inst}"
                                    );
                                }
                                self.instance_add_dev = None;
                                self.instances[inst].devices.push(i);
                                // kb/mouse join as one I/O unit — pull in the partner.
                                self.join_kbm_partners(inst, i);
                                i += 1;
                                continue;
                            }
                            // Set activate_focused - display code will handle the toggle
                            self.activate_focused = true;
                            i += 1;
                            continue;
                        }
                        InstanceFocus::Devices => {}
                    }

                    // Normal device handling (focus is on the device strip). A
                    // keyboard/mouse adds exactly like a gamepad — pressing to join
                    // is the signal that the user wants it. Adding one flips on input
                    // holding so the launch actually binds it to the new player
                    // (that flag is a launch concern, not something to toggle first).
                    if self.input_devices[i].device_type() != DeviceType::Gamepad {
                        self.options.input_holding = true;
                    }
                    if !self.options.allow_multiple_instances_on_same_device
                        && self.is_device_in_any_instance(i)
                    {
                        if debug_input {
                            eprintln!("[splitux/input]   -> skip: device already assigned (allow-multiple off)");
                        }
                        i += 1;
                        continue;
                    }
                    if self.input_devices[i].device_type() != DeviceType::Gamepad
                        && self.is_device_in_any_instance(i)
                    {
                        i += 1;
                        continue;
                    }

                    match self.instance_add_dev {
                        Some(inst) => {
                            if !self.is_device_in_instance(inst, i) {
                                if debug_input {
                                    eprintln!("[splitux/input]   -> add: device {i} -> invited instance {inst}");
                                }
                                self.instance_add_dev = None;
                                self.instances[inst].devices.push(i);
                                self.join_kbm_partners(inst, i);
                            } else {
                                i += 1;
                                continue;
                            }
                        }
                        None => {
                            if debug_input {
                                eprintln!("[splitux/input]   -> add: device {i} -> new instance {}", self.instances.len());
                            }
                            self.instances.push(crate::instance::Instance {
                                devices: vec![i],
                                game: 0,
                                profname: String::new(),
                                profselection: 0,
                                monitor: 0,
                                width: 0,
                                height: 0,
                                together: false,
                                together_input: crate::instance::TogetherInput::Gamepad,
                                together_seats: 0,
                                local_input: false,
                            });
                            // kb/mouse join as one I/O unit — pull the partner into
                            // the just-created seat.
                            let new_inst = self.instances.len() - 1;
                            self.join_kbm_partners(new_inst, i);
                        }
                    }
                }
                PollResult::Button(PadButton::BBtn) | PollResult::Button(PadButton::XKey) => {
                    // Handle custom layout mode - B exits
                    if self.layout_custom_mode {
                        self.exit_custom_layout_mode();
                        i += 1;
                        continue;
                    }

                    match &self.instance_focus {
                        InstanceFocus::GamesSidebar => {
                            // B backs out of the sidebar to the setup content.
                            self.instance_focus = InstanceFocus::Devices;
                        }
                        InstanceFocus::LaunchOptions | InstanceFocus::StartButton => {
                            if !self.instances.is_empty() {
                                self.instance_focus = InstanceFocus::InstanceCard(
                                    self.instances.len() - 1,
                                    InstanceCardFocus::Profile
                                );
                            } else {
                                self.instance_focus = InstanceFocus::Devices;
                            }
                        }
                        InstanceFocus::InstanceCard(_, _) => {
                            if self.is_instance_dropdown_open() {
                                // Inject Escape key to close dropdown
                                raw_input.events.push(egui::Event::Key {
                                    key: egui::Key::Escape,
                                    physical_key: None,
                                    pressed: true,
                                    repeat: false,
                                    modifiers: egui::Modifiers::NONE,
                                });
                                // Clear our dropdown tracking
                                self.active_dropdown = None;
                            } else {
                                self.instance_focus = InstanceFocus::Devices;
                            }
                        }
                        InstanceFocus::Devices => {
                            if self.instance_add_dev.is_some() {
                                self.instance_add_dev = None;
                            } else if self.is_device_in_any_instance(i) {
                                // kb/mouse join as one unit → X removes the whole player.
                                self.remove_player_by_device(i);
                            } else if self.instances.is_empty() {
                                self.cur_page = MenuPage::Games;
                                self.instance_focus = InstanceFocus::Devices;
                            }
                        }
                    }
                }
                PollResult::Button(PadButton::YBtn) | PollResult::Button(PadButton::AKey) => {
                    // Y button enters custom layout mode when the layout carousel
                    // is showing (2+ local seats; not local-coop / Together-only).
                    if self.instance_focus == InstanceFocus::LaunchOptions
                        && self.show_layout_carousel()
                    {
                        let player_count = self.instances.len();
                        let preset_id = self
                            .options
                            .layout_presets
                            .get_for_count(player_count)
                            .to_string();
                        self.enter_custom_layout_mode(player_count, &preset_id);
                        i += 1;
                        continue;
                    }

                    if self.instance_add_dev.is_none()
                        && let Some((instance, _)) = self.find_device_in_instance(i) {
                            self.instance_add_dev = Some(instance);
                        }
                }
                PollResult::Button(PadButton::StartBtn) => {
                    if !self.instances.is_empty() && self.is_device_in_any_instance(i) {
                        self.prepare_game_launch();
                    }
                }
                PollResult::Button(PadButton::Up) => {
                    // Handle custom layout mode navigation
                    if self.layout_custom_mode {
                        self.navigate_custom_layout_up();
                        i += 1;
                        continue;
                    }

                    if let Some(ref dropdown) = self.active_dropdown {
                        // Navigate within dropdown - all use dropdown_selection_idx
                        match dropdown {
                            ActiveDropdown::InstanceProfile(_) |
                            ActiveDropdown::InstanceMonitor(_) |
                            ActiveDropdown::InstanceAudioOverride(_) |
                            ActiveDropdown::InstanceAudioPreference(_) => {
                                if self.dropdown_selection_idx > 0 {
                                    self.dropdown_selection_idx -= 1;
                                }
                            }
                            _ => {}
                        }
                    } else {
                        self.handle_instance_up();
                    }
                }
                PollResult::Button(PadButton::Down) => {
                    // Handle custom layout mode navigation
                    if self.layout_custom_mode {
                        self.navigate_custom_layout_down();
                        i += 1;
                        continue;
                    }

                    if let Some(ref dropdown) = self.active_dropdown {
                        // Navigate within dropdown - all use dropdown_selection_idx
                        let max_items = match dropdown {
                            ActiveDropdown::InstanceProfile(_) => self.profiles.len(),
                            ActiveDropdown::InstanceMonitor(_) => self.monitors.len(),
                            ActiveDropdown::InstanceAudioOverride(_) => self.audio_devices.len() + 2, // devices + mute + reset
                            ActiveDropdown::InstanceAudioPreference(_) => self.audio_devices.len() + 1, // devices + clear
                            _ => 0,
                        };
                        if self.dropdown_selection_idx < max_items.saturating_sub(1) {
                            self.dropdown_selection_idx += 1;
                        }
                    } else {
                        self.handle_instance_down();
                    }
                }
                PollResult::Button(PadButton::Left) => {
                    if self.layout_custom_mode {
                        self.navigate_custom_layout_left();
                        i += 1;
                        continue;
                    }
                    self.handle_instance_left();
                }
                PollResult::Button(PadButton::Right) => {
                    if self.layout_custom_mode {
                        self.navigate_custom_layout_right();
                        i += 1;
                        continue;
                    }
                    self.handle_instance_right();
                }
                PollResult::Button(PadButton::LB) => {
                    self.active_dropdown = None;
                    self.cur_page = MenuPage::Settings;
                }
                PollResult::Button(PadButton::RB) => {
                    self.active_dropdown = None;
                    self.cur_page = MenuPage::Registry;
                    if self.registry_index.is_none() && !self.registry_loading {
                        self.fetch_registry();
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}
