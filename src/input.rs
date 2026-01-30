// Input device management module

pub mod operations;
pub mod pipelines;
pub mod pure;
pub mod types;

// Re-export types
pub use types::{DeviceInfo, DeviceType, PadButton, PollResult};

// Re-export operations
pub use operations::{DeviceEvent, DeviceMonitor, InputDevice};
pub use operations::{open_device, scan_input_devices};

// Re-export pure functions
pub use pure::generate_display_names;

// Re-export pipelines
pub use pipelines::{check_permissions, install_udev_rules, PermissionStatus};

/// Find a device index by its unique identifier (Bluetooth MAC or USB serial)
/// Returns None if no device matches or the uniq is empty
pub fn find_device_by_uniq(devices: &[InputDevice], uniq: &str) -> Option<usize> {
    if uniq.is_empty() {
        return None;
    }
    devices
        .iter()
        .position(|d| d.uniq() == uniq && !d.uniq().is_empty())
}

/// Check if a device is already assigned to any instance
pub fn is_device_assigned(device_idx: usize, instances: &[crate::instance::Instance]) -> bool {
    instances
        .iter()
        .any(|inst| inst.devices.contains(&device_idx))
}
