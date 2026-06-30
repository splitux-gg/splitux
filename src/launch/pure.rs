//! Pure functions module (no side effects)

pub mod command;
pub mod numbering;
pub mod validation;

pub use numbering::per_game_instance_numbering;
pub use validation::validate_runtime;
