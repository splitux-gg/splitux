//! Navigation action application
//!
//! Provides the bridge between pure navigation functions and app state mutation.
//! - `build_nav_context()` - Snapshot current state into NavContext
//! - `apply_nav_actions()` - Apply NavAction results to mutate state

use crate::app::app::Splitux;
use crate::ui::focus::pipelines::handle_input::{NavAction, NavContext};
use crate::ui::MenuPage;

impl Splitux {
    /// Build a NavContext snapshot from current app state
    ///
    /// This captures the state needed for pure navigation functions
    /// to make decisions without accessing the full app state.
    pub fn build_nav_context(&self) -> NavContext {
        NavContext {
            page: self.cur_page,
            // Games page state
            focus_pane: self.focus_pane,
            action_bar_index: self.action_bar_index,
            info_pane_index: self.info_pane_index,
            selected_handler: self.selected_handler,
            handlers_count: self.handlers.len(),
            // Instances page state
            instance_focus: self.instance_focus.clone(),
            launch_option_index: self.launch_option_index,
            instances_count: self.instances.len(),
            // Registry page state
            registry_focus: self.registry_focus,
            registry_selected: self.registry_selected,
            registry_handler_count: self
                .registry_index
                .as_ref()
                .map(|r| r.handlers.len())
                .unwrap_or(0),
            // Settings page state
            settings_focus: self.settings_focus,
            settings_category: self.settings_category,
            settings_option_index: self.settings_option_index,
            settings_button_index: self.settings_button_index,
            settings_max_options: self.settings_max_option_index(),
            // Dropdown state
            dropdown_open: self.profile_dropdown_open,
            dropdown_selection: self.profile_dropdown_selection,
            dropdown_total: self.profiles.len() + 1, // +1 for "New Profile" option
        }
    }

    /// Apply navigation actions to mutate app state
    ///
    /// Takes the declarative actions returned by pure navigation functions
    /// and applies them to the mutable app state.
    pub fn apply_nav_actions(&mut self, actions: Vec<NavAction>) {
        for action in actions {
            match action {
                NavAction::None => {}
                // Games page actions
                NavAction::SetFocusPane(pane) => {
                    self.focus_pane = pane;
                }
                NavAction::SetActionBarIndex(idx) => {
                    self.action_bar_index = idx;
                }
                NavAction::SetInfoPaneIndex(idx) => {
                    self.info_pane_index = idx;
                }
                NavAction::SetSelectedHandler(idx) => {
                    if idx < self.handlers.len() {
                        self.selected_handler = idx;
                    }
                }
                // Instances page actions
                NavAction::SetInstanceFocus(focus) => {
                    self.instance_focus = focus;
                }
                NavAction::SetLaunchOptionIndex(idx) => {
                    self.launch_option_index = idx;
                }
                // Registry page actions
                NavAction::SetRegistryFocus(focus) => {
                    self.registry_focus = focus;
                }
                NavAction::SetRegistrySelected(sel) => {
                    self.registry_selected = sel;
                }
                // Settings page actions
                NavAction::SetSettingsFocus(focus) => {
                    self.settings_focus = focus;
                }
                NavAction::SetSettingsCategory(cat) => {
                    self.settings_category = cat;
                }
                NavAction::SetSettingsOptionIndex(idx) => {
                    self.settings_option_index = idx;
                }
                NavAction::SetSettingsButtonIndex(idx) => {
                    self.settings_button_index = idx;
                }
                NavAction::ScrollToFocus => {
                    self.settings_scroll_to_focus = true;
                }
                // Shared actions
                NavAction::SetDropdownSelection(idx) => {
                    self.profile_dropdown_selection = idx;
                }
                NavAction::ChangePage(page) => {
                    self.change_page(page);
                }
            }
        }
    }

    /// Change to a new page with proper focus reset
    fn change_page(&mut self, page: MenuPage) {
        use crate::ui::focus::pipelines::handle_input::reset_focus_for_page;

        self.cur_page = page;

        // Apply focus reset actions for the new page
        let reset_actions = reset_focus_for_page(page);
        for action in reset_actions {
            match action {
                NavAction::SetFocusPane(pane) => self.focus_pane = pane,
                NavAction::SetActionBarIndex(idx) => self.action_bar_index = idx,
                NavAction::SetInfoPaneIndex(idx) => self.info_pane_index = idx,
                NavAction::SetInstanceFocus(focus) => self.instance_focus = focus,
                NavAction::SetLaunchOptionIndex(idx) => self.launch_option_index = idx,
                NavAction::SetRegistryFocus(focus) => self.registry_focus = focus,
                NavAction::SetSettingsFocus(focus) => self.settings_focus = focus,
                NavAction::SetSettingsOptionIndex(idx) => self.settings_option_index = idx,
                _ => {}
            }
        }
    }
}
