//! Keyboard navigation handling

use crate::app::app::{MenuPage, SettingsFocus, Splitux};
use crate::input::PadButton;
use crate::ui::focus::types::{FocusPane, NavDirection, RegistryFocus};
use eframe::egui::{self, Key};

impl Splitux {
    /// Process keyboard navigation events, returns true if events were consumed.
    ///
    /// Keyboard drives the SAME custom focus system as the gamepad on every page:
    /// arrows move focus, Enter activates, Escape goes back. Games/Registry route
    /// through `handle_direction_input` (the gamepad's own nav), so the two input
    /// methods stay in lockstep.
    pub(super) fn process_keyboard_nav(
        &mut self,
        raw_input: &egui::RawInput,
        on_instances_page: bool,
        on_settings_page: bool,
        key: &mut Option<Key>,
        page_changed: &mut bool,
    ) -> bool {
        let on_games_page = self.cur_page == MenuPage::Games;
        let on_registry_page = self.cur_page == MenuPage::Registry;
        let mut kb_nav_consumed = false;

        for event in &raw_input.events {
            if let egui::Event::Key { key: k, pressed: true, .. } = event {
                match k {
                    Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight | Key::Enter | Key::Escape => {
                        if on_instances_page {
                            kb_nav_consumed |= self.handle_instances_keyboard(*k);
                        }
                        if on_settings_page {
                            kb_nav_consumed |= self.handle_settings_keyboard(*k, key, page_changed);
                        }
                        if on_games_page {
                            kb_nav_consumed |= self.handle_games_keyboard(*k, key);
                        }
                        if on_registry_page {
                            kb_nav_consumed |= self.handle_registry_keyboard(*k, key);
                        }
                    }
                    _ => {}
                }
            }
        }
        kb_nav_consumed
    }

    /// Games page: arrows move focus (same as gamepad), Enter plays the selected
    /// game (GameList focus) or activates the focused action/info button.
    fn handle_games_keyboard(&mut self, k: Key, key: &mut Option<Key>) -> bool {
        match k {
            Key::ArrowUp => self.handle_direction_input(NavDirection::Up, key),
            Key::ArrowDown => self.handle_direction_input(NavDirection::Down, key),
            Key::ArrowLeft => self.handle_direction_input(NavDirection::Left, key),
            Key::ArrowRight => self.handle_direction_input(NavDirection::Right, key),
            Key::Enter => {
                if self.handlers.is_empty() {
                    return false;
                }
                match self.focus_pane {
                    FocusPane::GameList => self.start_game_setup(),
                    FocusPane::ActionBar | FocusPane::InfoPane => self.activate_focused = true,
                }
            }
            _ => return false,
        }
        true
    }

    /// Registry page: arrows move focus, Enter steps into the install button (from
    /// the list) or activates it — mirrors the gamepad A-button behaviour.
    fn handle_registry_keyboard(&mut self, k: Key, key: &mut Option<Key>) -> bool {
        match k {
            Key::ArrowUp => self.handle_direction_input(NavDirection::Up, key),
            Key::ArrowDown => self.handle_direction_input(NavDirection::Down, key),
            Key::ArrowLeft => self.handle_direction_input(NavDirection::Left, key),
            Key::ArrowRight => self.handle_direction_input(NavDirection::Right, key),
            Key::Enter => match self.registry_focus {
                RegistryFocus::HandlerList => {
                    if self.registry_selected.is_some() {
                        self.registry_focus = RegistryFocus::InstallButton;
                    }
                }
                RegistryFocus::InstallButton => self.activate_focused = true,
            },
            Key::Escape => return false,
            _ => return false,
        }
        true
    }

    fn handle_instances_keyboard(&mut self, k: Key) -> bool {
        let kb_action = match k {
            Key::ArrowUp => Some(PadButton::Up),
            Key::ArrowDown => Some(PadButton::Down),
            Key::ArrowLeft => Some(PadButton::Left),
            Key::ArrowRight => Some(PadButton::Right),
            Key::Enter => Some(PadButton::ABtn),
            Key::Escape => Some(PadButton::BBtn),
            _ => None,
        };

        if let Some(btn) = kb_action {
            match btn {
                PadButton::Up | PadButton::Down | PadButton::Left | PadButton::Right => {
                    self.process_instance_nav_key(btn);
                    return true;
                }
                PadButton::ABtn => {
                    self.process_instance_activate_key();
                    return true;
                }
                PadButton::BBtn => {
                    self.process_instance_back_key();
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn handle_settings_keyboard(
        &mut self,
        k: Key,
        key: &mut Option<Key>,
        page_changed: &mut bool,
    ) -> bool {
        // Profile Builder has its own navigation
        if self.is_profile_builder_active() {
            return self.handle_profile_builder_keyboard(k);
        }

        match k {
            Key::ArrowUp => {
                if self.settings_focus == SettingsFocus::Options && self.settings_option_index > 0 {
                    self.settings_option_index -= 1;
                    self.settings_scroll_to_focus = true;
                } else if self.settings_focus == SettingsFocus::BottomButtons {
                    self.settings_focus = SettingsFocus::Options;
                    self.settings_scroll_to_focus = true;
                }
                true
            }
            Key::ArrowDown => {
                const SETTINGS_MAX_OPTIONS: usize = 19;
                if self.settings_focus == SettingsFocus::Options {
                    if self.settings_option_index < SETTINGS_MAX_OPTIONS {
                        self.settings_option_index += 1;
                        self.settings_scroll_to_focus = true;
                    } else {
                        self.settings_focus = SettingsFocus::BottomButtons;
                        self.settings_button_index = 0;
                    }
                } else if self.settings_focus == SettingsFocus::BottomButtons {
                    self.settings_button_index = (self.settings_button_index + 1) % 2;
                }
                true
            }
            Key::ArrowLeft | Key::ArrowRight => {
                if self.settings_focus == SettingsFocus::Options {
                    *key = Some(k);
                } else if k == Key::ArrowLeft && self.settings_button_index > 0 {
                    self.settings_button_index -= 1;
                } else if k == Key::ArrowRight && self.settings_button_index < 1 {
                    self.settings_button_index += 1;
                }
                true
            }
            Key::Enter => {
                self.activate_focused = true;
                true
            }
            Key::Escape => {
                self.cur_page = MenuPage::Games;
                *page_changed = true;
                true
            }
            _ => false,
        }
    }

    fn handle_profile_builder_keyboard(&mut self, k: Key) -> bool {
        use crate::ui::focus::types::NavDirection;

        match k {
            Key::ArrowUp => {
                self.handle_profile_builder_direction(NavDirection::Up);
                true
            }
            Key::ArrowDown => {
                self.handle_profile_builder_direction(NavDirection::Down);
                true
            }
            Key::ArrowLeft => {
                self.handle_profile_builder_direction(NavDirection::Left);
                true
            }
            Key::ArrowRight => {
                self.handle_profile_builder_direction(NavDirection::Right);
                true
            }
            Key::Enter => {
                self.handle_profile_builder_a_button();
                self.activate_focused = true;
                true
            }
            Key::Escape => {
                if !self.handle_profile_builder_b_button() {
                    // Go back to category list
                    self.settings_focus = SettingsFocus::CategoryList;
                }
                true
            }
            _ => false,
        }
    }
}
