//! Layout preset definitions for splitscreen configurations
//!
//! Each preset defines normalized regions (0.0-1.0) for player positions.
//! Regions are [x, y, width, height] in normalized screen coordinates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A layout preset defines where each player's window is positioned
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutPreset {
    /// Unique ID for serialization (e.g., "2p_horizontal", "3p_t_shape")
    pub id: &'static str,
    /// Human-readable name (e.g., "Top/Bottom", "Side by Side")
    pub name: &'static str,
    /// Player count this preset applies to
    pub player_count: usize,
    /// Normalized rectangles for each player [x, y, w, h] in 0.0-1.0 range
    pub regions: &'static [[f32; 4]],
}

// ============================================================================
// 2-Player Presets
// ============================================================================

/// P1 top half, P2 bottom half
pub static PRESET_2P_HORIZONTAL: LayoutPreset = LayoutPreset {
    id: "2p_horizontal",
    name: "Top / Bottom",
    player_count: 2,
    regions: &[
        [0.0, 0.0, 1.0, 0.5], // P1: top half
        [0.0, 0.5, 1.0, 0.5], // P2: bottom half
    ],
};

/// P1 left half, P2 right half
pub static PRESET_2P_VERTICAL: LayoutPreset = LayoutPreset {
    id: "2p_vertical",
    name: "Side by Side",
    player_count: 2,
    regions: &[
        [0.0, 0.0, 0.5, 1.0], // P1: left half
        [0.5, 0.0, 0.5, 1.0], // P2: right half
    ],
};

// ============================================================================
// 3-Player Presets
// ============================================================================

/// P1 full top, P2/P3 bottom halves (T-shape)
pub static PRESET_3P_T_SHAPE: LayoutPreset = LayoutPreset {
    id: "3p_t_shape",
    name: "T-Shape",
    player_count: 3,
    regions: &[
        [0.0, 0.0, 1.0, 0.5], // P1: full top
        [0.0, 0.5, 0.5, 0.5], // P2: bottom-left
        [0.5, 0.5, 0.5, 0.5], // P3: bottom-right
    ],
};

/// P1/P2 top halves, P3 full bottom (Inverted T)
pub static PRESET_3P_INVERTED_T: LayoutPreset = LayoutPreset {
    id: "3p_inverted_t",
    name: "Inverted T",
    player_count: 3,
    regions: &[
        [0.0, 0.0, 0.5, 0.5], // P1: top-left
        [0.5, 0.0, 0.5, 0.5], // P2: top-right
        [0.0, 0.5, 1.0, 0.5], // P3: full bottom
    ],
};

/// P1 left half, P2/P3 stacked on right
pub static PRESET_3P_LEFT_MAIN: LayoutPreset = LayoutPreset {
    id: "3p_left_main",
    name: "Left Main",
    player_count: 3,
    regions: &[
        [0.0, 0.0, 0.5, 1.0], // P1: left half
        [0.5, 0.0, 0.5, 0.5], // P2: top-right
        [0.5, 0.5, 0.5, 0.5], // P3: bottom-right
    ],
};

/// P1/P2 stacked on left, P3 right half
pub static PRESET_3P_RIGHT_MAIN: LayoutPreset = LayoutPreset {
    id: "3p_right_main",
    name: "Right Main",
    player_count: 3,
    regions: &[
        [0.0, 0.0, 0.5, 0.5], // P1: top-left
        [0.0, 0.5, 0.5, 0.5], // P2: bottom-left
        [0.5, 0.0, 0.5, 1.0], // P3: right half
    ],
};

// ============================================================================
// 4-Player Presets
// ============================================================================

/// Standard 2x2 grid (P1 top-left, P2 top-right, P3 bottom-left, P4 bottom-right)
pub static PRESET_4P_GRID: LayoutPreset = LayoutPreset {
    id: "4p_grid",
    name: "Grid",
    player_count: 4,
    regions: &[
        [0.0, 0.0, 0.5, 0.5], // P1: top-left
        [0.5, 0.0, 0.5, 0.5], // P2: top-right
        [0.0, 0.5, 0.5, 0.5], // P3: bottom-left
        [0.5, 0.5, 0.5, 0.5], // P4: bottom-right
    ],
};

/// Rows: P1/P2 on top row, P3/P4 on bottom row (reads left-to-right, top-to-bottom)
pub static PRESET_4P_ROWS: LayoutPreset = LayoutPreset {
    id: "4p_rows",
    name: "Rows",
    player_count: 4,
    regions: &[
        [0.0, 0.0, 0.5, 0.5], // P1: top-left
        [0.5, 0.0, 0.5, 0.5], // P2: top-right
        [0.0, 0.5, 0.5, 0.5], // P3: bottom-left
        [0.5, 0.5, 0.5, 0.5], // P4: bottom-right
    ],
};

/// Columns: P1/P2 on left column, P3/P4 on right column
pub static PRESET_4P_COLUMNS: LayoutPreset = LayoutPreset {
    id: "4p_columns",
    name: "Columns",
    player_count: 4,
    regions: &[
        [0.0, 0.0, 0.5, 0.5], // P1: top-left
        [0.0, 0.5, 0.5, 0.5], // P2: bottom-left
        [0.5, 0.0, 0.5, 0.5], // P3: top-right
        [0.5, 0.5, 0.5, 0.5], // P4: bottom-right
    ],
};

/// P1 gets 50% (left half), P2/P3/P4 share the right half in thirds
pub static PRESET_4P_MAIN_PLUS_3: LayoutPreset = LayoutPreset {
    id: "4p_main_plus_3",
    name: "Main + 3",
    player_count: 4,
    regions: &[
        [0.0, 0.0, 0.5, 1.0],          // P1: left half (main)
        [0.5, 0.0, 0.5, 1.0 / 3.0],    // P2: top-right third
        [0.5, 1.0 / 3.0, 0.5, 1.0 / 3.0], // P3: middle-right third
        [0.5, 2.0 / 3.0, 0.5, 1.0 / 3.0], // P4: bottom-right third
    ],
};

// ============================================================================
// Preset Registry
// ============================================================================

/// All 2-player presets
pub static PRESETS_2P: &[&LayoutPreset] = &[
    &PRESET_2P_HORIZONTAL,
    &PRESET_2P_VERTICAL,
];

/// All 3-player presets
pub static PRESETS_3P: &[&LayoutPreset] = &[
    &PRESET_3P_T_SHAPE,
    &PRESET_3P_INVERTED_T,
    &PRESET_3P_LEFT_MAIN,
    &PRESET_3P_RIGHT_MAIN,
];

/// All 4-player presets
pub static PRESETS_4P: &[&LayoutPreset] = &[
    &PRESET_4P_GRID,
    &PRESET_4P_ROWS,
    &PRESET_4P_COLUMNS,
    &PRESET_4P_MAIN_PLUS_3,
];

/// Get all presets for a given player count
pub fn get_presets_for_count(player_count: usize) -> &'static [&'static LayoutPreset] {
    match player_count {
        2 => PRESETS_2P,
        3 => PRESETS_3P,
        4 => PRESETS_4P,
        _ => &[],
    }
}

/// Get a specific preset by ID
pub fn get_preset_by_id(id: &str) -> Option<&'static LayoutPreset> {
    // Check all preset arrays
    for preset in PRESETS_2P.iter().chain(PRESETS_3P.iter()).chain(PRESETS_4P.iter()) {
        if preset.id == id {
            return Some(preset);
        }
    }
    None
}


/// Selected layout presets per player count, stored in config
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LayoutPresets {
    /// Preset ID for 2-player layout
    #[serde(default = "default_2p")]
    pub two_player: String,
    /// Preset ID for 3-player layout
    #[serde(default = "default_3p")]
    pub three_player: String,
    /// Preset ID for 4-player layout
    #[serde(default = "default_4p")]
    pub four_player: String,
    /// Custom instance-to-region mappings per preset
    /// Key: preset_id, Value: instance order (index=region, value=instance_idx)
    #[serde(default)]
    pub custom_orders: HashMap<String, Vec<usize>>,
}

fn default_2p() -> String {
    "2p_horizontal".to_string()
}

fn default_3p() -> String {
    "3p_t_shape".to_string()
}

fn default_4p() -> String {
    "4p_grid".to_string()
}

impl LayoutPresets {
    /// Get the preset ID for a given player count
    pub fn get_for_count(&self, player_count: usize) -> &str {
        match player_count {
            2 => &self.two_player,
            3 => &self.three_player,
            4 => &self.four_player,
            _ => "2p_horizontal",
        }
    }

    /// Set the preset ID for a given player count
    pub fn set_for_count(&mut self, player_count: usize, preset_id: String) {
        match player_count {
            2 => self.two_player = preset_id,
            3 => self.three_player = preset_id,
            4 => self.four_player = preset_id,
            _ => {}
        }
    }

    /// Get the current preset index for a given player count
    pub fn get_index_for_count(&self, player_count: usize) -> usize {
        let presets = get_presets_for_count(player_count);
        let current_id = self.get_for_count(player_count);
        presets
            .iter()
            .position(|p| p.id == current_id)
            .unwrap_or(0)
    }

    /// Cycle to the next preset for a given player count
    pub fn cycle_next(&mut self, player_count: usize) {
        let presets = get_presets_for_count(player_count);
        if presets.is_empty() {
            return;
        }
        let current_idx = self.get_index_for_count(player_count);
        let next_idx = (current_idx + 1) % presets.len();
        self.set_for_count(player_count, presets[next_idx].id.to_string());
    }

    /// Cycle to the previous preset for a given player count
    pub fn cycle_prev(&mut self, player_count: usize) {
        let presets = get_presets_for_count(player_count);
        if presets.is_empty() {
            return;
        }
        let current_idx = self.get_index_for_count(player_count);
        let prev_idx = if current_idx == 0 {
            presets.len() - 1
        } else {
            current_idx - 1
        };
        self.set_for_count(player_count, presets[prev_idx].id.to_string());
    }

    /// Get the instance order for a preset (custom or default sequential)
    pub fn get_order(&self, preset_id: &str, count: usize) -> Vec<usize> {
        if let Some(order) = self.custom_orders.get(preset_id) {
            // Validate the order has correct length and valid indices
            if order.len() == count && order.iter().all(|&i| i < count) {
                return order.clone();
            }
        }
        // Default: sequential order [0, 1, 2, ...]
        (0..count).collect()
    }

    /// Set a custom instance order for a preset
    pub fn set_order(&mut self, preset_id: &str, order: Vec<usize>) {
        self.custom_orders.insert(preset_id.to_string(), order);
    }

    /// Check if a preset has a custom order defined
    pub fn has_custom_order(&self, preset_id: &str) -> bool {
        self.custom_orders.contains_key(preset_id)
    }

}
