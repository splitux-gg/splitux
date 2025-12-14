//! Pipelines module (orchestration)

pub mod build_cmds;
pub mod execute;

pub use build_cmds::{launch_cmds, print_launch_cmds};
pub use execute::launch_game;
