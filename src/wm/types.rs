// Shared window manager types

/// Layout type for tiled window arrangement (shared between niri/hyprland)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutType {
    /// N separate columns (side by side)
    Columns,
    /// All windows stacked in one column
    Stacked,
    /// 2x2 grid (2 columns with 2 stacked each)
    Grid,
}

/// WM-agnostic monitor info
#[derive(Debug, Clone)]
pub struct WmMonitor {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// A column in a tiling layout, containing window indices
#[derive(Debug, Clone, PartialEq)]
pub struct TilingColumn {
    /// Window indices (spawn order) assigned to this column
    pub windows: Vec<usize>,
    /// Column width as a percentage of monitor width
    pub width_percent: u32,
}

/// A complete tiling plan: which windows go in which columns
#[derive(Debug, Clone, PartialEq)]
pub struct TilingPlan {
    pub columns: Vec<TilingColumn>,
}

/// Determine layout type from a preset ID string
pub fn get_layout_type(preset_id: &str) -> LayoutType {
    match preset_id {
        // Vertical = side-by-side columns
        "2p_vertical" | "3p_vertical" => LayoutType::Columns,
        // Horizontal = stacked in one column
        "2p_horizontal" | "3p_horizontal" => LayoutType::Stacked,
        // Grid = 2 columns with 2 stacked each
        "4p_grid" | "4p_rows" | "4p_columns" => LayoutType::Grid,
        _ => LayoutType::Columns, // Default fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_presets_return_columns() {
        assert_eq!(get_layout_type("2p_vertical"), LayoutType::Columns);
        assert_eq!(get_layout_type("3p_vertical"), LayoutType::Columns);
    }

    #[test]
    fn horizontal_presets_return_stacked() {
        assert_eq!(get_layout_type("2p_horizontal"), LayoutType::Stacked);
        assert_eq!(get_layout_type("3p_horizontal"), LayoutType::Stacked);
    }

    #[test]
    fn grid_presets_return_grid() {
        assert_eq!(get_layout_type("4p_grid"), LayoutType::Grid);
        assert_eq!(get_layout_type("4p_rows"), LayoutType::Grid);
        assert_eq!(get_layout_type("4p_columns"), LayoutType::Grid);
    }

    #[test]
    fn unknown_preset_returns_columns() {
        assert_eq!(get_layout_type("unknown_layout"), LayoutType::Columns);
    }

    #[test]
    fn empty_string_returns_columns() {
        assert_eq!(get_layout_type(""), LayoutType::Columns);
    }
}
