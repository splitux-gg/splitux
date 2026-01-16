//! Button action handling (A, B, X, Y, etc.)

use crate::app::app::{FocusPane, RegistryFocus, SettingsFocus, Splitux};
use eframe::egui::Key;

impl Splitux {
    pub(super) fn handle_a_button(
        &mut self,
        dropdown_open: bool,
        dropdown_selection: usize,
        profiles_len: usize,
        on_games_page: bool,
        on_registry_page: bool,
        on_settings_page: bool,
        has_handlers: bool,
        confirm_profile_selection: &mut bool,
        start_pressed: &mut bool,
        key: &mut Option<Key>,
    ) {
        if dropdown_open {
            if dropdown_selection >= profiles_len {
                self.profile_dropdown_open = false;
                self.show_new_profile_dialog = true;
            } else {
                *confirm_profile_selection = true;
                self.profile_dropdown_open = false;
            }
        } else if on_games_page && has_handlers {
            match self.focus_pane {
                FocusPane::GameList => {
                    *start_pressed = true;
                }
                FocusPane::ActionBar | FocusPane::InfoPane => {
                    self.activate_focused = true;
                }
            }
        } else if on_registry_page {
            match self.registry_focus {
                RegistryFocus::HandlerList => {
                    if self.registry_selected.is_some() {
                        self.registry_focus = RegistryFocus::InstallButton;
                    }
                }
                RegistryFocus::InstallButton => {
                    self.activate_focused = true;
                }
            }
        } else if on_settings_page {
            // Check for Profile Builder first
            if self.is_profile_builder_active() {
                if self.handle_profile_builder_a_button() {
                    self.activate_focused = true;
                    return;
                }
            }

            match self.settings_focus {
                SettingsFocus::CategoryList => {
                    // A on category = enter options
                    self.settings_focus = SettingsFocus::Options;
                    self.settings_option_index = 0;
                    self.settings_scroll_to_focus = true;
                }
                SettingsFocus::Options => {
                    self.activate_focused = true;
                    *key = Some(Key::Enter);
                }
                SettingsFocus::BottomButtons => {
                    self.activate_focused = true;
                }
            }
        } else {
            self.activate_focused = true;
            *key = Some(Key::Enter);
        }
    }
}
