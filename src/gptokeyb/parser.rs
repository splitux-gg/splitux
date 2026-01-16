//! gptokeyb .gptk file parser and serializer
//!
//! The .gptk format is an INI-style file with two sections:
//! - [config] - Deadzone, mouse settings, etc.
//! - [controls] - Button-to-key mappings

use super::profile::{
    AnalogMode, ControllerButton, DeadzoneMode, GptokeybProfile, ProfileConfig,
};

/// Parse a .gptk file content into a GptokeybProfile
pub fn parse_gptk(content: &str, name: &str) -> Result<GptokeybProfile, String> {
    let mut profile = GptokeybProfile::new(name);
    let mut current_section = "";

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Section header
        if line.starts_with('[') && line.ends_with(']') {
            current_section = &line[1..line.len() - 1];
            continue;
        }

        // Key=value pair
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match current_section {
                "config" => parse_config_line(&mut profile.config, key, value),
                "controls" => parse_controls_line(&mut profile, key, value),
                _ => {} // Ignore unknown sections
            }
        }
    }

    Ok(profile)
}

/// Parse a config section line
fn parse_config_line(config: &mut ProfileConfig, key: &str, value: &str) {
    match key {
        "deadzone" => {
            if let Ok(v) = value.parse() {
                config.deadzone = v;
            }
        }
        "deadzone_mode" => {
            config.deadzone_mode = DeadzoneMode::from_str(value);
        }
        "deadzone_scale" => {
            if let Ok(v) = value.parse() {
                config.deadzone_scale = v;
            }
        }
        "mouse_scale" => {
            if let Ok(v) = value.parse() {
                config.mouse_scale = v;
            }
        }
        "mouse_delay" => {
            if let Ok(v) = value.parse() {
                config.mouse_delay = v;
            }
        }
        _ => {} // Ignore unknown config keys
    }
}

/// Parse a controls section line
fn parse_controls_line(profile: &mut GptokeybProfile, key: &str, value: &str) {
    // Handle analog stick mode assignments
    match key {
        "left_analog" => {
            if let Some(mode) = AnalogMode::from_gptk_value(value) {
                profile.left_analog_mode = mode;
            }
            return;
        }
        "right_analog" => {
            if let Some(mode) = AnalogMode::from_gptk_value(value) {
                profile.right_analog_mode = mode;
            }
            return;
        }
        _ => {}
    }

    // Handle button mappings
    if let Some(button) = ControllerButton::from_gptk_key(key) {
        profile.button_mappings.insert(button, value.to_string());
    }
}

/// Serialize a GptokeybProfile to .gptk format
pub fn serialize_gptk(profile: &GptokeybProfile) -> String {
    let mut output = String::new();

    // Header comment
    output.push_str(&format!(
        "# {} - Custom gptokeyb profile\n",
        profile.name
    ));
    output.push_str("# Created with Splitux Profile Builder\n\n");

    // Config section
    output.push_str("[config]\n");
    output.push_str(&format!("deadzone = {}\n", profile.config.deadzone));
    output.push_str(&format!(
        "deadzone_mode = {}\n",
        profile.config.deadzone_mode.as_str()
    ));
    output.push_str(&format!(
        "deadzone_scale = {}\n",
        profile.config.deadzone_scale
    ));
    output.push_str(&format!("mouse_scale = {}\n", profile.config.mouse_scale));
    output.push_str(&format!("mouse_delay = {}\n", profile.config.mouse_delay));
    output.push('\n');

    // Controls section
    output.push_str("[controls]\n");

    // Analog stick modes first
    if let Some(value) = profile.left_analog_mode.to_gptk_value() {
        output.push_str(&format!("left_analog = {}\n", value));
    }
    if let Some(value) = profile.right_analog_mode.to_gptk_value() {
        output.push_str(&format!("right_analog = {}\n", value));
    }

    // Group mappings by category for readability
    let button_groups = [
        ("# D-pad", &[
            ControllerButton::Up,
            ControllerButton::Down,
            ControllerButton::Left,
            ControllerButton::Right,
        ] as &[ControllerButton]),
        ("# Face buttons", &[
            ControllerButton::A,
            ControllerButton::B,
            ControllerButton::X,
            ControllerButton::Y,
        ]),
        ("# Shoulder buttons", &[
            ControllerButton::L1,
            ControllerButton::R1,
            ControllerButton::L2,
            ControllerButton::R2,
        ]),
        ("# Special buttons", &[
            ControllerButton::Start,
            ControllerButton::Back,
            ControllerButton::Guide,
        ]),
        ("# Stick buttons", &[
            ControllerButton::L3,
            ControllerButton::R3,
        ]),
        ("# Left analog directions", &[
            ControllerButton::LeftAnalogUp,
            ControllerButton::LeftAnalogDown,
            ControllerButton::LeftAnalogLeft,
            ControllerButton::LeftAnalogRight,
        ]),
        ("# Right analog directions", &[
            ControllerButton::RightAnalogUp,
            ControllerButton::RightAnalogDown,
            ControllerButton::RightAnalogLeft,
            ControllerButton::RightAnalogRight,
        ]),
    ];

    for (comment, buttons) in button_groups {
        let mut group_has_mappings = false;

        for button in buttons.iter() {
            if profile.button_mappings.contains_key(button) {
                if !group_has_mappings {
                    output.push_str(comment);
                    output.push('\n');
                    group_has_mappings = true;
                }
                let value = &profile.button_mappings[button];
                output.push_str(&format!("{} = {}\n", button.gptk_key(), value));
            }
        }

        if group_has_mappings {
            output.push('\n');
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fps_profile() {
        let content = r#"
[config]
deadzone = 2000
deadzone_mode = axial
mouse_scale = 48
mouse_delay = 16

[controls]
a = space
b = ctrl
right_analog = mouse_movement
left_analog_up = w
"#;

        let profile = parse_gptk(content, "test").unwrap();
        assert_eq!(profile.config.deadzone, 2000);
        assert_eq!(profile.config.mouse_scale, 48);
        assert_eq!(profile.right_analog_mode, AnalogMode::MouseMovement);
        assert_eq!(
            profile.button_mappings.get(&ControllerButton::A),
            Some(&"space".to_string())
        );
        assert_eq!(
            profile.button_mappings.get(&ControllerButton::LeftAnalogUp),
            Some(&"w".to_string())
        );
    }

    #[test]
    fn test_roundtrip() {
        let mut profile = GptokeybProfile::new("test");
        profile.config.deadzone = 3000;
        profile.right_analog_mode = AnalogMode::MouseMovement;
        profile.set_mapping(ControllerButton::A, "space");
        profile.set_mapping(ControllerButton::B, "ctrl");

        let serialized = serialize_gptk(&profile);
        let parsed = parse_gptk(&serialized, "test").unwrap();

        assert_eq!(parsed.config.deadzone, 3000);
        assert_eq!(parsed.right_analog_mode, AnalogMode::MouseMovement);
        assert_eq!(parsed.get_mapping(ControllerButton::A), Some("space"));
        assert_eq!(parsed.get_mapping(ControllerButton::B), Some("ctrl"));
    }
}
