//! Pipelines module (orchestration)

pub mod build_cmds;
pub mod execute;

pub use execute::launch_game;
