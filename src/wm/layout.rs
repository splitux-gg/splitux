//! Shared layout calculation logic for splitscreen window positioning.

use super::presets::LayoutPreset;

/// Represents the window geometry for a game instance
#[derive(Debug, Clone)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Calculate window geometry using a layout preset
pub fn calculate_geometry_from_preset(
    preset: &LayoutPreset,
    player_index: usize,
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
) -> WindowGeometry {
    let index = player_index.min(preset.regions.len().saturating_sub(1));
    let region = preset.regions[index];

    WindowGeometry {
        x: monitor_x + (region[0] * monitor_width as f32) as i32,
        y: monitor_y + (region[1] * monitor_height as f32) as i32,
        width: (region[2] * monitor_width as f32) as u32,
        height: (region[3] * monitor_height as f32) as u32,
    }
}
