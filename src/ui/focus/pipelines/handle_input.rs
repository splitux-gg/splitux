// Main input handling entry point
//
// This module orchestrates navigation input processing using pure functions.
// It provides a high-level API that app code can call.

use crate::ui::focus::pure::{
    apply_index_delta, navigate_dropdown, navigate_games_page, navigate_instances_page,
    GamesPaneNav, InstancesNav,
};
use crate::ui::focus::types::{FocusPane, InstanceFocus, NavDirection, RegistryFocus, SettingsFocus};
use crate::ui::MenuPage;

/// State snapshot needed for navigation decisions
pub struct NavContext {
    pub page: MenuPage,
    // Games page state
    pub focus_pane: FocusPane,
    pub action_bar_index: usize,
    pub info_pane_index: usize,
    pub selected_handler: usize,
    pub handlers_count: usize,
    // Instances page state
    pub instance_focus: InstanceFocus,
    pub launch_option_index: usize,
    pub instances_count: usize,
    // Registry page state
    pub registry_focus: RegistryFocus,
    pub registry_selected: Option<usize>,
    pub registry_handler_count: usize,
    // Settings page state
    pub settings_focus: SettingsFocus,
    pub settings_option_index: usize,
    pub settings_button_index: usize,
    pub settings_max_options: usize,
    // Dropdown state (shared across pages)
    pub dropdown_open: bool,
    pub dropdown_selection: usize,
    pub dropdown_total: usize,
}

/// Result of handling a navigation input
#[derive(Debug, Clone)]
pub enum NavAction {
    /// No action needed
    None,
    // Games page actions
    /// Update focus pane
    SetFocusPane(FocusPane),
    /// Update action bar selection
    SetActionBarIndex(usize),
    /// Update info pane selection
    SetInfoPaneIndex(usize),
    /// Update selected handler (game list)
    SetSelectedHandler(usize),
    // Instances page actions
    /// Update instance focus area
    SetInstanceFocus(InstanceFocus),
    /// Update launch option selection
    SetLaunchOptionIndex(usize),
    // Registry page actions
    /// Update registry focus area
    SetRegistryFocus(RegistryFocus),
    /// Update registry selected handler
    SetRegistrySelected(Option<usize>),
    // Settings page actions
    /// Update settings focus area
    SetSettingsFocus(SettingsFocus),
    /// Update settings option index
    SetSettingsOptionIndex(usize),
    /// Update settings button index
    SetSettingsButtonIndex(usize),
    /// Trigger scroll to focus
    ScrollToFocus,
    // Shared actions
    /// Update dropdown selection
    SetDropdownSelection(usize),
    /// Navigate to a different page - reserved for future use
    #[allow(dead_code)]
    ChangePage(MenuPage),
}

/// Process directional navigation input
pub fn handle_direction(ctx: &NavContext, direction: NavDirection) -> Vec<NavAction> {
    if ctx.dropdown_open {
        // When dropdown is open, only up/down navigation
        let new_selection = navigate_dropdown(ctx.dropdown_selection, ctx.dropdown_total, direction);
        return vec![NavAction::SetDropdownSelection(new_selection)];
    }

    match ctx.page {
        MenuPage::Games => handle_games_direction(ctx, direction),
        MenuPage::Instances => handle_instances_direction(ctx, direction),
        MenuPage::Registry => handle_registry_direction(ctx, direction),
        MenuPage::Settings => handle_settings_direction(ctx, direction),
    }
}

fn handle_games_direction(ctx: &NavContext, direction: NavDirection) -> Vec<NavAction> {
    if ctx.handlers_count == 0 {
        return vec![NavAction::None];
    }

    let result = navigate_games_page(
        ctx.focus_pane,
        direction,
        ctx.action_bar_index,
        3, // Action bar has 3 buttons: Play, Profile, Edit
    );

    match result {
        GamesPaneNav::SwitchPane(new_pane) => {
            let mut actions = vec![NavAction::SetFocusPane(new_pane)];
            // Reset index when entering action bar
            if new_pane == FocusPane::ActionBar {
                actions.push(NavAction::SetActionBarIndex(0));
            }
            actions
        }
        GamesPaneNav::MoveWithin { delta } => {
            match ctx.focus_pane {
                FocusPane::GameList => {
                    let new_idx = apply_index_delta(ctx.selected_handler, delta, ctx.handlers_count);
                    vec![NavAction::SetSelectedHandler(new_idx)]
                }
                FocusPane::ActionBar => {
                    let new_idx = apply_index_delta(ctx.action_bar_index, delta, 3);
                    vec![NavAction::SetActionBarIndex(new_idx)]
                }
                FocusPane::InfoPane => {
                    // Info pane index has no fixed max - let the caller handle bounds
                    let new_idx = if delta < 0 {
                        ctx.info_pane_index.saturating_sub((-delta) as usize)
                    } else {
                        ctx.info_pane_index + delta as usize
                    };
                    vec![NavAction::SetInfoPaneIndex(new_idx)]
                }
            }
        }
        GamesPaneNav::None => vec![NavAction::None],
    }
}

fn handle_instances_direction(ctx: &NavContext, direction: NavDirection) -> Vec<NavAction> {
    let max_options = if ctx.instances_count == 2 { 2 } else { 1 };

    let result = navigate_instances_page(
        ctx.instance_focus.clone(),
        direction,
        ctx.launch_option_index,
        max_options,
        ctx.instances_count > 0,
    );

    match result {
        InstancesNav::SwitchFocus(new_focus) => {
            let is_launch_options = new_focus == InstanceFocus::LaunchOptions;
            let mut actions = vec![NavAction::SetInstanceFocus(new_focus)];
            if is_launch_options {
                actions.push(NavAction::SetLaunchOptionIndex(0));
            }
            actions
        }
        InstancesNav::MoveWithin { delta } => {
            let new_idx = apply_index_delta(ctx.launch_option_index, delta, max_options);
            vec![NavAction::SetLaunchOptionIndex(new_idx)]
        }
        InstancesNav::None => vec![NavAction::None],
    }
}

fn handle_registry_direction(ctx: &NavContext, direction: NavDirection) -> Vec<NavAction> {
    match ctx.registry_focus {
        RegistryFocus::HandlerList => match direction {
            NavDirection::Up => {
                if let Some(sel) = ctx.registry_selected {
                    if sel > 0 {
                        vec![NavAction::SetRegistrySelected(Some(sel - 1))]
                    } else {
                        vec![NavAction::None]
                    }
                } else if ctx.registry_handler_count > 0 {
                    vec![NavAction::SetRegistrySelected(Some(0))]
                } else {
                    vec![NavAction::None]
                }
            }
            NavDirection::Down => {
                if let Some(sel) = ctx.registry_selected {
                    if sel + 1 < ctx.registry_handler_count {
                        vec![NavAction::SetRegistrySelected(Some(sel + 1))]
                    } else {
                        vec![NavAction::None]
                    }
                } else if ctx.registry_handler_count > 0 {
                    vec![NavAction::SetRegistrySelected(Some(0))]
                } else {
                    vec![NavAction::None]
                }
            }
            NavDirection::Right => {
                if ctx.registry_selected.is_some() {
                    vec![NavAction::SetRegistryFocus(RegistryFocus::InstallButton)]
                } else {
                    vec![NavAction::None]
                }
            }
            NavDirection::Left => vec![NavAction::None],
        },
        RegistryFocus::InstallButton => match direction {
            NavDirection::Left => vec![NavAction::SetRegistryFocus(RegistryFocus::HandlerList)],
            _ => vec![NavAction::None],
        },
    }
}

fn handle_settings_direction(ctx: &NavContext, direction: NavDirection) -> Vec<NavAction> {
    match ctx.settings_focus {
        SettingsFocus::Options => match direction {
            NavDirection::Up => {
                if ctx.settings_option_index > 0 {
                    vec![
                        NavAction::SetSettingsOptionIndex(ctx.settings_option_index - 1),
                        NavAction::ScrollToFocus,
                    ]
                } else {
                    vec![NavAction::None]
                }
            }
            NavDirection::Down => {
                if ctx.settings_option_index < ctx.settings_max_options {
                    vec![
                        NavAction::SetSettingsOptionIndex(ctx.settings_option_index + 1),
                        NavAction::ScrollToFocus,
                    ]
                } else {
                    vec![
                        NavAction::SetSettingsFocus(SettingsFocus::BottomButtons),
                        NavAction::SetSettingsButtonIndex(0),
                    ]
                }
            }
            // Left/Right in Options area - pass through as key events (handled by caller)
            NavDirection::Left | NavDirection::Right => vec![NavAction::None],
        },
        SettingsFocus::BottomButtons => match direction {
            NavDirection::Up => {
                vec![
                    NavAction::SetSettingsFocus(SettingsFocus::Options),
                    NavAction::ScrollToFocus,
                ]
            }
            NavDirection::Left => {
                if ctx.settings_button_index > 0 {
                    vec![NavAction::SetSettingsButtonIndex(ctx.settings_button_index - 1)]
                } else {
                    vec![NavAction::None]
                }
            }
            NavDirection::Right => {
                if ctx.settings_button_index < 1 {
                    vec![NavAction::SetSettingsButtonIndex(ctx.settings_button_index + 1)]
                } else {
                    vec![NavAction::None]
                }
            }
            NavDirection::Down => {
                // Wrap around to first button
                vec![NavAction::SetSettingsButtonIndex((ctx.settings_button_index + 1) % 2)]
            }
        },
    }
}

/// Reset focus state when page changes
pub fn reset_focus_for_page(page: MenuPage) -> Vec<NavAction> {
    match page {
        MenuPage::Games => vec![
            NavAction::SetFocusPane(FocusPane::GameList),
            NavAction::SetActionBarIndex(0),
            NavAction::SetInfoPaneIndex(0),
        ],
        MenuPage::Instances => vec![
            NavAction::SetInstanceFocus(InstanceFocus::Devices),
            NavAction::SetLaunchOptionIndex(0),
        ],
        _ => vec![],
    }
}
