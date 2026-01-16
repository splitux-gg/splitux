//! EOS operations - atomic side effects
//!
//! Each function performs a single I/O operation.

mod create_overlay;
mod find_dlls;
mod write_settings;

pub use create_overlay::create_instance_overlay;
pub use find_dlls::find_eos_dlls;
