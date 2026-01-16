//! Profile Builder (KB/Mouse Mapper) gamepad navigation
//!
//! Handles d-pad navigation and button actions within the Profile Builder settings page.

use crate::app::app::{ProfileBuilderFocus, SettingsCategory, SettingsFocus, Splitux};
use crate::gptokeyb::{delete_profile, list_user_profiles, load_user_profile};
use crate::ui::components::controller_diagram::DIAGRAM_BUTTONS;
use crate::ui::focus::types::NavDirection;

impl Splitux {
    /// Check if we should use profile builder navigation
    pub fn is_profile_builder_active(&self) -> bool {
        self.settings_category == SettingsCategory::ProfileBuilder
            && self.settings_focus == SettingsFocus::Options
    }

    /// Handle d-pad navigation within Profile Builder
    pub fn handle_profile_builder_direction(&mut self, direction: NavDirection) {
        let in_editor = self.profile_builder_editing.is_some();
        let has_selected_button = self.profile_builder_selected_button.is_some();

        if in_editor {
            self.navigate_profile_editor(direction, has_selected_button);
        } else {
            self.navigate_profile_list(direction);
        }
    }

    /// Navigate the profile list view
    fn navigate_profile_list(&mut self, direction: NavDirection) {
        let profile_count = self.profile_builder_profiles.len();

        match self.profile_builder_focus {
            ProfileBuilderFocus::NewButton => match direction {
                NavDirection::Down => {
                    if profile_count > 0 {
                        self.profile_builder_focus = ProfileBuilderFocus::ProfileRow(0, 0);
                    }
                }
                NavDirection::Left => {
                    // Go back to category list
                    self.settings_focus = SettingsFocus::CategoryList;
                }
                _ => {}
            },
            ProfileBuilderFocus::ProfileRow(idx, sub) => match direction {
                NavDirection::Up => {
                    if idx > 0 {
                        self.profile_builder_focus = ProfileBuilderFocus::ProfileRow(idx - 1, 0);
                    } else {
                        self.profile_builder_focus = ProfileBuilderFocus::NewButton;
                    }
                }
                NavDirection::Down => {
                    if idx + 1 < profile_count {
                        self.profile_builder_focus = ProfileBuilderFocus::ProfileRow(idx + 1, 0);
                    }
                }
                NavDirection::Left => {
                    if sub > 0 {
                        self.profile_builder_focus = ProfileBuilderFocus::ProfileRow(idx, sub - 1);
                    } else {
                        self.settings_focus = SettingsFocus::CategoryList;
                    }
                }
                NavDirection::Right => {
                    if sub < 2 {
                        self.profile_builder_focus = ProfileBuilderFocus::ProfileRow(idx, sub + 1);
                    }
                }
            },
            // Shouldn't be in editor states while in list view
            _ => {
                self.profile_builder_focus = ProfileBuilderFocus::NewButton;
            }
        }
    }

    /// Navigate the profile editor view
    fn navigate_profile_editor(&mut self, direction: NavDirection, has_selected_button: bool) {
        match self.profile_builder_focus {
            // Header row: Name -> Save -> Cancel
            ProfileBuilderFocus::NameInput => match direction {
                NavDirection::Right => {
                    self.profile_builder_focus = ProfileBuilderFocus::SaveButton;
                }
                NavDirection::Down => {
                    self.profile_builder_focus = ProfileBuilderFocus::DiagramButton(0);
                }
                _ => {}
            },
            ProfileBuilderFocus::SaveButton => match direction {
                NavDirection::Left => {
                    self.profile_builder_focus = ProfileBuilderFocus::NameInput;
                }
                NavDirection::Right => {
                    self.profile_builder_focus = ProfileBuilderFocus::CancelButton;
                }
                NavDirection::Down => {
                    self.profile_builder_focus = ProfileBuilderFocus::DiagramButton(0);
                }
                _ => {}
            },
            ProfileBuilderFocus::CancelButton => match direction {
                NavDirection::Left => {
                    self.profile_builder_focus = ProfileBuilderFocus::SaveButton;
                }
                NavDirection::Down => {
                    self.profile_builder_focus = ProfileBuilderFocus::DiagramButton(0);
                }
                _ => {}
            },

            // Diagram navigation
            ProfileBuilderFocus::DiagramButton(idx) => {
                let new_idx = self.navigate_diagram_button(idx, direction);
                if let Some(new_idx) = new_idx {
                    self.profile_builder_focus = ProfileBuilderFocus::DiagramButton(new_idx);
                } else {
                    // Exit diagram
                    match direction {
                        NavDirection::Up => {
                            self.profile_builder_focus = ProfileBuilderFocus::NameInput;
                        }
                        NavDirection::Down => {
                            if has_selected_button {
                                self.profile_builder_focus = ProfileBuilderFocus::MappingInput;
                            } else {
                                self.profile_builder_focus = ProfileBuilderFocus::RightStickMouse;
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Mapping input row
            ProfileBuilderFocus::MappingInput => match direction {
                NavDirection::Right => {
                    self.profile_builder_focus = ProfileBuilderFocus::ClearMapping;
                }
                NavDirection::Up => {
                    self.profile_builder_focus = ProfileBuilderFocus::DiagramButton(0);
                }
                NavDirection::Down => {
                    self.profile_builder_focus = ProfileBuilderFocus::RightStickMouse;
                }
                _ => {}
            },
            ProfileBuilderFocus::ClearMapping => match direction {
                NavDirection::Left => {
                    self.profile_builder_focus = ProfileBuilderFocus::MappingInput;
                }
                NavDirection::Up => {
                    self.profile_builder_focus = ProfileBuilderFocus::DiagramButton(0);
                }
                NavDirection::Down => {
                    self.profile_builder_focus = ProfileBuilderFocus::RightStickMouse;
                }
                _ => {}
            },

            // Config row
            ProfileBuilderFocus::RightStickMouse => match direction {
                NavDirection::Right => {
                    self.profile_builder_focus = ProfileBuilderFocus::LeftStickMouse;
                }
                NavDirection::Up => {
                    if has_selected_button {
                        self.profile_builder_focus = ProfileBuilderFocus::MappingInput;
                    } else {
                        self.profile_builder_focus = ProfileBuilderFocus::DiagramButton(0);
                    }
                }
                NavDirection::Down => {
                    self.profile_builder_focus = ProfileBuilderFocus::MouseSpeed;
                }
                _ => {}
            },
            ProfileBuilderFocus::LeftStickMouse => match direction {
                NavDirection::Left => {
                    self.profile_builder_focus = ProfileBuilderFocus::RightStickMouse;
                }
                NavDirection::Up => {
                    if has_selected_button {
                        self.profile_builder_focus = ProfileBuilderFocus::MappingInput;
                    } else {
                        self.profile_builder_focus = ProfileBuilderFocus::DiagramButton(0);
                    }
                }
                NavDirection::Down => {
                    self.profile_builder_focus = ProfileBuilderFocus::MouseSpeed;
                }
                _ => {}
            },
            ProfileBuilderFocus::MouseSpeed => match direction {
                NavDirection::Up => {
                    self.profile_builder_focus = ProfileBuilderFocus::RightStickMouse;
                }
                _ => {}
            },

            // Default case
            _ => {
                self.profile_builder_focus = ProfileBuilderFocus::NameInput;
            }
        }
    }

    /// Navigate between diagram buttons spatially
    /// Returns Some(new_index) if staying in diagram, None if exiting
    fn navigate_diagram_button(&self, current_idx: usize, direction: NavDirection) -> Option<usize> {
        // Simple linear navigation through diagram buttons
        // Could be improved with spatial navigation based on button positions
        let count = DIAGRAM_BUTTONS.len();

        match direction {
            NavDirection::Up => {
                // Group navigation: move between rows
                if current_idx >= 4 {
                    Some(current_idx.saturating_sub(4).min(count - 1))
                } else {
                    None // Exit diagram upward
                }
            }
            NavDirection::Down => {
                // Move down in grid
                if current_idx + 4 < count {
                    Some(current_idx + 4)
                } else {
                    None // Exit diagram downward
                }
            }
            NavDirection::Left => {
                if current_idx > 0 {
                    Some(current_idx - 1)
                } else {
                    Some(current_idx) // Stay at first
                }
            }
            NavDirection::Right => {
                if current_idx + 1 < count {
                    Some(current_idx + 1)
                } else {
                    Some(current_idx) // Stay at last
                }
            }
        }
    }

    /// Handle A button press in Profile Builder
    pub fn handle_profile_builder_a_button(&mut self) -> bool {
        let in_editor = self.profile_builder_editing.is_some();

        if in_editor {
            self.handle_editor_a_button()
        } else {
            self.handle_list_a_button()
        }
    }

    fn handle_list_a_button(&mut self) -> bool {
        match self.profile_builder_focus {
            ProfileBuilderFocus::NewButton => {
                // Create new profile
                self.create_new_profile_from_nav();
                true
            }
            ProfileBuilderFocus::ProfileRow(idx, sub) => {
                let profiles = self.profile_builder_profiles.clone();
                if let Some(name) = profiles.get(idx) {
                    match sub {
                        0 | 1 => {
                            // Row or Edit button - edit the profile
                            if let Ok(profile) = load_user_profile(name) {
                                self.profile_builder_name_buffer = profile.name.clone();
                                self.profile_builder_editing = Some(profile);
                                self.profile_builder_focus = ProfileBuilderFocus::NameInput;
                            }
                        }
                        2 => {
                            // Delete button
                            let _ = delete_profile(name);
                            self.profile_builder_profiles = list_user_profiles();
                            // Adjust focus if needed
                            if idx >= self.profile_builder_profiles.len() {
                                if self.profile_builder_profiles.is_empty() {
                                    self.profile_builder_focus = ProfileBuilderFocus::NewButton;
                                } else {
                                    self.profile_builder_focus = ProfileBuilderFocus::ProfileRow(
                                        self.profile_builder_profiles.len() - 1,
                                        0,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn handle_editor_a_button(&mut self) -> bool {
        match self.profile_builder_focus {
            ProfileBuilderFocus::SaveButton => {
                // Trigger save via activate_focused
                true
            }
            ProfileBuilderFocus::CancelButton => {
                // Cancel editing
                self.profile_builder_editing = None;
                self.profile_builder_selected_button = None;
                self.profile_builder_focus = ProfileBuilderFocus::NewButton;
                true
            }
            ProfileBuilderFocus::DiagramButton(idx) => {
                // Select this button for mapping
                if let Some(&button) = DIAGRAM_BUTTONS.get(idx) {
                    self.profile_builder_selected_button = Some(button);
                    self.profile_builder_focus = ProfileBuilderFocus::MappingInput;
                }
                true
            }
            ProfileBuilderFocus::ClearMapping => {
                // Clear the current mapping
                if let (Some(ref mut profile), Some(button)) = (
                    self.profile_builder_editing.as_mut(),
                    self.profile_builder_selected_button,
                ) {
                    profile.clear_mapping(button);
                }
                true
            }
            ProfileBuilderFocus::RightStickMouse | ProfileBuilderFocus::LeftStickMouse => {
                // Toggle checkbox - handled by activate_focused
                true
            }
            _ => false,
        }
    }

    fn create_new_profile_from_nav(&mut self) {
        use crate::gptokeyb::{AnalogMode, GptokeybProfile};
        let mut profile = GptokeybProfile::new("my_profile");
        profile.right_analog_mode = AnalogMode::MouseMovement;
        self.profile_builder_editing = Some(profile);
        self.profile_builder_name_buffer = "my_profile".to_string();
        self.profile_builder_focus = ProfileBuilderFocus::NameInput;
    }

    /// Handle B button press in Profile Builder editor (cancel)
    pub fn handle_profile_builder_b_button(&mut self) -> bool {
        if self.profile_builder_editing.is_some() {
            // Check current focus to decide what to do
            match self.profile_builder_focus {
                ProfileBuilderFocus::MappingInput | ProfileBuilderFocus::ClearMapping => {
                    // Clear selection, go back to diagram
                    self.profile_builder_selected_button = None;
                    self.profile_builder_focus = ProfileBuilderFocus::DiagramButton(0);
                    true
                }
                _ => {
                    // Cancel editing entirely
                    self.profile_builder_editing = None;
                    self.profile_builder_selected_button = None;
                    self.profile_builder_focus = ProfileBuilderFocus::NewButton;
                    true
                }
            }
        } else {
            false // Let parent handle
        }
    }
}
