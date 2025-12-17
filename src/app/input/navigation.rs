//! D-pad navigation handling for different pages

use crate::app::app::{FocusPane, RegistryFocus, SettingsFocus, Splitux};
use eframe::egui::{self, Key};

impl Splitux {
    pub(super) fn handle_up(
        &mut self,
        dropdown_open: bool,
        profiles_len: usize,
        on_games_page: bool,
        on_registry_page: bool,
        on_settings_page: bool,
        on_instances_page: bool,
        has_handlers: bool,
        registry_handler_count: usize,
        key: &mut Option<Key>,
    ) {
        if dropdown_open {
            let total = profiles_len + 1;
            if self.profile_dropdown_selection == 0 {
                self.profile_dropdown_selection = total - 1;
            } else {
                self.profile_dropdown_selection -= 1;
            }
        } else if on_games_page && has_handlers {
            self.handle_games_up();
        } else if on_registry_page {
            self.handle_registry_up(registry_handler_count);
        } else if on_settings_page {
            self.handle_settings_up();
        } else if !on_instances_page {
            *key = Some(Key::ArrowUp);
        }
    }

    pub(super) fn handle_down(
        &mut self,
        dropdown_open: bool,
        profiles_len: usize,
        on_games_page: bool,
        on_registry_page: bool,
        on_settings_page: bool,
        on_instances_page: bool,
        has_handlers: bool,
        registry_handler_count: usize,
        key: &mut Option<Key>,
    ) {
        if dropdown_open {
            let total = profiles_len + 1;
            self.profile_dropdown_selection = (self.profile_dropdown_selection + 1) % total;
        } else if on_games_page && has_handlers {
            self.handle_games_down();
        } else if on_registry_page {
            self.handle_registry_down(registry_handler_count);
        } else if on_settings_page {
            self.handle_settings_down();
        } else if !on_instances_page {
            *key = Some(Key::ArrowDown);
        }
    }

    pub(super) fn handle_left(
        &mut self,
        dropdown_open: bool,
        on_games_page: bool,
        on_registry_page: bool,
        on_settings_page: bool,
        on_instances_page: bool,
        has_handlers: bool,
        key: &mut Option<Key>,
    ) {
        if dropdown_open {
            // Do nothing
        } else if on_games_page && has_handlers {
            self.handle_games_left();
        } else if on_registry_page {
            self.handle_registry_left();
        } else if on_settings_page {
            self.handle_settings_left(key);
        } else if !on_instances_page {
            *key = Some(Key::ArrowLeft);
        }
    }

    pub(super) fn handle_right(
        &mut self,
        dropdown_open: bool,
        on_games_page: bool,
        on_registry_page: bool,
        on_settings_page: bool,
        on_instances_page: bool,
        has_handlers: bool,
        key: &mut Option<Key>,
    ) {
        if dropdown_open {
            // Do nothing
        } else if on_games_page && has_handlers {
            self.handle_games_right();
        } else if on_registry_page {
            self.handle_registry_right();
        } else if on_settings_page {
            self.handle_settings_right(key);
        } else if !on_instances_page {
            *key = Some(Key::ArrowRight);
        }
    }

    // Games page navigation
    fn handle_games_up(&mut self) {
        match self.focus_pane {
            FocusPane::GameList => {
                if self.game_panel_bottom_focused {
                    if self.game_panel_bottom_index > 0 {
                        self.game_panel_bottom_index -= 1;
                    } else {
                        self.game_panel_bottom_focused = false;
                    }
                } else if self.selected_handler > 0 {
                    self.selected_handler -= 1;
                }
            }
            FocusPane::ActionBar => {}
            FocusPane::InfoPane => {
                if self.info_pane_index > 0 {
                    self.info_pane_index -= 1;
                }
            }
        }
    }

    fn handle_games_down(&mut self) {
        match self.focus_pane {
            FocusPane::GameList => {
                if self.game_panel_bottom_focused {
                    if self.game_panel_bottom_index < 1 {
                        self.game_panel_bottom_index += 1;
                    }
                } else if self.selected_handler < self.handlers.len() - 1 {
                    self.selected_handler += 1;
                } else {
                    self.game_panel_bottom_focused = true;
                    self.game_panel_bottom_index = 0;
                }
            }
            FocusPane::ActionBar => {}
            FocusPane::InfoPane => {
                self.info_pane_index += 1;
            }
        }
    }

    fn handle_games_left(&mut self) {
        match self.focus_pane {
            FocusPane::GameList => {}
            FocusPane::ActionBar => {
                if self.action_bar_index > 0 {
                    self.action_bar_index -= 1;
                } else {
                    // Auto-expand games panel when navigating into it
                    if self.games_panel_collapsed {
                        self.games_panel_collapsed = false;
                    }
                    self.focus_pane = FocusPane::GameList;
                }
            }
            FocusPane::InfoPane => {
                self.focus_pane = FocusPane::ActionBar;
            }
        }
    }

    fn handle_games_right(&mut self) {
        match self.focus_pane {
            FocusPane::GameList => {
                self.focus_pane = FocusPane::ActionBar;
                self.action_bar_index = 0;
                self.game_panel_bottom_focused = false;
            }
            FocusPane::ActionBar => {
                if self.action_bar_index < 2 {
                    self.action_bar_index += 1;
                } else {
                    self.focus_pane = FocusPane::InfoPane;
                }
            }
            FocusPane::InfoPane => {}
        }
    }

    // Registry page navigation
    fn handle_registry_up(&mut self, registry_handler_count: usize) {
        if let RegistryFocus::HandlerList = self.registry_focus {
            if let Some(sel) = self.registry_selected {
                if sel > 0 {
                    self.registry_selected = Some(sel - 1);
                }
            } else if registry_handler_count > 0 {
                self.registry_selected = Some(0);
            }
        }
    }

    fn handle_registry_down(&mut self, registry_handler_count: usize) {
        if let RegistryFocus::HandlerList = self.registry_focus {
            if let Some(sel) = self.registry_selected {
                if sel + 1 < registry_handler_count {
                    self.registry_selected = Some(sel + 1);
                }
            } else if registry_handler_count > 0 {
                self.registry_selected = Some(0);
            }
        }
    }

    fn handle_registry_left(&mut self) {
        if let RegistryFocus::InstallButton = self.registry_focus {
            self.registry_focus = RegistryFocus::HandlerList;
        }
    }

    fn handle_registry_right(&mut self) {
        if let RegistryFocus::HandlerList = self.registry_focus {
            if self.registry_selected.is_some() {
                self.registry_focus = RegistryFocus::InstallButton;
            }
        }
    }

    // Settings page navigation

    /// Get the maximum settings option index (dynamic based on profile count)
    /// Base options: 0-19 (General, Gamescope, Audio)
    /// Profile section: 20 = "New Profile" button, 21+ = profile entries
    pub fn settings_max_option_index(&self) -> usize {
        const BASE_OPTIONS: usize = 19; // indices 0-19
        // +1 for "New Profile" button, +N for profile entries
        BASE_OPTIONS + 1 + self.profiles.len()
    }

    /// Check if the current settings option index is in the profile section
    pub fn is_in_profile_section(&self) -> bool {
        self.settings_option_index >= 20
    }

    fn handle_settings_up(&mut self) {
        // If a profile pref dropdown is open, navigate within it
        if self.profile_ctrl_combo_open.is_some() || self.profile_audio_combo_open.is_some() {
            if self.profile_dropdown_selection_idx > 0 {
                self.profile_dropdown_selection_idx -= 1;
            }
            return;
        }

        match self.settings_focus {
            SettingsFocus::Options => {
                // Check if we're in an expanded profile and need to navigate sub-items
                if self.settings_option_index >= 21 {
                    let profile_idx = self.settings_option_index - 21;
                    if self.profile_prefs_expanded == Some(profile_idx) && self.profile_prefs_focus > 0 {
                        // Move up within expanded profile sub-items
                        self.profile_prefs_focus -= 1;
                        self.settings_scroll_to_focus = true;
                        return;
                    }
                }

                if self.settings_option_index > 0 {
                    // Close any open dropdowns when leaving profile
                    self.profile_ctrl_combo_open = None;
                    self.profile_audio_combo_open = None;

                    self.settings_option_index -= 1;
                    self.settings_scroll_to_focus = true;

                    // If entering an expanded profile from below, start at bottom sub-item
                    if self.settings_option_index >= 21 {
                        let profile_idx = self.settings_option_index - 21;
                        if self.profile_prefs_expanded == Some(profile_idx) {
                            self.profile_prefs_focus = 2; // Start at audio (bottom)
                        } else {
                            self.profile_prefs_focus = 0;
                        }
                    }
                }
            }
            SettingsFocus::BottomButtons => {
                self.settings_focus = SettingsFocus::Options;
                self.settings_scroll_to_focus = true;
                // If on an expanded profile, go to bottom sub-item
                if self.settings_option_index >= 21 {
                    let profile_idx = self.settings_option_index - 21;
                    if self.profile_prefs_expanded == Some(profile_idx) {
                        self.profile_prefs_focus = 2;
                    }
                }
            }
        }
    }

    fn handle_settings_down(&mut self) {
        // If a profile pref dropdown is open, navigate within it
        if self.profile_ctrl_combo_open.is_some() || self.profile_audio_combo_open.is_some() {
            // Calculate max items (None + devices count)
            let max_items = if self.profile_ctrl_combo_open.is_some() {
                1 + self.input_devices.iter().filter(|d| !d.uniq().is_empty()).count()
            } else {
                1 + self.audio_devices.len()
            };
            if self.profile_dropdown_selection_idx < max_items.saturating_sub(1) {
                self.profile_dropdown_selection_idx += 1;
            }
            return;
        }

        let max_options = self.settings_max_option_index();
        match self.settings_focus {
            SettingsFocus::Options => {
                // Check if we're in an expanded profile and need to navigate sub-items
                if self.settings_option_index >= 21 {
                    let profile_idx = self.settings_option_index - 21;
                    if self.profile_prefs_expanded == Some(profile_idx) && self.profile_prefs_focus < 2 {
                        // Move down within expanded profile sub-items
                        self.profile_prefs_focus += 1;
                        self.settings_scroll_to_focus = true;
                        return;
                    }
                }

                // Move to next option
                if self.settings_option_index < max_options {
                    self.settings_option_index += 1;
                    self.settings_scroll_to_focus = true;
                    self.profile_prefs_focus = 0; // Reset to header when moving to new item
                    // Close any open dropdowns when leaving profile
                    self.profile_ctrl_combo_open = None;
                    self.profile_audio_combo_open = None;
                } else {
                    self.settings_focus = SettingsFocus::BottomButtons;
                    self.settings_button_index = 0;
                }
            }
            SettingsFocus::BottomButtons => {
                self.settings_button_index = (self.settings_button_index + 1) % 2;
            }
        }
    }

    fn handle_settings_left(&mut self, key: &mut Option<Key>) {
        match self.settings_focus {
            SettingsFocus::Options => {
                *key = Some(Key::ArrowLeft);
            }
            SettingsFocus::BottomButtons => {
                if self.settings_button_index > 0 {
                    self.settings_button_index -= 1;
                }
            }
        }
    }

    fn handle_settings_right(&mut self, key: &mut Option<Key>) {
        match self.settings_focus {
            SettingsFocus::Options => {
                *key = Some(Key::ArrowRight);
            }
            SettingsFocus::BottomButtons => {
                if self.settings_button_index < 1 {
                    self.settings_button_index += 1;
                }
            }
        }
    }

    pub(super) fn reset_page_focus(&mut self, ctx: &egui::Context) {
        self.focus_pane = FocusPane::GameList;
        self.action_bar_index = 0;
        self.info_pane_index = 0;
        self.info_pane_scroll = 0.0;
        self.game_panel_bottom_focused = false;
        self.game_panel_bottom_index = 0;
        self.registry_focus = RegistryFocus::HandlerList;
        self.settings_focus = SettingsFocus::Options;
        self.settings_button_index = 0;
        self.settings_option_index = 0;
        self.focus_manager.focus_first();
        ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
    }
}
