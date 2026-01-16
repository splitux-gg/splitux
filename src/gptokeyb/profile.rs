//! gptokeyb profile data model
//!
//! Represents the configuration for controller→keyboard/mouse translation
//! profiles that can be created in the Profile Builder UI.

use std::collections::HashMap;

/// Controller button identifiers matching gptokeyb's expected keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControllerButton {
    // D-pad
    Up,
    Down,
    Left,
    Right,
    // Face buttons
    A,
    B,
    X,
    Y,
    // Shoulder buttons
    L1,
    R1,
    L2,
    R2,
    // Stick buttons
    L3,
    R3,
    // Special buttons
    Start,
    Back,
    Guide,
    // Left analog directional
    LeftAnalogUp,
    LeftAnalogDown,
    LeftAnalogLeft,
    LeftAnalogRight,
    // Right analog directional
    RightAnalogUp,
    RightAnalogDown,
    RightAnalogLeft,
    RightAnalogRight,
}

impl ControllerButton {
    /// Get the gptk file key name for this button
    pub fn gptk_key(&self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
            Self::A => "a",
            Self::B => "b",
            Self::X => "x",
            Self::Y => "y",
            Self::L1 => "l1",
            Self::R1 => "r1",
            Self::L2 => "l2",
            Self::R2 => "r2",
            Self::L3 => "l3",
            Self::R3 => "r3",
            Self::Start => "start",
            Self::Back => "back",
            Self::Guide => "guide",
            Self::LeftAnalogUp => "left_analog_up",
            Self::LeftAnalogDown => "left_analog_down",
            Self::LeftAnalogLeft => "left_analog_left",
            Self::LeftAnalogRight => "left_analog_right",
            Self::RightAnalogUp => "right_analog_up",
            Self::RightAnalogDown => "right_analog_down",
            Self::RightAnalogLeft => "right_analog_left",
            Self::RightAnalogRight => "right_analog_right",
        }
    }

    /// Parse from gptk file key name
    pub fn from_gptk_key(key: &str) -> Option<Self> {
        match key {
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "a" => Some(Self::A),
            "b" => Some(Self::B),
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "l1" => Some(Self::L1),
            "r1" => Some(Self::R1),
            "l2" | "lt" => Some(Self::L2),
            "r2" | "rt" => Some(Self::R2),
            "l3" => Some(Self::L3),
            "r3" => Some(Self::R3),
            "start" => Some(Self::Start),
            "back" => Some(Self::Back),
            "guide" => Some(Self::Guide),
            "left_analog_up" => Some(Self::LeftAnalogUp),
            "left_analog_down" => Some(Self::LeftAnalogDown),
            "left_analog_left" => Some(Self::LeftAnalogLeft),
            "left_analog_right" => Some(Self::LeftAnalogRight),
            "right_analog_up" => Some(Self::RightAnalogUp),
            "right_analog_down" => Some(Self::RightAnalogDown),
            "right_analog_left" => Some(Self::RightAnalogLeft),
            "right_analog_right" => Some(Self::RightAnalogRight),
            _ => None,
        }
    }

    /// User-friendly display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Up => "D-Pad Up",
            Self::Down => "D-Pad Down",
            Self::Left => "D-Pad Left",
            Self::Right => "D-Pad Right",
            Self::A => "A Button",
            Self::B => "B Button",
            Self::X => "X Button",
            Self::Y => "Y Button",
            Self::L1 => "Left Bumper (LB)",
            Self::R1 => "Right Bumper (RB)",
            Self::L2 => "Left Trigger (LT)",
            Self::R2 => "Right Trigger (RT)",
            Self::L3 => "Left Stick Click (L3)",
            Self::R3 => "Right Stick Click (R3)",
            Self::Start => "Start / Menu",
            Self::Back => "Back / View",
            Self::Guide => "Guide / Xbox",
            Self::LeftAnalogUp => "Left Stick Up",
            Self::LeftAnalogDown => "Left Stick Down",
            Self::LeftAnalogLeft => "Left Stick Left",
            Self::LeftAnalogRight => "Left Stick Right",
            Self::RightAnalogUp => "Right Stick Up",
            Self::RightAnalogDown => "Right Stick Down",
            Self::RightAnalogLeft => "Right Stick Left",
            Self::RightAnalogRight => "Right Stick Right",
        }
    }
}

/// Analog stick mode for left_analog and right_analog keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalogMode {
    /// Analog stick disabled (individual directions mapped separately)
    #[default]
    Disabled,
    /// Analog stick controls mouse movement
    MouseMovement,
}

impl AnalogMode {
    /// Parse from gptk value
    pub fn from_gptk_value(value: &str) -> Option<Self> {
        match value {
            "mouse_movement" => Some(Self::MouseMovement),
            _ => None,
        }
    }

    /// Convert to gptk value
    pub fn to_gptk_value(&self) -> Option<&'static str> {
        match self {
            Self::Disabled => None,
            Self::MouseMovement => Some("mouse_movement"),
        }
    }
}

/// Deadzone calculation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeadzoneMode {
    /// Axial deadzone (per-axis)
    #[default]
    Axial,
    /// Radial deadzone (circular)
    Radial,
}

impl DeadzoneMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "radial" => Self::Radial,
            _ => Self::Axial,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Axial => "axial",
            Self::Radial => "radial",
        }
    }
}

/// Configuration section of a gptokeyb profile
#[derive(Debug, Clone)]
pub struct ProfileConfig {
    /// Analog stick deadzone (0-32767, default: 2000)
    pub deadzone: u32,
    /// Deadzone mode: axial or radial
    pub deadzone_mode: DeadzoneMode,
    /// Deadzone scale factor (default: 7)
    pub deadzone_scale: u32,
    /// Mouse movement speed multiplier (default: 48)
    pub mouse_scale: u32,
    /// Mouse update delay in ms (default: 16 = ~60fps)
    pub mouse_delay: u32,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            deadzone: 2000,
            deadzone_mode: DeadzoneMode::Axial,
            deadzone_scale: 7,
            mouse_scale: 48,
            mouse_delay: 16,
        }
    }
}

/// A complete gptokeyb profile with mappings
#[derive(Debug, Clone)]
pub struct GptokeybProfile {
    /// Profile name (used for filename: {name}.gptk)
    pub name: String,
    /// Configuration settings
    pub config: ProfileConfig,
    /// Button-to-key mappings (ControllerButton -> keyboard key/mouse action string)
    pub button_mappings: HashMap<ControllerButton, String>,
    /// Left analog stick mode (if set, overrides individual left analog directions)
    pub left_analog_mode: AnalogMode,
    /// Right analog stick mode (if set, overrides individual right analog directions)
    pub right_analog_mode: AnalogMode,
}

impl Default for GptokeybProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            config: ProfileConfig::default(),
            button_mappings: HashMap::new(),
            left_analog_mode: AnalogMode::Disabled,
            right_analog_mode: AnalogMode::Disabled,
        }
    }
}

impl GptokeybProfile {
    /// Create a new empty profile with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Get the mapped action for a button (if any)
    pub fn get_mapping(&self, button: ControllerButton) -> Option<&str> {
        self.button_mappings.get(&button).map(|s| s.as_str())
    }

    /// Set a button mapping
    pub fn set_mapping(&mut self, button: ControllerButton, action: impl Into<String>) {
        self.button_mappings.insert(button, action.into());
    }

    /// Remove a button mapping
    pub fn clear_mapping(&mut self, button: ControllerButton) {
        self.button_mappings.remove(&button);
    }
}
