// Input handling for gamepad/keyboard navigation

use super::app::{FocusPane, InstanceFocus, MenuPage, RegistryFocus, SettingsFocus, Splitux};
use crate::input::*;

use eframe::egui::{self, Key, Vec2};

impl Splitux {
    pub(super) fn handle_gamepad_gui(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // Debug: uncomment to trace input flow
        // println!("[DEBUG] handle_gamepad_gui: page={:?}, handlers={}, pane={:?}",
        //          self.cur_page, self.handlers.len(), self.focus_pane);

        // Reset activation flag at start of each input poll
        self.activate_focused = false;

        let mut key: Option<egui::Key> = None;
        let mut scroll_delta: Option<Vec2> = None;
        let mut page_changed = false;
        let mut start_pressed = false;
        let mut confirm_profile_selection = false;
        let mut open_profile_dropdown = false;
        let mut fetch_registry_needed = false;

        // Cache values needed during loop
        let dropdown_open = self.profile_dropdown_open;
        let dropdown_selection = self.profile_dropdown_selection;
        let profiles_len = self.profiles.len();
        let on_games_page = self.cur_page == MenuPage::Games;
        let on_registry_page = self.cur_page == MenuPage::Registry;
        let on_settings_page = self.cur_page == MenuPage::Settings;
        let on_instances_page = self.cur_page == MenuPage::Instances;
        let has_handlers = !self.handlers.is_empty();
        let registry_needs_fetch = self.registry_index.is_none() && !self.registry_loading;
        let registry_handler_count = self.registry_index.as_ref().map(|r| r.handlers.len()).unwrap_or(0);

        for pad in &mut self.input_devices {
            if !pad.enabled() {
                continue;
            }
            match pad.poll() {
                Some(PadButton::ABtn) => {
                    if dropdown_open {
                        // Check if "New Profile" is selected (last item)
                        if dropdown_selection >= profiles_len {
                            // Create new profile
                            self.profile_dropdown_open = false;
                            self.show_new_profile_dialog = true;
                        } else {
                            // Confirm profile selection
                            confirm_profile_selection = true;
                            self.profile_dropdown_open = false;
                        }
                    } else if on_games_page && has_handlers {
                        // Pane-aware A button behavior
                        match self.focus_pane {
                            FocusPane::GameList => {
                                // A on game list = start game immediately
                                start_pressed = true;
                            }
                            FocusPane::ActionBar | FocusPane::InfoPane => {
                                // Activate focused action button
                                self.activate_focused = true;
                            }
                        }
                    } else if on_registry_page {
                        // Registry page A button behavior
                        match self.registry_focus {
                            RegistryFocus::HandlerList => {
                                // A on handler list = move to install button
                                if self.registry_selected.is_some() {
                                    self.registry_focus = RegistryFocus::InstallButton;
                                }
                            }
                            RegistryFocus::InstallButton => {
                                // A on install button = activate it
                                self.activate_focused = true;
                            }
                        }
                    } else if on_settings_page {
                        // Settings page A button behavior
                        match self.settings_focus {
                            SettingsFocus::Options => {
                                // A in options area = activate focused widget
                                self.activate_focused = true;
                                key = Some(Key::Enter);
                            }
                            SettingsFocus::BottomButtons => {
                                // A on bottom buttons = activate
                                self.activate_focused = true;
                            }
                        }
                    } else {
                        // A button activates focused widget (other pages)
                        self.activate_focused = true;
                        key = Some(Key::Enter);
                    }
                }
                Some(PadButton::BBtn) => {
                    if dropdown_open {
                        // Cancel dropdown without saving
                        self.profile_dropdown_open = false;
                    } else if self.handler_lite.is_some() {
                        self.cur_page = MenuPage::Instances;
                        page_changed = true;
                    } else {
                        self.cur_page = MenuPage::Games;
                        page_changed = true;
                    }
                }
                Some(PadButton::XBtn) => {
                    // Context action - edit selected handler
                    if has_handlers && on_games_page {
                        self.handler_edit = Some(self.handlers[self.selected_handler].clone());
                        self.show_edit_modal = true;
                    }
                }
                Some(PadButton::YBtn) => {
                    // On Games page: toggle profile dropdown
                    // On other pages: go to Settings
                    if on_games_page && has_handlers {
                        if dropdown_open {
                            // Close dropdown and save selection
                            confirm_profile_selection = true;
                            self.profile_dropdown_open = false;
                        } else {
                            // Open dropdown
                            open_profile_dropdown = true;
                        }
                    } else {
                        self.cur_page = MenuPage::Settings;
                        page_changed = true;
                    }
                }
                Some(PadButton::SelectBtn) => key = Some(Key::Tab),
                Some(PadButton::StartBtn) => {
                    start_pressed = true;
                }
                // D-pad navigation - pane-based navigation on Games page
                Some(PadButton::Up) => {
                    if dropdown_open {
                        // Navigate dropdown up (with wrap)
                        let total = profiles_len + 1; // +1 for "New Profile"
                        if self.profile_dropdown_selection == 0 {
                            self.profile_dropdown_selection = total - 1;
                        } else {
                            self.profile_dropdown_selection -= 1;
                        }
                    } else if on_games_page && has_handlers {
                        match self.focus_pane {
                            FocusPane::GameList => {
                                if self.game_panel_bottom_focused {
                                    // Navigate up from bottom buttons
                                    if self.game_panel_bottom_index > 0 {
                                        self.game_panel_bottom_index -= 1;
                                    } else {
                                        // Return to game list
                                        self.game_panel_bottom_focused = false;
                                    }
                                } else {
                                    // Navigate game list up
                                    if self.selected_handler > 0 {
                                        self.selected_handler -= 1;
                                    }
                                }
                            }
                            FocusPane::ActionBar => {
                                // No vertical nav in action bar (all in one row)
                            }
                            FocusPane::InfoPane => {
                                // Navigate up in info pane (between interactive elements)
                                if self.info_pane_index > 0 {
                                    self.info_pane_index -= 1;
                                }
                            }
                        }
                    } else if on_registry_page {
                        // Registry page Up navigation
                        match self.registry_focus {
                            RegistryFocus::HandlerList => {
                                // Navigate handler list up
                                if let Some(sel) = self.registry_selected {
                                    if sel > 0 {
                                        self.registry_selected = Some(sel - 1);
                                    }
                                } else if registry_handler_count > 0 {
                                    self.registry_selected = Some(0);
                                }
                            }
                            RegistryFocus::InstallButton => {
                                // No vertical nav on install button
                            }
                        }
                    } else if on_settings_page {
                        // Settings page Up navigation
                        match self.settings_focus {
                            SettingsFocus::Options => {
                                // Navigate up through settings options
                                if self.settings_option_index > 0 {
                                    self.settings_option_index -= 1;
                                }
                            }
                            SettingsFocus::BottomButtons => {
                                // Move back to options area (last option)
                                self.settings_focus = SettingsFocus::Options;
                            }
                        }
                    } else if !on_instances_page {
                        key = Some(Key::ArrowUp);
                    }
                }
                Some(PadButton::Down) => {
                    if dropdown_open {
                        // Navigate dropdown down (with wrap)
                        let total = profiles_len + 1; // +1 for "New Profile"
                        self.profile_dropdown_selection = (self.profile_dropdown_selection + 1) % total;
                    } else if on_games_page && has_handlers {
                        match self.focus_pane {
                            FocusPane::GameList => {
                                if self.game_panel_bottom_focused {
                                    // Navigate down within bottom buttons
                                    if self.game_panel_bottom_index < 1 {
                                        self.game_panel_bottom_index += 1;
                                    }
                                } else {
                                    // Navigate game list down
                                    if self.selected_handler < self.handlers.len() - 1 {
                                        self.selected_handler += 1;
                                    } else {
                                        // At last game, move to bottom buttons
                                        self.game_panel_bottom_focused = true;
                                        self.game_panel_bottom_index = 0;
                                    }
                                }
                            }
                            FocusPane::ActionBar => {
                                // No vertical nav in action bar (all in one row)
                            }
                            FocusPane::InfoPane => {
                                // Navigate down in info pane (limit set in pages_games)
                                self.info_pane_index += 1;
                            }
                        }
                    } else if on_registry_page {
                        // Registry page Down navigation
                        match self.registry_focus {
                            RegistryFocus::HandlerList => {
                                // Navigate handler list down
                                if let Some(sel) = self.registry_selected {
                                    if sel + 1 < registry_handler_count {
                                        self.registry_selected = Some(sel + 1);
                                    }
                                } else if registry_handler_count > 0 {
                                    self.registry_selected = Some(0);
                                }
                            }
                            RegistryFocus::InstallButton => {
                                // No vertical nav on install button
                            }
                        }
                    } else if on_settings_page {
                        // Settings page Down navigation (20 total options: 0-19)
                        const SETTINGS_MAX_OPTIONS: usize = 19; // 0-indexed max
                        match self.settings_focus {
                            SettingsFocus::Options => {
                                // Navigate down through settings options
                                if self.settings_option_index < SETTINGS_MAX_OPTIONS {
                                    self.settings_option_index += 1;
                                } else {
                                    // Move to bottom buttons
                                    self.settings_focus = SettingsFocus::BottomButtons;
                                    self.settings_button_index = 0;
                                }
                            }
                            SettingsFocus::BottomButtons => {
                                // Cycle button selection
                                self.settings_button_index = (self.settings_button_index + 1) % 2;
                            }
                        }
                    } else if !on_instances_page {
                        key = Some(Key::ArrowDown);
                    }
                }
                Some(PadButton::Left) => {
                    if dropdown_open {
                        // Do nothing in dropdown
                    } else if on_games_page && has_handlers {
                        match self.focus_pane {
                            FocusPane::GameList => {
                                // Already at leftmost - do nothing
                            }
                            FocusPane::ActionBar => {
                                if self.action_bar_index > 0 {
                                    // Navigate left in action bar
                                    self.action_bar_index -= 1;
                                } else {
                                    // Move back to game list
                                    self.focus_pane = FocusPane::GameList;
                                }
                            }
                            FocusPane::InfoPane => {
                                // Move back to action bar
                                self.focus_pane = FocusPane::ActionBar;
                            }
                        }
                    } else if on_registry_page {
                        // Registry page Left navigation
                        match self.registry_focus {
                            RegistryFocus::HandlerList => {
                                // Already at leftmost - do nothing
                            }
                            RegistryFocus::InstallButton => {
                                // Move back to handler list
                                self.registry_focus = RegistryFocus::HandlerList;
                            }
                        }
                    } else if on_settings_page {
                        // Settings page Left navigation
                        match self.settings_focus {
                            SettingsFocus::Options => {
                                // Left in options can cycle radio buttons (handled in UI)
                                key = Some(Key::ArrowLeft);
                            }
                            SettingsFocus::BottomButtons => {
                                // Navigate between bottom buttons
                                if self.settings_button_index > 0 {
                                    self.settings_button_index -= 1;
                                }
                            }
                        }
                    } else if !on_instances_page {
                        key = Some(Key::ArrowLeft);
                    }
                }
                Some(PadButton::Right) => {
                    if dropdown_open {
                        // Do nothing in dropdown
                    } else if on_games_page && has_handlers {
                        match self.focus_pane {
                            FocusPane::GameList => {
                                // Move from game list/bottom buttons to action bar
                                self.focus_pane = FocusPane::ActionBar;
                                self.action_bar_index = 0; // Start at Play button
                                self.game_panel_bottom_focused = false; // Reset bottom focus
                            }
                            FocusPane::ActionBar => {
                                // Navigate right in action bar (Play -> Profile -> Edit)
                                if self.action_bar_index < 2 {
                                    self.action_bar_index += 1;
                                } else {
                                    // Move to info pane
                                    self.focus_pane = FocusPane::InfoPane;
                                }
                            }
                            FocusPane::InfoPane => {
                                // Already at rightmost - do nothing
                            }
                        }
                    } else if on_registry_page {
                        // Registry page Right navigation
                        match self.registry_focus {
                            RegistryFocus::HandlerList => {
                                // Move to install button if handler selected
                                if self.registry_selected.is_some() {
                                    self.registry_focus = RegistryFocus::InstallButton;
                                }
                            }
                            RegistryFocus::InstallButton => {
                                // Already at rightmost - do nothing
                            }
                        }
                    } else if on_settings_page {
                        // Settings page Right navigation
                        match self.settings_focus {
                            SettingsFocus::Options => {
                                // Right in options can cycle radio buttons (handled in UI)
                                key = Some(Key::ArrowRight);
                            }
                            SettingsFocus::BottomButtons => {
                                // Navigate between bottom buttons
                                if self.settings_button_index < 1 {
                                    self.settings_button_index += 1;
                                }
                            }
                        }
                    } else if !on_instances_page {
                        key = Some(Key::ArrowRight);
                    }
                }
                Some(PadButton::RB) => {
                    // Cycle forward: Games → Registry → Settings → Games
                    match self.cur_page {
                        MenuPage::Games => {
                            self.cur_page = MenuPage::Registry;
                            if registry_needs_fetch {
                                fetch_registry_needed = true;
                            }
                            page_changed = true;
                        }
                        MenuPage::Registry => {
                            self.cur_page = MenuPage::Settings;
                            page_changed = true;
                        }
                        MenuPage::Settings => {
                            self.cur_page = MenuPage::Games;
                            page_changed = true;
                        }
                        MenuPage::Instances => {} // Don't cycle from Instances page
                    }
                }
                Some(PadButton::LB) => {
                    // Cycle backward: Games → Settings → Registry → Games
                    match self.cur_page {
                        MenuPage::Games => {
                            self.cur_page = MenuPage::Settings;
                            page_changed = true;
                        }
                        MenuPage::Registry => {
                            self.cur_page = MenuPage::Games;
                            page_changed = true;
                        }
                        MenuPage::Settings => {
                            self.cur_page = MenuPage::Registry;
                            if registry_needs_fetch {
                                fetch_registry_needed = true;
                            }
                            page_changed = true;
                        }
                        MenuPage::Instances => {} // Don't cycle from Instances page
                    }
                }
                Some(PadButton::LT) | Some(PadButton::RT) => {
                    // Reserved for future use (not used for navigation)
                }
                Some(PadButton::ScrollUp) => {
                    // Store scroll for the info pane scroll area
                    self.info_pane_scroll -= 60.0;
                    scroll_delta = Some(Vec2::new(0.0, 60.0));
                }
                Some(PadButton::ScrollDown) => {
                    // Store scroll for the info pane scroll area
                    self.info_pane_scroll += 60.0;
                    scroll_delta = Some(Vec2::new(0.0, -60.0));
                }
                Some(_) => {}
                None => {}
            }
        }

        if let Some(key) = key {
            raw_input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            });
        }

        if let Some(delta) = scroll_delta {
            // Set pointer position to center of screen so scroll events work
            let screen_rect = ctx.screen_rect();
            let center = screen_rect.center();
            raw_input.events.push(egui::Event::PointerMoved(center));
            raw_input.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta,
                modifiers: egui::Modifiers::default(),
            });
        }

        // Handle deferred profile actions
        if confirm_profile_selection {
            self.set_current_profile(dropdown_selection);
        }
        if open_profile_dropdown {
            self.profile_dropdown_selection = self.get_current_profile();
            self.profile_dropdown_open = true;
        }
        if fetch_registry_needed {
            self.fetch_registry();
        }

        // Handle Start button press after loop to avoid borrow issues
        if start_pressed && self.cur_page == MenuPage::Games && !self.handlers.is_empty() {
            self.start_game_setup();
            page_changed = true;
        }

        // When page changes, reset focus to first widget
        if page_changed {
            // Reset Games page focus
            self.focus_pane = FocusPane::GameList;
            self.action_bar_index = 0;
            self.info_pane_index = 0;
            self.info_pane_scroll = 0.0;
            self.game_panel_bottom_focused = false;
            self.game_panel_bottom_index = 0;
            // Reset Registry page focus
            self.registry_focus = RegistryFocus::HandlerList;
            // Reset Settings page focus
            self.settings_focus = SettingsFocus::Options;
            self.settings_button_index = 0;
            self.settings_option_index = 0;
            // Reset general focus
            self.focus_manager.focus_first();
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
        }
    }

    pub(super) fn handle_devices_instance_menu(&mut self) {
        let mut i = 0;
        while i < self.input_devices.len() {
            if !self.input_devices[i].enabled() {
                i += 1;
                continue;
            }
            match self.input_devices[i].poll() {
                Some(PadButton::ABtn) | Some(PadButton::ZKey) | Some(PadButton::RightClick) => {
                    // Handle A button based on current focus
                    if self.instance_focus == InstanceFocus::LaunchOptions {
                        // Toggle the focused launch option
                        let max_options = if self.instances.len() == 2 { 2 } else { 1 };
                        match self.launch_option_index {
                            0 if self.instances.len() == 2 => {
                                // Toggle split style
                                self.options.vertical_two_player = !self.options.vertical_two_player;
                            }
                            idx if idx == max_options - 1 => {
                                // Toggle KB/mouse support (last option)
                                self.options.input_holding = !self.options.input_holding;
                            }
                            _ => {}
                        }
                        i += 1;
                        continue;
                    }

                    // Normal device handling when focused on devices
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
                    // Prevent same keyboard/mouse device in multiple instances due to current custom gamescope limitations
                    // TODO: Remove this when custom gamescope supports the same keyboard/mouse device for multiple instances
                    if self.input_devices[i].device_type() != DeviceType::Gamepad
                        && self.is_device_in_any_instance(i)
                    {
                        i += 1;
                        continue;
                    }

                    match self.instance_add_dev {
                        Some(inst) => {
                            // Add the device in the instance only if it's not already there
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
                    if self.instance_focus == InstanceFocus::LaunchOptions {
                        // Move focus back to devices
                        self.instance_focus = InstanceFocus::Devices;
                    } else if self.instance_add_dev != None {
                        self.instance_add_dev = None;
                    } else if self.is_device_in_any_instance(i) {
                        self.remove_device(i);
                    } else if self.instances.len() < 1 {
                        self.cur_page = MenuPage::Games;
                        self.instance_focus = InstanceFocus::Devices;
                    }
                }
                Some(PadButton::YBtn) | Some(PadButton::AKey) => {
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
                // D-pad navigation for launch options
                Some(PadButton::Up) => {
                    if self.instance_focus == InstanceFocus::LaunchOptions {
                        // Move focus back to devices area
                        self.instance_focus = InstanceFocus::Devices;
                    }
                }
                Some(PadButton::Down) => {
                    if self.instance_focus == InstanceFocus::Devices && self.instances.len() > 0 {
                        // Move focus to launch options
                        self.instance_focus = InstanceFocus::LaunchOptions;
                        self.launch_option_index = 0;
                    }
                }
                Some(PadButton::Left) => {
                    if self.instance_focus == InstanceFocus::LaunchOptions {
                        if self.launch_option_index > 0 {
                            self.launch_option_index -= 1;
                        }
                    }
                }
                Some(PadButton::Right) => {
                    if self.instance_focus == InstanceFocus::LaunchOptions {
                        let max_options = if self.instances.len() == 2 { 2 } else { 1 };
                        if self.launch_option_index < max_options - 1 {
                            self.launch_option_index += 1;
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}
