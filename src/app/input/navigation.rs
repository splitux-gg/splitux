//! Navigation handling - legacy settings handlers and helpers
//!
//! Most navigation has been migrated to the new focus pipeline system.
//! This module contains:
//! - Settings page handlers (complex profile prefs/dropdown navigation)
//! - Helper methods used by the pipeline

use crate::app::app::{ActiveDropdown, SettingsCategory, SettingsFocus, Splitux};
use eframe::egui::Key;

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
            SettingsFocus::CategoryList => {
                // Move up in category list
                let idx = self.settings_category.to_index();
                if idx > 0 {
                    self.settings_category = SettingsCategory::from_index(idx - 1);
                }
            }
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
                } else {
                    // At top of options, go back to category list
                    self.settings_focus = SettingsFocus::CategoryList;
                }
            }
            SettingsFocus::BottomButtons => {
                self.settings_focus = SettingsFocus::CategoryList;
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
                // Instance dropdowns are handled in instances.rs, not here
                ActiveDropdown::InstanceProfile(_)
                | ActiveDropdown::InstanceMonitor(_)
                | ActiveDropdown::InstanceAudioOverride(_)
                | ActiveDropdown::InstanceAudioPreference(_)
                | ActiveDropdown::InstanceGptokeyb(_) => return,
            };
            if self.dropdown_selection_idx < max_items.saturating_sub(1) {
                self.dropdown_selection_idx += 1;
            }
            return;
        }

        let max_options = self.settings_max_option_index();
        match self.settings_focus {
            SettingsFocus::CategoryList => {
                // Move down in category list, then to bottom buttons
                let idx = self.settings_category.to_index();
                if idx < SettingsCategory::count() - 1 {
                    self.settings_category = SettingsCategory::from_index(idx + 1);
                } else {
                    self.settings_focus = SettingsFocus::BottomButtons;
                    self.settings_button_index = 0;
                }
            }
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
                    // At bottom of options, go to category list
                    self.settings_focus = SettingsFocus::CategoryList;
                }
            }
            SettingsFocus::BottomButtons => {
                self.settings_button_index = (self.settings_button_index + 1) % 2;
            }
        }
    }

    pub(super) fn handle_settings_left(&mut self, key: &mut Option<Key>) {
        match self.settings_focus {
            SettingsFocus::CategoryList => {
                // Collapse panel when pressing left on category list
                self.settings_panel_collapsed = true;
            }
            SettingsFocus::Options => {
                // Return to category list or handle option left/right
                if self.settings_option_index == 0 {
                    self.settings_focus = SettingsFocus::CategoryList;
                } else {
                    *key = Some(Key::ArrowLeft);
                }
            }
            SettingsFocus::BottomButtons => {
                if self.settings_button_index > 0 {
                    self.settings_button_index -= 1;
                } else {
                    // Go back to category list
                    self.settings_focus = SettingsFocus::CategoryList;
                }
            }
        }
    }

    pub(super) fn handle_settings_right(&mut self, key: &mut Option<Key>) {
        match self.settings_focus {
            SettingsFocus::CategoryList => {
                // Enter options panel
                self.settings_focus = SettingsFocus::Options;
                self.settings_option_index = 0;
                self.settings_scroll_to_focus = true;
            }
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

}
