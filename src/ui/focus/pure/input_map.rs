// Input mapping from PadButton to NavInput

use crate::input::PadButton;
use crate::ui::focus::types::{NavDirection, NavInput};

/// Map a gamepad button to a navigation input
pub fn map_button_to_nav(button: PadButton) -> Option<NavInput> {
    match button {
        // D-pad → Directional navigation
        PadButton::Up => Some(NavInput::Direction(NavDirection::Up)),
        PadButton::Down => Some(NavInput::Direction(NavDirection::Down)),
        PadButton::Left => Some(NavInput::Direction(NavDirection::Left)),
        PadButton::Right => Some(NavInput::Direction(NavDirection::Right)),

        // Face buttons
        PadButton::ABtn => Some(NavInput::Accept),
        PadButton::BBtn => Some(NavInput::Back),

        // Shoulder buttons → Tab navigation
        PadButton::LB => Some(NavInput::TabPrev),
        PadButton::RB => Some(NavInput::TabNext),

        // Not navigation inputs
        _ => None,
    }
}

/// Check if a button is a navigation-related input
pub fn is_nav_button(button: PadButton) -> bool {
    map_button_to_nav(button).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpad_mapping() {
        assert_eq!(
            map_button_to_nav(PadButton::Up),
            Some(NavInput::Direction(NavDirection::Up))
        );
        assert_eq!(
            map_button_to_nav(PadButton::Down),
            Some(NavInput::Direction(NavDirection::Down))
        );
    }

    #[test]
    fn test_face_buttons() {
        assert_eq!(map_button_to_nav(PadButton::ABtn), Some(NavInput::Accept));
        assert_eq!(map_button_to_nav(PadButton::BBtn), Some(NavInput::Back));
    }

    #[test]
    fn test_non_nav_buttons() {
        assert_eq!(map_button_to_nav(PadButton::StartBtn), None);
        assert_eq!(map_button_to_nav(PadButton::SelectBtn), None);
    }
}
