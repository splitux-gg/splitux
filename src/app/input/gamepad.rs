//! Main gamepad input handling - entry point that dispatches to other handlers

use crate::app::app::{MenuPage, Splitux};
use crate::input::*;
use crate::ui::focus::pipelines::handle_input::handle_direction;
use crate::ui::focus::types::NavDirection;
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
                    } else if on_settings_page && self.active_dropdown.is_some() {
                        // Close profile preference dropdowns first
                        self.active_dropdown = None;
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
                    self.handle_direction_input(NavDirection::Up, &mut key);
                }
                Some(PadButton::Down) => {
                    self.handle_direction_input(NavDirection::Down, &mut key);
                }
                Some(PadButton::Left) => {
                    self.handle_direction_input(NavDirection::Left, &mut key);
                }
                Some(PadButton::Right) => {
                    self.handle_direction_input(NavDirection::Right, &mut key);
                }
                Some(PadButton::RB) => {
                    self.active_dropdown = None;
                    self.profile_dropdown_open = false;
                    if let Some((new_page, needs_fetch)) = self.cycle_page_forward(registry_needs_fetch) {
                        self.cur_page = new_page;
                        fetch_registry_needed = needs_fetch;
                        page_changed = true;
                    }
                }
                Some(PadButton::LB) => {
                    self.active_dropdown = None;
                    self.profile_dropdown_open = false;
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
        // Focus state is preserved when switching pages (no reset)
        let _ = page_changed;
    }

    fn cycle_page_forward(&self, registry_needs_fetch: bool) -> Option<(MenuPage, bool)> {
        match self.cur_page {
            MenuPage::Games => Some((MenuPage::Registry, registry_needs_fetch)),
            MenuPage::Registry => Some((MenuPage::Settings, false)),
            MenuPage::Settings => Some((MenuPage::Games, false)),
            MenuPage::Instances => Some((MenuPage::Registry, registry_needs_fetch)),
        }
    }

    fn cycle_page_backward(&self, registry_needs_fetch: bool) -> Option<(MenuPage, bool)> {
        match self.cur_page {
            MenuPage::Games => Some((MenuPage::Settings, false)),
            MenuPage::Registry => Some((MenuPage::Games, false)),
            MenuPage::Settings => Some((MenuPage::Registry, registry_needs_fetch)),
            MenuPage::Instances => Some((MenuPage::Settings, false)),
        }
    }

    /// Unified direction input handler
    ///
    /// Uses new focus pipeline for all pages. State is captured via build_nav_context().
    fn handle_direction_input(&mut self, direction: NavDirection, key: &mut Option<Key>) {
        // Handle dropdown navigation first (uses new pipeline)
        if self.profile_dropdown_open {
            let ctx = self.build_nav_context();
            let actions = handle_direction(&ctx, direction);
            self.apply_nav_actions(actions);
            return;
        }

        match self.cur_page {
            MenuPage::Games => {
                if !self.handlers.is_empty() {
                    self.handle_games_direction_new(direction);
                }
            }
            MenuPage::Instances => {
                let ctx = self.build_nav_context();
                let actions = handle_direction(&ctx, direction);
                self.apply_nav_actions(actions);
            }
            MenuPage::Registry => {
                let ctx = self.build_nav_context();
                let actions = handle_direction(&ctx, direction);
                self.apply_nav_actions(actions);
            }
            MenuPage::Settings => {
                // Complex settings states still need legacy handling
                if self.needs_legacy_settings_nav() {
                    match direction {
                        NavDirection::Up => self.handle_settings_up(),
                        NavDirection::Down => self.handle_settings_down(),
                        NavDirection::Left => self.handle_settings_left(key),
                        NavDirection::Right => self.handle_settings_right(key),
                    }
                } else {
                    let ctx = self.build_nav_context();
                    let actions = handle_direction(&ctx, direction);
                    self.apply_nav_actions(actions);
                    // Settings left/right in Options still emits key events
                    if matches!(direction, NavDirection::Left | NavDirection::Right)
                        && self.settings_focus == crate::ui::focus::types::SettingsFocus::Options
                    {
                        *key = Some(match direction {
                            NavDirection::Left => Key::ArrowLeft,
                            NavDirection::Right => Key::ArrowRight,
                            _ => unreachable!(),
                        });
                    }
                }
            }
        }
    }

    /// Check if settings navigation needs legacy handling
    ///
    /// Returns true for complex settings states that the new pipeline doesn't handle:
    /// - Profile preferences expanded (sub-items navigation)
    /// - Profile preference dropdowns open
    fn needs_legacy_settings_nav(&self) -> bool {
        // Profile prefs expanded with sub-focus
        if self.profile_prefs_expanded.is_some() {
            return true;
        }
        // Settings-specific dropdown open
        if self.active_dropdown.is_some() {
            return true;
        }
        false
    }

    /// Games page direction handling using new pipeline
    ///
    /// Uses the new focus pipeline but adds special handling for:
    /// - game_panel_bottom_focused (Add Game / Import buttons)
    /// - games_panel_collapsed auto-expand
    fn handle_games_direction_new(&mut self, direction: NavDirection) {
        use crate::ui::focus::types::FocusPane;

        // Special case: bottom panel navigation (not in new pipeline yet)
        if self.game_panel_bottom_focused {
            match direction {
                NavDirection::Up => {
                    if self.game_panel_bottom_index > 0 {
                        self.game_panel_bottom_index -= 1;
                    } else {
                        self.game_panel_bottom_focused = false;
                    }
                }
                NavDirection::Down => {
                    if self.game_panel_bottom_index < 1 {
                        self.game_panel_bottom_index += 1;
                    }
                }
                NavDirection::Right => {
                    self.focus_pane = FocusPane::ActionBar;
                    self.action_bar_index = 0;
                    self.game_panel_bottom_focused = false;
                }
                NavDirection::Left => {}
            }
            return;
        }

        // Special case: entering bottom panel from last handler
        if self.focus_pane == FocusPane::GameList
            && direction == NavDirection::Down
            && self.selected_handler >= self.handlers.len().saturating_sub(1)
        {
            self.game_panel_bottom_focused = true;
            self.game_panel_bottom_index = 0;
            return;
        }

        // Special case: auto-expand games panel when navigating into it
        if self.focus_pane == FocusPane::ActionBar
            && direction == NavDirection::Left
            && self.action_bar_index == 0
            && self.games_panel_collapsed
        {
            self.games_panel_collapsed = false;
        }

        // Use new pipeline for standard navigation
        let ctx = self.build_nav_context();
        let actions = handle_direction(&ctx, direction);
        self.apply_nav_actions(actions);
    }
}
