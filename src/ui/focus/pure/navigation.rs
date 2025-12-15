// Within-region navigation logic (pure functions)

use crate::ui::focus::types::{FocusPane, InstanceFocus, NavDirection};

/// Result of navigating within the Games page
#[derive(Debug, Clone, PartialEq)]
pub enum GamesPaneNav {
    /// Stay in current pane, move selection by delta
    MoveWithin { delta: i32 },
    /// Switch to a different pane
    SwitchPane(FocusPane),
    /// No navigation (at boundary or blocked)
    None,
}

/// Navigate within the Games page pane system
///
/// Returns the navigation result without mutating state (pure function)
pub fn navigate_games_page(
    current_pane: FocusPane,
    direction: NavDirection,
    action_bar_index: usize,
    action_bar_max: usize,
) -> GamesPaneNav {
    match current_pane {
        FocusPane::GameList => match direction {
            NavDirection::Up => GamesPaneNav::MoveWithin { delta: -1 },
            NavDirection::Down => GamesPaneNav::MoveWithin { delta: 1 },
            NavDirection::Left => GamesPaneNav::None, // Already at leftmost
            NavDirection::Right => GamesPaneNav::SwitchPane(FocusPane::ActionBar),
        },
        FocusPane::ActionBar => match direction {
            NavDirection::Up | NavDirection::Down => GamesPaneNav::None, // Horizontal bar
            NavDirection::Left => {
                if action_bar_index > 0 {
                    GamesPaneNav::MoveWithin { delta: -1 }
                } else {
                    GamesPaneNav::SwitchPane(FocusPane::GameList)
                }
            }
            NavDirection::Right => {
                if action_bar_index < action_bar_max - 1 {
                    GamesPaneNav::MoveWithin { delta: 1 }
                } else {
                    GamesPaneNav::SwitchPane(FocusPane::InfoPane)
                }
            }
        },
        FocusPane::InfoPane => match direction {
            NavDirection::Up => GamesPaneNav::MoveWithin { delta: -1 },
            NavDirection::Down => GamesPaneNav::MoveWithin { delta: 1 },
            NavDirection::Left => GamesPaneNav::SwitchPane(FocusPane::ActionBar),
            NavDirection::Right => GamesPaneNav::None, // Already at rightmost
        },
    }
}

/// Navigate within the Instances page
pub fn navigate_instances_page(
    current_focus: InstanceFocus,
    direction: NavDirection,
    launch_option_index: usize,
    launch_option_max: usize,
    has_instances: bool,
) -> InstancesNav {
    match current_focus {
        InstanceFocus::Devices => match direction {
            NavDirection::Down if has_instances => InstancesNav::SwitchFocus(InstanceFocus::LaunchOptions),
            _ => InstancesNav::None,
        },
        InstanceFocus::LaunchOptions => match direction {
            NavDirection::Up => InstancesNav::SwitchFocus(InstanceFocus::Devices),
            NavDirection::Left => {
                if launch_option_index > 0 {
                    InstancesNav::MoveWithin { delta: -1 }
                } else {
                    InstancesNav::None
                }
            }
            NavDirection::Right => {
                if launch_option_index < launch_option_max - 1 {
                    InstancesNav::MoveWithin { delta: 1 }
                } else {
                    InstancesNav::None
                }
            }
            NavDirection::Down => InstancesNav::None,
        },
    }
}

/// Result of navigating within the Instances page
#[derive(Debug, Clone, PartialEq)]
pub enum InstancesNav {
    /// Move selection within current focus area
    MoveWithin { delta: i32 },
    /// Switch focus between Devices and LaunchOptions
    SwitchFocus(InstanceFocus),
    /// No navigation
    None,
}

/// Navigate dropdown selection (wrapping)
pub fn navigate_dropdown(current: usize, total: usize, direction: NavDirection) -> usize {
    match direction {
        NavDirection::Up => {
            if current == 0 {
                total.saturating_sub(1)
            } else {
                current - 1
            }
        }
        NavDirection::Down => (current + 1) % total,
        _ => current, // Left/Right do nothing in dropdown
    }
}

/// Clamp an index after applying a delta
pub fn apply_index_delta(current: usize, delta: i32, max: usize) -> usize {
    if delta < 0 {
        current.saturating_sub((-delta) as usize)
    } else {
        (current + delta as usize).min(max.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_list_navigation() {
        // Moving right from GameList should switch to ActionBar
        assert_eq!(
            navigate_games_page(FocusPane::GameList, NavDirection::Right, 0, 3),
            GamesPaneNav::SwitchPane(FocusPane::ActionBar)
        );

        // Moving left from GameList should do nothing
        assert_eq!(
            navigate_games_page(FocusPane::GameList, NavDirection::Left, 0, 3),
            GamesPaneNav::None
        );
    }

    #[test]
    fn test_action_bar_navigation() {
        // At index 0, left should switch to GameList
        assert_eq!(
            navigate_games_page(FocusPane::ActionBar, NavDirection::Left, 0, 3),
            GamesPaneNav::SwitchPane(FocusPane::GameList)
        );

        // At index 1, left should move within
        assert_eq!(
            navigate_games_page(FocusPane::ActionBar, NavDirection::Left, 1, 3),
            GamesPaneNav::MoveWithin { delta: -1 }
        );

        // At max index, right should switch to InfoPane
        assert_eq!(
            navigate_games_page(FocusPane::ActionBar, NavDirection::Right, 2, 3),
            GamesPaneNav::SwitchPane(FocusPane::InfoPane)
        );
    }

    #[test]
    fn test_dropdown_wrap() {
        assert_eq!(navigate_dropdown(0, 5, NavDirection::Up), 4); // Wrap to end
        assert_eq!(navigate_dropdown(4, 5, NavDirection::Down), 0); // Wrap to start
        assert_eq!(navigate_dropdown(2, 5, NavDirection::Up), 1); // Normal up
    }

    #[test]
    fn test_apply_delta() {
        assert_eq!(apply_index_delta(5, -1, 10), 4);
        assert_eq!(apply_index_delta(0, -1, 10), 0); // Clamp at 0
        assert_eq!(apply_index_delta(8, 5, 10), 9); // Clamp at max-1
    }
}
