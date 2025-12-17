//! Main gamepad input handling - entry point that dispatches to other handlers

use crate::app::app::{MenuPage, Splitux};
use crate::input::*;
use eframe::egui::{self, Key, Vec2};

impl Splitux {
    pub(crate) fn handle_gamepad_gui(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.activate_focused = false;

        let mut key: Option<egui::Key> = None;
        let mut scroll_delta: Option<Vec2> = None;
        let mut page_changed = false;
        let mut start_pressed = false;
        let mut confirm_profile_selection = false;
        let mut open_profile_dropdown = false;
        let mut fetch_registry_needed = false;

        // Cache page state
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

        // Process keyboard navigation
        let kb_nav_consumed = self.process_keyboard_nav(
            raw_input, on_instances_page, on_settings_page, &mut key, &mut page_changed
        );
        if kb_nav_consumed {
            raw_input.events.retain(|event| {
                !matches!(event, egui::Event::Key { key: k, pressed: true, .. }
                    if matches!(k, Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight | Key::Enter | Key::Escape))
            });
        }

        // Collect button presses
        let buttons: Vec<Option<PadButton>> = self.input_devices
            .iter_mut()
            .filter(|pad| pad.enabled())
            .map(|pad| pad.poll())
            .collect();

        // Process each button
        for btn in buttons {
            match btn {
                Some(PadButton::ABtn) => {
                    self.handle_a_button(
                        dropdown_open, dropdown_selection, profiles_len,
                        on_games_page, on_registry_page, on_settings_page, has_handlers,
                        &mut confirm_profile_selection, &mut start_pressed, &mut key,
                    );
                }
                Some(PadButton::BBtn) => {
                    if dropdown_open {
                        self.profile_dropdown_open = false;
                    } else if on_settings_page && (self.profile_ctrl_combo_open.is_some() || self.profile_audio_combo_open.is_some()) {
                        // Close profile preference dropdowns first
                        self.profile_ctrl_combo_open = None;
                        self.profile_audio_combo_open = None;
                    } else if on_settings_page && self.profile_prefs_expanded.is_some() && self.profile_prefs_focus > 0 {
                        // Move focus back to profile header
                        self.profile_prefs_focus = 0;
                    } else if self.handler_lite.is_some() {
                        self.cur_page = MenuPage::Instances;
                        page_changed = true;
                    } else {
                        self.cur_page = MenuPage::Games;
                        page_changed = true;
                    }
                }
                Some(PadButton::XBtn) => {
                    if on_settings_page && self.is_in_profile_section() {
                        // X = Delete profile when focused on a profile entry (index 21+)
                        if self.settings_option_index >= 21 {
                            let profile_idx = self.settings_option_index - 21;
                            self.profile_delete_confirm = Some(profile_idx);
                        }
                    } else if has_handlers && on_games_page {
                        self.handler_edit = Some(self.handlers[self.selected_handler].clone());
                        self.show_edit_modal = true;
                    }
                }
                Some(PadButton::YBtn) => {
                    if on_settings_page && self.is_in_profile_section() {
                        // Y = Rename profile when focused on a profile entry (index 21+)
                        if self.settings_option_index >= 21 {
                            let profile_idx = self.settings_option_index - 21;
                            if profile_idx < self.profiles.len() {
                                self.profile_edit_index = Some(profile_idx);
                                self.profile_rename_buffer = self.profiles[profile_idx].clone();
                            }
                        }
                    } else if on_games_page && has_handlers {
                        if dropdown_open {
                            confirm_profile_selection = true;
                            self.profile_dropdown_open = false;
                        } else {
                            open_profile_dropdown = true;
                        }
                    } else {
                        self.cur_page = MenuPage::Settings;
                        page_changed = true;
                    }
                }
                Some(PadButton::SelectBtn) => key = Some(Key::Tab),
                Some(PadButton::StartBtn) => start_pressed = true,
                Some(PadButton::Up) => {
                    self.handle_up(
                        dropdown_open, profiles_len, on_games_page, on_registry_page,
                        on_settings_page, on_instances_page, has_handlers, registry_handler_count, &mut key,
                    );
                }
                Some(PadButton::Down) => {
                    self.handle_down(
                        dropdown_open, profiles_len, on_games_page, on_registry_page,
                        on_settings_page, on_instances_page, has_handlers, registry_handler_count, &mut key,
                    );
                }
                Some(PadButton::Left) => {
                    self.handle_left(
                        dropdown_open, on_games_page, on_registry_page,
                        on_settings_page, on_instances_page, has_handlers, &mut key,
                    );
                }
                Some(PadButton::Right) => {
                    self.handle_right(
                        dropdown_open, on_games_page, on_registry_page,
                        on_settings_page, on_instances_page, has_handlers, &mut key,
                    );
                }
                Some(PadButton::RB) => {
                    if let Some((new_page, needs_fetch)) = self.cycle_page_forward(registry_needs_fetch) {
                        self.cur_page = new_page;
                        fetch_registry_needed = needs_fetch;
                        page_changed = true;
                    }
                }
                Some(PadButton::LB) => {
                    if let Some((new_page, needs_fetch)) = self.cycle_page_backward(registry_needs_fetch) {
                        self.cur_page = new_page;
                        fetch_registry_needed = needs_fetch;
                        page_changed = true;
                    }
                }
                Some(PadButton::ScrollUp) => {
                    self.info_pane_scroll -= 60.0;
                    scroll_delta = Some(Vec2::new(0.0, 60.0));
                }
                Some(PadButton::ScrollDown) => {
                    self.info_pane_scroll += 60.0;
                    scroll_delta = Some(Vec2::new(0.0, -60.0));
                }
                Some(PadButton::LT) | Some(PadButton::RT) | Some(_) | None => {}
            }
        }

        // Inject key events
        if let Some(key) = key {
            raw_input.events.push(egui::Event::Key {
                key, physical_key: None, pressed: true, repeat: false,
                modifiers: egui::Modifiers::default(),
            });
        }

        // Inject scroll events
        if let Some(delta) = scroll_delta {
            let center = ctx.screen_rect().center();
            raw_input.events.push(egui::Event::PointerMoved(center));
            raw_input.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point, delta,
                modifiers: egui::Modifiers::default(),
            });
        }

        // Handle deferred actions
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
        if start_pressed && on_games_page && has_handlers {
            self.start_game_setup();
            page_changed = true;
        }
        if page_changed {
            self.reset_page_focus(ctx);
        }
    }

    fn cycle_page_forward(&self, registry_needs_fetch: bool) -> Option<(MenuPage, bool)> {
        match self.cur_page {
            MenuPage::Games => Some((MenuPage::Registry, registry_needs_fetch)),
            MenuPage::Registry => Some((MenuPage::Settings, false)),
            MenuPage::Settings => Some((MenuPage::Games, false)),
            MenuPage::Instances => None,
        }
    }

    fn cycle_page_backward(&self, registry_needs_fetch: bool) -> Option<(MenuPage, bool)> {
        match self.cur_page {
            MenuPage::Games => Some((MenuPage::Settings, false)),
            MenuPage::Registry => Some((MenuPage::Games, false)),
            MenuPage::Settings => Some((MenuPage::Registry, registry_needs_fetch)),
            MenuPage::Instances => None,
        }
    }
}
