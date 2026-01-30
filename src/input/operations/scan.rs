// Input device scanning operations (I/O: evdev enumeration, device opening)

use crate::app::PadFilterType;
use crate::input::operations::device::InputDevice;
use crate::input::pure::classify::{calculate_stick_calibration, classify_device, is_device_enabled};
use crate::input::types::DeviceType;
use evdev::*;

/// Scan all input devices and return those matching the filter
pub fn scan_input_devices(filter: &PadFilterType) -> Vec<InputDevice> {
    let mut pads: Vec<InputDevice> = Vec::new();
    for dev in evdev::enumerate() {
        let enabled = is_device_enabled(filter, dev.1.input_id().vendor());
        let device_type = classify_device(dev.1.supported_keys());

        if device_type != DeviceType::Other {
            if dev.1.set_nonblocking(true).is_err() {
                println!(
                    "[splitux] evdev: Failed to set non-blocking mode for {}",
                    dev.0.display()
                );
                continue;
            }

            // Detect stick axis range from device info
            let (stick_center, stick_threshold) = if let Ok(abs_info) = dev.1.get_abs_state() {
                // Try to get ABS_X info for stick range
                if let Some(x_info) = abs_info.get(AbsoluteAxisCode::ABS_X.0 as usize) {
                    let (center, threshold) = calculate_stick_calibration(x_info.minimum, x_info.maximum);
                    println!(
                        "[splitux] evdev: {} stick range: {}-{}, center={}, threshold={}",
                        dev.0.display(),
                        x_info.minimum,
                        x_info.maximum,
                        center,
                        threshold
                    );
                    (center, threshold)
                } else {
                    // Default to signed 16-bit range
                    (0, 8000)
                }
            } else {
                // Default to signed 16-bit range
                (0, 8000)
            };

            // Get the unique identifier (Bluetooth MAC or USB serial)
            let uniq = dev.1.unique_name().unwrap_or("").to_string();

            pads.push(InputDevice::new(
                dev.0.to_str().unwrap().to_string(),
                dev.1,
                enabled,
                device_type,
                stick_center,
                stick_threshold,
                uniq,
            ));
        }
    }
    pads.sort_by_key(|pad| pad.path().to_string());
    pads
}

/// Try to open a single device by path and create an InputDevice.
/// Retries with exponential backoff for udev race conditions.
pub fn open_device(path: &str, filter: &PadFilterType) -> Option<InputDevice> {
    let dev = {
        let mut attempts = 0;
        let max_attempts = 8;
        let mut delay_ms = 50u64;
        loop {
            match Device::open(path) {
                Ok(d) => break d,
                Err(e) => {
                    attempts += 1;
                    let is_permission_error = e.kind() == std::io::ErrorKind::PermissionDenied;

                    if attempts >= max_attempts {
                        if is_permission_error {
                            println!("[splitux] evdev: Permission denied for {} - ensure your user is in the 'input' group (run: sudo usermod -aG input $USER)", path);
                        } else {
                            println!(
                                "[splitux] evdev: Failed to open {} after {} attempts: {}",
                                path, attempts, e
                            );
                        }
                        return None;
                    }

                    // Use longer delays for permission errors (udev rules may be slow)
                    let wait_time = if is_permission_error {
                        delay_ms * 2
                    } else {
                        delay_ms
                    };
                    std::thread::sleep(std::time::Duration::from_millis(wait_time));
                    delay_ms = (delay_ms * 2).min(500); // Exponential backoff, max 500ms
                }
            }
        }
    };

    let enabled = is_device_enabled(filter, dev.input_id().vendor());
    let device_type = classify_device(dev.supported_keys());

    if device_type == DeviceType::Other {
        println!(
            "[splitux] evdev: Skipping {} - not a gamepad/keyboard/mouse",
            path
        );
        return None;
    }

    if dev.set_nonblocking(true).is_err() {
        println!(
            "[splitux] evdev: Failed to set non-blocking mode for {}",
            path
        );
        return None;
    }

    // Detect stick axis range from device info
    let (stick_center, stick_threshold) = if let Ok(abs_info) = dev.get_abs_state() {
        if let Some(x_info) = abs_info.get(AbsoluteAxisCode::ABS_X.0 as usize) {
            let (center, threshold) = calculate_stick_calibration(x_info.minimum, x_info.maximum);
            println!(
                "[splitux] evdev: {} stick range: {}-{}, center={}, threshold={}",
                path, x_info.minimum, x_info.maximum, center, threshold
            );
            (center, threshold)
        } else {
            (0, 8000)
        }
    } else {
        (0, 8000)
    };

    let uniq = dev.unique_name().unwrap_or("").to_string();

    Some(InputDevice::new(
        path.to_string(),
        dev,
        enabled,
        device_type,
        stick_center,
        stick_threshold,
        uniq,
    ))
}
