// Device blocking operations (I/O: checks path.exists() + write permissions)

use crate::input::DeviceInfo;

use super::super::pure::matching::filter_unassigned_gamepad_evdev;
use super::devices::get_gamepad_hidraw_devices;

/// Filter paths to only those currently accessible for writing.
fn filter_accessible_paths(paths: &[String]) -> Vec<&str> {
    paths
        .iter()
        .filter(|p| {
            let path = std::path::Path::new(p);
            path.exists() && std::fs::OpenOptions::new().write(true).open(path).is_ok()
        })
        .map(|p| p.as_str())
        .collect()
}

/// Build blocking args for js devices. Returns bwrap --bind args as a flat Vec.
///
/// Call this RIGHT BEFORE spawning so we check which devices are currently
/// accessible. Gamescope may recreate js devices with different ownership.
pub fn get_js_blocking_args(initial_js_devices: &[String], instance_idx: usize) -> Vec<String> {
    let js_to_block = filter_accessible_paths(initial_js_devices);
    println!(
        "[splitux] Instance {}: Blocking {} js devices: {:?}",
        instance_idx,
        js_to_block.len(),
        js_to_block
    );
    super::super::pure::matching::build_blocking_args(&js_to_block)
}

/// Build blocking args for evdev and hidraw devices. Returns bwrap --bind args as a flat Vec.
///
/// Call this RIGHT BEFORE spawning so we check which devices are currently
/// accessible. Gamescope may recreate devices with different ownership.
pub fn get_evdev_hidraw_blocking_args(
    input_devices: &[DeviceInfo],
    assigned_indices: &[usize],
    instance_idx: usize,
) -> Vec<String> {
    let mut args = Vec::new();

    // Block evdev devices with permission check
    let unassigned_evdev = filter_unassigned_gamepad_evdev(input_devices, assigned_indices);
    let evdev_to_block = filter_accessible_paths(&unassigned_evdev);

    println!(
        "[splitux] Instance {}: Blocking {} evdev devices: {:?}",
        instance_idx,
        evdev_to_block.len(),
        evdev_to_block
    );

    args.extend(super::super::pure::matching::build_blocking_args(&evdev_to_block));

    // Block hidraw devices with permission check
    let unassigned_hidraw = get_gamepad_hidraw_devices(input_devices, assigned_indices);
    let hidraw_to_block = filter_accessible_paths(&unassigned_hidraw);

    println!(
        "[splitux] Instance {}: Blocking {} hidraw devices: {:?}",
        instance_idx,
        hidraw_to_block.len(),
        hidraw_to_block
    );

    args.extend(super::super::pure::matching::build_blocking_args(&hidraw_to_block));

    args
}
