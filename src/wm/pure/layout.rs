// Pure layout planning — deterministic, no I/O

use crate::wm::types::{get_layout_type, LayoutType, TilingColumn, TilingPlan};

/// Plan a tiling layout given a preset ID and window count.
///
/// Returns a `TilingPlan` describing how windows should be arranged into columns.
/// Each column has a list of window indices (spawn order) and a width percentage.
pub fn plan_tiling_layout(preset_id: &str, window_count: usize) -> TilingPlan {
    let layout_type = get_layout_type(preset_id);

    match layout_type {
        LayoutType::Columns => {
            // Each window in its own column with equal width
            let width = 100 / window_count as u32;
            let columns = (0..window_count)
                .map(|i| TilingColumn {
                    windows: vec![i],
                    width_percent: width,
                })
                .collect();
            TilingPlan { columns }
        }

        LayoutType::Stacked => {
            // All windows in a single column
            let windows = (0..window_count).collect();
            TilingPlan {
                columns: vec![TilingColumn {
                    windows,
                    width_percent: 100,
                }],
            }
        }

        LayoutType::Grid => {
            // 2x2 grid: 2 columns with 2 stacked windows each
            // Window ordering depends on preset
            let order: [usize; 4] = match preset_id {
                // 4p_columns: P1/P2 left column, P3/P4 right column
                "4p_columns" => [0, 1, 2, 3],
                // 4p_grid/4p_rows: P1/P3 left column (top/bottom), P2/P4 right column
                _ => [0, 2, 1, 3],
            };

            TilingPlan {
                columns: vec![
                    TilingColumn {
                        windows: vec![order[0], order[1]],
                        width_percent: 50,
                    },
                    TilingColumn {
                        windows: vec![order[2], order[3]],
                        width_percent: 50,
                    },
                ],
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_columns_layout() {
        let plan = plan_tiling_layout("2p_vertical", 2);
        assert_eq!(plan.columns.len(), 2);
        assert_eq!(plan.columns[0].windows, vec![0]);
        assert_eq!(plan.columns[1].windows, vec![1]);
        assert_eq!(plan.columns[0].width_percent, 50);
    }

    #[test]
    fn test_stacked_layout() {
        let plan = plan_tiling_layout("2p_horizontal", 2);
        assert_eq!(plan.columns.len(), 1);
        assert_eq!(plan.columns[0].windows, vec![0, 1]);
        assert_eq!(plan.columns[0].width_percent, 100);
    }

    #[test]
    fn test_grid_default() {
        let plan = plan_tiling_layout("4p_grid", 4);
        assert_eq!(plan.columns.len(), 2);
        // Default grid: P1/P3 left, P2/P4 right
        assert_eq!(plan.columns[0].windows, vec![0, 2]);
        assert_eq!(plan.columns[1].windows, vec![1, 3]);
    }

    #[test]
    fn test_grid_columns_preset() {
        let plan = plan_tiling_layout("4p_columns", 4);
        assert_eq!(plan.columns.len(), 2);
        // 4p_columns: P1/P2 left, P3/P4 right
        assert_eq!(plan.columns[0].windows, vec![0, 1]);
        assert_eq!(plan.columns[1].windows, vec![2, 3]);
    }

    #[test]
    fn test_three_columns() {
        let plan = plan_tiling_layout("3p_vertical", 3);
        assert_eq!(plan.columns.len(), 3);
        assert_eq!(plan.columns[0].width_percent, 33);
    }
}
