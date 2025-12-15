// Main input handling entry point
//
// This module orchestrates navigation input processing using pure functions.
// It provides a high-level API that app code can call.

use crate::ui::focus::pure::{
    apply_index_delta, navigate_dropdown, navigate_games_page, navigate_instances_page,
    GamesPaneNav, InstancesNav,
};
use crate::ui::focus::types::{FocusPane, InstanceFocus, NavDirection};
use crate::ui::MenuPage;

/// State snapshot needed for navigation decisions
pub struct NavContext {
    pub page: MenuPage,
    pub focus_pane: FocusPane,
    pub instance_focus: InstanceFocus,
    pub action_bar_index: usize,
    pub info_pane_index: usize,
    pub selected_handler: usize,
    pub handlers_count: usize,
    pub launch_option_index: usize,
    pub instances_count: usize,
    pub dropdown_open: bool,
    pub dropdown_selection: usize,
    pub dropdown_total: usize,
}

/// Result of handling a navigation input
#[derive(Debug, Clone)]
pub enum NavAction {
    /// No action needed
    None,
    /// Update focus pane
    SetFocusPane(FocusPane),
    /// Update instance focus area
    SetInstanceFocus(InstanceFocus),
    /// Update action bar selection
    SetActionBarIndex(usize),
    /// Update info pane selection
    SetInfoPaneIndex(usize),
    /// Update selected handler (game list)
    SetSelectedHandler(usize),
    /// Update launch option selection
    SetLaunchOptionIndex(usize),
    /// Update dropdown selection
    SetDropdownSelection(usize),
    /// Navigate to a different page
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
        _ => vec![NavAction::None],
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
        ctx.instance_focus,
        direction,
        ctx.launch_option_index,
        max_options,
        ctx.instances_count > 0,
    );

    match result {
        InstancesNav::SwitchFocus(new_focus) => {
            let mut actions = vec![NavAction::SetInstanceFocus(new_focus)];
            if new_focus == InstanceFocus::LaunchOptions {
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

/// Process tab navigation (LB/RB buttons)
pub fn handle_tab(ctx: &NavContext, next: bool) -> NavAction {
    match ctx.page {
        MenuPage::Games => NavAction::ChangePage(MenuPage::Settings),
        MenuPage::Settings => NavAction::ChangePage(MenuPage::Games),
        _ => NavAction::None, // Don't tab from Instances
    }
}

/// Process back button (B)
pub fn handle_back(ctx: &NavContext, is_lite_mode: bool) -> NavAction {
    if ctx.dropdown_open {
        return NavAction::None; // Dropdown close handled separately
    }

    if ctx.page == MenuPage::Instances && ctx.instance_focus == InstanceFocus::LaunchOptions {
        return NavAction::SetInstanceFocus(InstanceFocus::Devices);
    }

    if is_lite_mode {
        NavAction::ChangePage(MenuPage::Instances)
    } else {
        NavAction::ChangePage(MenuPage::Games)
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
