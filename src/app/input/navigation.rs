//! Navigation handling - legacy settings handlers and helpers
//!
//! Most navigation has been migrated to the new focus pipeline system.
//! This module contains:
//! - Settings page handlers (complex profile prefs/dropdown navigation)
//! - Helper methods used by the pipeline

use crate::app::app::{ActiveDropdown, FocusPane, RegistryFocus, SettingsFocus, Splitux};
use eframe::egui::{self, Key};

impl Splitux {
    // =========================================================================
    // Settings helpers (used by new pipeline via build_nav_context)
    // =========================================================================

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

    // =========================================================================
    // Settings page navigation (legacy - handles complex profile prefs/dropdowns)
    // =========================================================================

    pub(super) fn handle_settings_up(&mut self) {
        // If a profile pref dropdown is open, navigate within it
        if self.active_dropdown.is_some() {
            if self.dropdown_selection_idx > 0 {
                self.dropdown_selection_idx -= 1;
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
                    self.active_dropdown = None;

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

    pub(super) fn handle_settings_down(&mut self) {
        // If a profile pref dropdown is open, navigate within it
        if let Some(ref dropdown) = self.active_dropdown {
            // Calculate max items (None + devices count)
            let max_items = match dropdown {
                ActiveDropdown::ProfileController(_) => {
                    1 + self.input_devices.iter().filter(|d| !d.uniq().is_empty()).count()
                }
                ActiveDropdown::ProfileAudio(_) => 1 + self.audio_devices.len(),
                ActiveDropdown::GameProfile => 1 + self.profiles.len(),
            };
            if self.dropdown_selection_idx < max_items.saturating_sub(1) {
                self.dropdown_selection_idx += 1;
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
                    self.active_dropdown = None;
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

    pub(super) fn handle_settings_left(&mut self, key: &mut Option<Key>) {
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

    pub(super) fn handle_settings_right(&mut self, key: &mut Option<Key>) {
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

    // =========================================================================
    // Page focus reset
    // =========================================================================

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
