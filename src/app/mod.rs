mod app;
mod input;
mod app_instances;
mod app_launch;
mod app_panels;
mod app_profiles;
mod pages_edit_handler;
mod pages_games;
mod pages_instances;
mod pages_registry;
mod pages_settings;

pub use app::Splitux;
pub use crate::config::{load_photon_ids, PadFilterType, SplituxConfig, WindowManagerType};
