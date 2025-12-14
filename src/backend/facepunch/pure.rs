//! Pure functions for Facepunch backend
//!
//! These functions have no side effects and are deterministic.

mod config_gen;

pub use config_gen::generate_config_content;
