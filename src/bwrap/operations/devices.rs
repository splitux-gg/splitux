// Device discovery operations (I/O: reads /dev and /sys)

use crate::input::DeviceInfo;

use super::super::pure::matching::{build_gamepad_maps, match_evdev_to_gamepad, match_uniq_to_gamepad};

/// Get all /dev/input/js* device paths (legacy joystick interface)
pub fn glob_js_devices() -> Vec<String> {
    let mut devices = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("js") {
                devices.push(format!("/dev/input/{}", name_str));
            }
        }
    }
    devices
}

/// Get hidraw devices for gamepads NOT assigned to this instance
///
/// This blocks HIDAPI access to other players' controllers.
/// Works by:
/// 1. Finding hidraw devices that share a parent with known gamepad evdev devices
/// 2. Matching Bluetooth controllers by HID_UNIQ (MAC address)
pub fn get_gamepad_hidraw_devices(
    input_devices: &[DeviceInfo],
    assigned_indices: &[usize],
) -> Vec<String> {
    let mut to_block = Vec::new();

    // Build maps for gamepad paths and UNIQs
    let (all_gamepad_evdev, all_gamepad_uniq) = build_gamepad_maps(input_devices, assigned_indices);

    // For each hidraw device, check if it belongs to a gamepad
    if let Ok(entries) = std::fs::read_dir("/sys/class/hidraw") {
        for entry in entries.flatten() {
            let hidraw_name = entry.file_name();
            let hidraw_name_str = hidraw_name.to_string_lossy();
            let device_path = entry.path().join("device");

            let mut found_gamepad = false;
            let mut is_assigned = false;

            // Method 1: Check HID_UNIQ from uevent (works for Bluetooth controllers)
            let uevent_path = device_path.join("uevent");
            if let Ok(uevent) = std::fs::read_to_string(&uevent_path) {
                for line in uevent.lines() {
                    if let Some(hid_uniq) = line.strip_prefix("HID_UNIQ=") {
                        if let Some((found, assigned)) = match_uniq_to_gamepad(hid_uniq, &all_gamepad_uniq) {
                            found_gamepad = found;
                            is_assigned = assigned;
                        }
                        break;
                    }
                }
            }

            // Method 2: Look for input/event* nodes under device (works for USB controllers)
            if !found_gamepad {
                if let Ok(device_entries) = std::fs::read_dir(&device_path) {
                    for dev_entry in device_entries.flatten() {
                        let dev_name = dev_entry.file_name();
                        let dev_str = dev_name.to_string_lossy();
                        if dev_str.starts_with("input") {
                            if let Ok(input_entries) = std::fs::read_dir(dev_entry.path()) {
                                for input_entry in input_entries.flatten() {
                                    let input_name = input_entry.file_name();
                                    let input_str = input_name.to_string_lossy();
                                    if input_str.starts_with("event") {
                                        let evdev_path = format!("/dev/input/{}", input_str);
                                        if let Some((found, assigned)) = match_evdev_to_gamepad(&evdev_path, &all_gamepad_evdev) {
                                            found_gamepad = found;
                                            is_assigned = assigned;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Block this hidraw if it's a gamepad that's NOT assigned to this instance
            if found_gamepad && !is_assigned {
                to_block.push(format!("/dev/{}", hidraw_name_str));
            }
        }
    }

    to_block
}

/// Get gamepad evdev paths for assigned devices
pub fn get_assigned_gamepad_paths(
    input_devices: &[DeviceInfo],
    assigned_indices: &[usize],
) -> Vec<String> {
    super::super::pure::matching::filter_assigned_gamepad_paths(input_devices, assigned_indices)
}

/// Log assigned device indices for this instance.
pub fn log_assigned_devices(
    _cmd: &mut std::process::Command,
    _input_devices: &[DeviceInfo],
    assigned_indices: &[usize],
    instance_idx: usize,
) {
    println!(
        "[splitux] Instance {}: Assigned device indices: {:?}",
        instance_idx, assigned_indices
    );
}
