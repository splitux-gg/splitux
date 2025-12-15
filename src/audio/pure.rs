//! Pure functions for audio module
//!
//! These functions have no side effects and are deterministic.

mod device_classification;
mod sink_name;

pub use device_classification::classify_device;
pub use sink_name::{
    generate_virtual_sink_description, generate_virtual_sink_name, is_splitux_sink,
    parse_module_id,
};
