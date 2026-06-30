//! Pipelines module (orchestration)

pub mod build_cmds;
pub mod execute;
pub mod session;

pub use execute::launch_game;
pub use session::run_launch;
