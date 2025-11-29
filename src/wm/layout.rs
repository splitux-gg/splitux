//! Shared layout calculation logic, ported from KWin JavaScript scripts.

use crate::instance::Instance;
use crate::monitor::Monitor;

/// Represents the window geometry for a game instance
#[derive(Debug, Clone)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutOrientation {
    /// Horizontal split for 2 players (top/bottom)
    Horizontal,
    /// Vertical split for 2 players (left/right)
    Vertical,
}

/// Layout tables for horizontal orientation (matching splitscreen_kwin.js)
/// Index by [player_count][player_index]
const X_HORIZONTAL: [[f32; 4]; 5] = [
    [0.0, 0.0, 0.0, 0.0],       // 0 players (unused)
    [0.0, 0.0, 0.0, 0.0],       // 1 player
    [0.0, 0.0, 0.0, 0.0],       // 2 players (top/bottom)
    [0.0, 0.0, 0.5, 0.0],       // 3 players
    [0.0, 0.5, 0.0, 0.5],       // 4 players
];

const Y_HORIZONTAL: [[f32; 4]; 5] = [
    [0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0],
    [0.0, 0.5, 0.0, 0.0],       // 2 players (top/bottom)
    [0.0, 0.5, 0.5, 0.0],
    [0.0, 0.0, 0.5, 0.5],
];

const WIDTH_HORIZONTAL: [[f32; 4]; 5] = [
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],       // 2 players (full width each)
    [1.0, 0.5, 0.5, 1.0],
    [0.5, 0.5, 0.5, 0.5],
];

const HEIGHT_HORIZONTAL: [[f32; 4]; 5] = [
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [0.5, 0.5, 1.0, 1.0],       // 2 players (half height each)
    [0.5, 0.5, 0.5, 1.0],
    [0.5, 0.5, 0.5, 0.5],
];

/// Layout tables for vertical orientation (matching splitscreen_kwin_vertical.js)
const X_VERTICAL: [[f32; 4]; 5] = [
    [0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0],
    [0.0, 0.5, 0.0, 0.0],       // 2 players (left/right)
    [0.0, 0.0, 0.5, 0.0],
    [0.0, 0.5, 0.0, 0.5],
];

const Y_VERTICAL: [[f32; 4]; 5] = [
    [0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0],       // 2 players (left/right, same y)
    [0.0, 0.5, 0.5, 0.0],
    [0.0, 0.0, 0.5, 0.5],
];

const WIDTH_VERTICAL: [[f32; 4]; 5] = [
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [0.5, 0.5, 1.0, 1.0],       // 2 players (half width each)
    [1.0, 0.5, 0.5, 1.0],
    [0.5, 0.5, 0.5, 0.5],
];

const HEIGHT_VERTICAL: [[f32; 4]; 5] = [
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],       // 2 players (full height each)
    [0.5, 0.5, 0.5, 1.0],
    [0.5, 0.5, 0.5, 0.5],
];

/// Calculate window geometry for a player within a monitor
pub fn calculate_geometry(
    player_count: usize,
    player_index: usize,
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
    orientation: LayoutOrientation,
) -> WindowGeometry {
    let count = player_count.min(4);
    let index = player_index.min(3);

    let (x_table, y_table, w_table, h_table) = match orientation {
        LayoutOrientation::Horizontal => (X_HORIZONTAL, Y_HORIZONTAL, WIDTH_HORIZONTAL, HEIGHT_HORIZONTAL),
        LayoutOrientation::Vertical => (X_VERTICAL, Y_VERTICAL, WIDTH_VERTICAL, HEIGHT_VERTICAL),
    };

    WindowGeometry {
        x: monitor_x + (x_table[count][index] * monitor_width as f32) as i32,
        y: monitor_y + (y_table[count][index] * monitor_height as f32) as i32,
        width: (w_table[count][index] * monitor_width as f32) as u32,
        height: (h_table[count][index] * monitor_height as f32) as u32,
    }
}

/// Calculate all window geometries for a set of instances
#[allow(dead_code)] // Exported for future WM backends that need centralized layout
pub fn calculate_all_geometries(
    instances: &[Instance],
    monitors: &[Monitor],
    orientation: LayoutOrientation,
) -> Vec<WindowGeometry> {
    if monitors.is_empty() {
        return Vec::new();
    }

    // Group instances by monitor
    let mut monitor_counts: Vec<usize> = vec![0; monitors.len()];
    for instance in instances {
        let mon = instance.monitor.min(monitors.len() - 1);
        monitor_counts[mon] += 1;
    }

    let mut monitor_indices: Vec<usize> = vec![0; monitors.len()];
    let mut geometries = Vec::with_capacity(instances.len());

    // Calculate cumulative monitor X positions (assumes horizontal arrangement)
    let mut monitor_x_positions: Vec<i32> = Vec::with_capacity(monitors.len());
    let mut cumulative_x = 0i32;
    for monitor in monitors {
        monitor_x_positions.push(cumulative_x);
        cumulative_x += monitor.width() as i32;
    }

    for instance in instances {
        let mon_idx = instance.monitor.min(monitors.len() - 1);
        let monitor = &monitors[mon_idx];
        let player_count = monitor_counts[mon_idx];
        let player_index = monitor_indices[mon_idx];
        monitor_indices[mon_idx] += 1;

        let monitor_x = monitor_x_positions[mon_idx];

        geometries.push(calculate_geometry(
            player_count,
            player_index,
            monitor_x,
            0,
            monitor.width(),
            monitor.height(),
            orientation,
        ));
    }

    geometries
}
