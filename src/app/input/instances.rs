//! Instance page input handling

use crate::app::app::{ActiveDropdown, InstanceFocus, MenuPage, Splitux};
use crate::input::*;
use crate::ui::focus::types::InstanceCardFocus;
use eframe::egui;

impl Splitux {
    /// Check if an instance dropdown is currently open
    fn is_instance_dropdown_open(&self) -> bool {
        matches!(
            self.active_dropdown,
            Some(ActiveDropdown::InstanceProfile(_))
                | Some(ActiveDropdown::InstanceMonitor(_))
                | Some(ActiveDropdown::InstanceAudioOverride(_))
                | Some(ActiveDropdown::InstanceAudioPreference(_))
        )
    }

    pub(crate) fn handle_devices_instance_menu(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.activate_focused = false;

        let mut i = 0;
        while i < self.input_devices.len() {
            if !self.input_devices[i].enabled() {
                i += 1;
                continue;
            }
            match self.input_devices[i].poll() {
                Some(PadButton::ABtn) | Some(PadButton::ZKey) | Some(PadButton::RightClick) => {
                    // Handle custom layout mode first
                    if self.layout_custom_mode {
                        self.cycle_instance_in_region(self.instances.len());
                        i += 1;
                        continue;
                    }

                    match &self.instance_focus {
                        InstanceFocus::LaunchOptions => {
                            let player_count = self.instances.len();
                            let has_carousel = player_count >= 2;
                            let max_options = if has_carousel { 2 } else { 1 };
                            match self.launch_option_index {
                                0 if has_carousel => {
                                    // A button on carousel cycles to next preset
                                    self.options.layout_presets.cycle_next(player_count);
                                }
                                idx if idx == max_options - 1 => {
                                    self.options.input_holding = !self.options.input_holding;
                                }
                                _ => {}
                            }
                            i += 1;
                            continue;
                        }
                        InstanceFocus::StartButton => {
                            if self.instances.len() > 0 {
                                self.prepare_game_launch();
                            }
                            i += 1;
                            continue;
                        }
                        InstanceFocus::InstanceCard(_, _) => {
                            // Set activate_focused - display code will handle the toggle
                            self.activate_focused = true;
                            i += 1;
                            continue;
                        }
                        InstanceFocus::Devices => {}
                    }

                    // Normal device handling
                    if self.input_devices[i].device_type() != DeviceType::Gamepad
                        && !self.options.input_holding
                    {
                        i += 1;
                        continue;
                    }
                    if !self.options.allow_multiple_instances_on_same_device
                        && self.is_device_in_any_instance(i)
                    {
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
                                self.instance_add_dev = None;
                                self.instances[inst].devices.push(i);
                            } else {
                                i += 1;
                                continue;
                            }
                        }
                        None => {
                            self.instances.push(crate::instance::Instance {
                                devices: vec![i],
                                profname: String::new(),
                                profselection: 0,
                                monitor: 0,
                                width: 0,
                                height: 0,
                            });
                        }
                    }
                }
                Some(PadButton::BBtn) | Some(PadButton::XKey) => {
                    // Handle custom layout mode - B exits
                    if self.layout_custom_mode {
                        self.exit_custom_layout_mode();
                        i += 1;
                        continue;
                    }

                    match &self.instance_focus {
                        InstanceFocus::LaunchOptions | InstanceFocus::StartButton => {
                            if self.instances.len() > 0 {
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
                            if self.instance_add_dev != None {
                                self.instance_add_dev = None;
                            } else if self.is_device_in_any_instance(i) {
                                self.remove_device(i);
                            } else if self.instances.len() < 1 {
                                self.cur_page = MenuPage::Games;
                                self.instance_focus = InstanceFocus::Devices;
                            }
                        }
                    }
                }
                Some(PadButton::YBtn) | Some(PadButton::AKey) => {
                    // Y button enters custom layout mode when on carousel
                    if self.instance_focus == InstanceFocus::LaunchOptions
                        && self.launch_option_index == 0
                        && self.instances.len() >= 2
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

                    if self.instance_add_dev == None {
                        if let Some((instance, _)) = self.find_device_in_instance(i) {
                            self.instance_add_dev = Some(instance);
                        }
                    }
                }
                Some(PadButton::StartBtn) => {
                    if self.instances.len() > 0 && self.is_device_in_any_instance(i) {
                        self.prepare_game_launch();
                    }
                }
                Some(PadButton::Up) => {
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
                        println!("[splitux] Nav Up - focus before: {:?}", self.instance_focus);
                        self.handle_instance_up();
                        println!("[splitux] Nav Up - focus after: {:?}", self.instance_focus);
                    }
                }
                Some(PadButton::Down) => {
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
                        println!("[splitux] Nav Down - focus before: {:?}", self.instance_focus);
                        self.handle_instance_down();
                        println!("[splitux] Nav Down - focus after: {:?}", self.instance_focus);
                    }
                }
                Some(PadButton::Left) => {
                    if self.layout_custom_mode {
                        self.navigate_custom_layout_left();
                        i += 1;
                        continue;
                    }
                    self.handle_instance_left();
                }
                Some(PadButton::Right) => {
                    if self.layout_custom_mode {
                        self.navigate_custom_layout_right();
                        i += 1;
                        continue;
                    }
                    self.handle_instance_right();
                }
                Some(PadButton::LB) => {
                    self.active_dropdown = None;
                    self.cur_page = MenuPage::Settings;
                }
                Some(PadButton::RB) => {
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

    fn handle_instance_up(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions => {
                if self.instances.len() > 0 {
                    self.instance_focus = InstanceFocus::InstanceCard(
                        self.instances.len() - 1,
                        InstanceCardFocus::AudioOverride
                    );
                } else {
                    self.instance_focus = InstanceFocus::Devices;
                }
            }
            InstanceFocus::StartButton => {
                self.instance_focus = InstanceFocus::LaunchOptions;
                self.launch_option_index = 0; // Reset to carousel
            }
            InstanceFocus::InstanceCard(idx, element) => {
                let idx = *idx;
                let new_element = match element {
                    InstanceCardFocus::Profile => {
                        if idx > 0 {
                            self.instance_focus = InstanceFocus::InstanceCard(
                                idx - 1,
                                InstanceCardFocus::AudioOverride
                            );
                            return;
                        } else {
                            self.instance_focus = InstanceFocus::Devices;
                            return;
                        }
                    }
                    InstanceCardFocus::SetMaster => InstanceCardFocus::Profile,
                    InstanceCardFocus::Monitor => InstanceCardFocus::SetMaster,
                    InstanceCardFocus::InviteDevice => {
                        if self.options.gamescope_sdl_backend {
                            InstanceCardFocus::Monitor
                        } else {
                            InstanceCardFocus::SetMaster
                        }
                    }
                    InstanceCardFocus::Device(0) => InstanceCardFocus::InviteDevice,
                    InstanceCardFocus::Device(d) => InstanceCardFocus::Device(d - 1),
                    InstanceCardFocus::AudioOverride => {
                        let dev_count = self.instances.get(idx).map(|inst| inst.devices.len()).unwrap_or(0);
                        if dev_count > 0 {
                            InstanceCardFocus::Device(dev_count - 1)
                        } else {
                            InstanceCardFocus::InviteDevice
                        }
                    }
                    InstanceCardFocus::AudioPreference => InstanceCardFocus::AudioOverride,
                };
                self.instance_focus = InstanceFocus::InstanceCard(idx, new_element);
            }
            InstanceFocus::Devices => {}
        }
    }

    fn handle_instance_down(&mut self) {
        match &self.instance_focus {
            InstanceFocus::Devices => {
                if self.instances.len() > 0 {
                    self.instance_focus = InstanceFocus::InstanceCard(0, InstanceCardFocus::Profile);
                }
            }
            InstanceFocus::LaunchOptions => {
                self.instance_focus = InstanceFocus::StartButton;
            }
            InstanceFocus::StartButton => {}
            InstanceFocus::InstanceCard(idx, element) => {
                let idx = *idx;
                let dev_count = self.instances.get(idx).map(|inst| inst.devices.len()).unwrap_or(0);
                let new_element = match element {
                    InstanceCardFocus::Profile => InstanceCardFocus::SetMaster,
                    InstanceCardFocus::SetMaster => {
                        if self.options.gamescope_sdl_backend {
                            InstanceCardFocus::Monitor
                        } else {
                            InstanceCardFocus::InviteDevice
                        }
                    }
                    InstanceCardFocus::Monitor => InstanceCardFocus::InviteDevice,
                    InstanceCardFocus::InviteDevice => {
                        if dev_count > 0 {
                            InstanceCardFocus::Device(0)
                        } else {
                            InstanceCardFocus::AudioOverride
                        }
                    }
                    InstanceCardFocus::Device(d) => {
                        if *d + 1 < dev_count {
                            InstanceCardFocus::Device(d + 1)
                        } else {
                            InstanceCardFocus::AudioOverride
                        }
                    }
                    InstanceCardFocus::AudioOverride => InstanceCardFocus::AudioPreference,
                    InstanceCardFocus::AudioPreference => {
                        if idx + 1 < self.instances.len() {
                            self.instance_focus = InstanceFocus::InstanceCard(
                                idx + 1,
                                InstanceCardFocus::Profile
                            );
                            return;
                        } else {
                            self.instance_focus = InstanceFocus::LaunchOptions;
                            self.launch_option_index = 0;
                            return;
                        }
                    }
                };
                self.instance_focus = InstanceFocus::InstanceCard(idx, new_element);
            }
        }
    }

    fn handle_instance_left(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions => {
                let player_count = self.instances.len();
                let has_carousel = player_count >= 2;

                // If on carousel (index 0), cycle preset
                if has_carousel && self.launch_option_index == 0 {
                    self.options.layout_presets.cycle_prev(player_count);
                } else if self.launch_option_index > 0 {
                    self.launch_option_index -= 1;
                }
            }
            InstanceFocus::InstanceCard(idx, element) => {
                if *idx > 0 {
                    self.instance_focus = InstanceFocus::InstanceCard(idx - 1, element.clone());
                }
            }
            _ => {}
        }
    }

    fn handle_instance_right(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions => {
                let player_count = self.instances.len();
                let has_carousel = player_count >= 2;
                let max_options = if has_carousel { 2 } else { 1 };

                // If on carousel (index 0), cycle preset
                if has_carousel && self.launch_option_index == 0 {
                    self.options.layout_presets.cycle_next(player_count);
                } else if self.launch_option_index < max_options - 1 {
                    self.launch_option_index += 1;
                }
            }
            InstanceFocus::InstanceCard(idx, element) => {
                if *idx + 1 < self.instances.len() {
                    self.instance_focus = InstanceFocus::InstanceCard(idx + 1, element.clone());
                }
            }
            _ => {}
        }
    }

    /// Process keyboard navigation for instance page
    pub(crate) fn process_instance_nav_key(&mut self, btn: PadButton) {
        match btn {
            PadButton::Up => self.handle_instance_up(),
            PadButton::Down => self.handle_instance_down(),
            PadButton::Left => self.handle_instance_left(),
            PadButton::Right => self.handle_instance_right(),
            _ => {}
        }
    }

    /// Process keyboard activation for instance page
    pub(crate) fn process_instance_activate_key(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions => {
                let player_count = self.instances.len();
                let has_carousel = player_count >= 2;
                let max_options = if has_carousel { 2 } else { 1 };
                match self.launch_option_index {
                    0 if has_carousel => {
                        // Enter/A on carousel cycles to next preset
                        self.options.layout_presets.cycle_next(player_count);
                    }
                    idx if idx == max_options - 1 => {
                        self.options.input_holding = !self.options.input_holding;
                    }
                    _ => {}
                }
            }
            InstanceFocus::StartButton => {
                if self.instances.len() > 0 {
                    self.prepare_game_launch();
                }
            }
            InstanceFocus::InstanceCard(_, _) => {
                self.activate_focused = true;
            }
            InstanceFocus::Devices => {}
        }
    }

    /// Process keyboard back for instance page
    pub(crate) fn process_instance_back_key(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions | InstanceFocus::StartButton => {
                if self.instances.len() > 0 {
                    self.instance_focus = InstanceFocus::InstanceCard(
                        self.instances.len() - 1,
                        InstanceCardFocus::Profile
                    );
                } else {
                    self.instance_focus = InstanceFocus::Devices;
                }
            }
            InstanceFocus::InstanceCard(_, _) => {
                self.instance_focus = InstanceFocus::Devices;
            }
            InstanceFocus::Devices => {}
        }
    }
}
