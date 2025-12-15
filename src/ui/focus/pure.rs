pub mod input_map;
pub mod navigation;
pub mod spatial;

// Re-exports
pub use input_map::{is_nav_button, map_button_to_nav};
pub use navigation::{
    apply_index_delta, navigate_dropdown, navigate_games_page, navigate_instances_page,
    GamesPaneNav, InstancesNav,
};
pub use spatial::{find_nearest_index, spatial_distance, Rect};
