//! Pure functions for Goldberg backend
//!
//! These functions have no side effects and are deterministic.

mod bitness;
mod interfaces;

pub use bitness::detect_bitness;
pub use interfaces::interfaces_file_contents;
