//! Keyboard navigation handling

use crate::app::app::{MenuPage, SettingsFocus, Splitux};
use crate::input::PadButton;
use eframe::egui::{self, Key};

impl Splitux {
    /// Process keyboard navigation events, returns true if events were consumed
    pub(super) fn process_keyboard_nav(
        &mut self,
        raw_input: &egui::RawInput,
        on_instances_page: bool,
        on_settings_page: bool,
        key: &mut Option<Key>,
        page_changed: &mut bool,
    ) -> bool {
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
                    }
                    _ => {}
                }
            }
        }
        kb_nav_consumed
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
}
