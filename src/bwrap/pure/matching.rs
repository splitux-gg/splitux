// Pure matching functions for device blocking (no I/O, no side effects)

use crate::input::{DeviceInfo, DeviceType};

/// Build maps of gamepad evdev paths and unique IDs, with assignment status.
///
/// Returns (evdev_map, uniq_map) where each entry is (identifier, is_assigned).
pub fn build_gamepad_maps<'a>(
    input_devices: &'a [DeviceInfo],
    assigned_indices: &[usize],
) -> (Vec<(&'a str, bool)>, Vec<(&'a str, bool)>) {
    let mut evdev_map = Vec::new();
    let mut uniq_map = Vec::new();

    for (i, dev) in input_devices.iter().enumerate() {
        if dev.device_type == DeviceType::Gamepad {
            let is_assigned = assigned_indices.contains(&i);
            evdev_map.push((dev.path.as_str(), is_assigned));
            if !dev.uniq.is_empty() {
                uniq_map.push((dev.uniq.as_str(), is_assigned));
            }
        }
    }

    (evdev_map, uniq_map)
}

/// Match a HID_UNIQ value against known gamepad unique IDs.
///
/// Returns Some((found_gamepad, is_assigned)) if matched.
pub fn match_uniq_to_gamepad(hid_uniq: &str, gamepad_uniqs: &[(&str, bool)]) -> Option<(bool, bool)> {
    for (gamepad_uniq, gamepad_assigned) in gamepad_uniqs {
        if *gamepad_uniq == hid_uniq {
            return Some((true, *gamepad_assigned));
        }
    }
    None
}

/// Match an evdev path against known gamepad evdev paths.
///
/// Returns Some((found_gamepad, is_assigned)) if matched.
pub fn match_evdev_to_gamepad(evdev_path: &str, gamepad_evdevs: &[(&str, bool)]) -> Option<(bool, bool)> {
    for (gamepad_path, gamepad_assigned) in gamepad_evdevs {
        if *gamepad_path == evdev_path {
            return Some((true, *gamepad_assigned));
        }
    }
    None
}

/// Build bwrap --bind args for blocking a list of device paths.
///
/// Each blocked path gets `--bind /dev/null {path}`.
pub fn build_blocking_args(paths_to_block: &[&str]) -> Vec<String> {
    let mut args = Vec::new();
    for path in paths_to_block {
        args.extend([
            "--bind".to_string(),
            "/dev/null".to_string(),
            path.to_string(),
        ]);
    }
    args
}

/// Get evdev paths for gamepads NOT assigned to this instance (pure filter).
pub fn filter_unassigned_gamepad_evdev(
    input_devices: &[DeviceInfo],
    assigned_indices: &[usize],
) -> Vec<String> {
    input_devices
        .iter()
        .enumerate()
        .filter(|(i, dev)| dev.device_type == DeviceType::Gamepad && !assigned_indices.contains(i))
        .map(|(_, dev)| dev.path.clone())
        .collect()
}

/// Get gamepad evdev paths for assigned devices (pure filter).
pub fn filter_assigned_gamepad_paths(
    input_devices: &[DeviceInfo],
    assigned_indices: &[usize],
) -> Vec<String> {
    assigned_indices
        .iter()
        .filter_map(|&d| {
            let dev = input_devices.get(d)?;
            if dev.device_type == DeviceType::Gamepad {
                Some(dev.path.clone())
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{DeviceInfo, DeviceType};

    fn make_device(path: &str, device_type: DeviceType, uniq: &str) -> DeviceInfo {
        DeviceInfo {
            path: path.to_string(),
            enabled: true,
            device_type,
            uniq: uniq.to_string(),
        }
    }

    // ── build_gamepad_maps ──────────────────────────────────────────

    #[test]
    fn build_gamepad_maps_mixed_devices() {
        let devices = vec![
            make_device("/dev/input/event0", DeviceType::Gamepad, "aabb"),
            make_device("/dev/input/event1", DeviceType::Keyboard, "ccdd"),
            make_device("/dev/input/event2", DeviceType::Gamepad, "eeff"),
            make_device("/dev/input/event3", DeviceType::Mouse, ""),
        ];
        let assigned = vec![0];

        let (evdev_map, uniq_map) = build_gamepad_maps(&devices, &assigned);

        assert_eq!(evdev_map.len(), 2);
        assert_eq!(evdev_map[0], ("/dev/input/event0", true));
        assert_eq!(evdev_map[1], ("/dev/input/event2", false));

        assert_eq!(uniq_map.len(), 2);
        assert_eq!(uniq_map[0], ("aabb", true));
        assert_eq!(uniq_map[1], ("eeff", false));
    }

    #[test]
    fn build_gamepad_maps_empty_uniq_excluded() {
        let devices = vec![
            make_device("/dev/input/event0", DeviceType::Gamepad, ""),
            make_device("/dev/input/event1", DeviceType::Gamepad, "1234"),
        ];
        let assigned = vec![0, 1];

        let (evdev_map, uniq_map) = build_gamepad_maps(&devices, &assigned);

        assert_eq!(evdev_map.len(), 2);
        // Only the device with a non-empty uniq appears in uniq_map
        assert_eq!(uniq_map.len(), 1);
        assert_eq!(uniq_map[0], ("1234", true));
    }

    #[test]
    fn build_gamepad_maps_no_gamepads() {
        let devices = vec![
            make_device("/dev/input/event0", DeviceType::Keyboard, "kb1"),
            make_device("/dev/input/event1", DeviceType::Mouse, "ms1"),
            make_device("/dev/input/event2", DeviceType::Other, "ot1"),
        ];

        let (evdev_map, uniq_map) = build_gamepad_maps(&devices, &[]);

        assert!(evdev_map.is_empty());
        assert!(uniq_map.is_empty());
    }

    #[test]
    fn build_gamepad_maps_empty_input() {
        let (evdev_map, uniq_map) = build_gamepad_maps(&[], &[]);
        assert!(evdev_map.is_empty());
        assert!(uniq_map.is_empty());
    }

    // ── match_uniq_to_gamepad ───────────────────────────────────────

    #[test]
    fn match_uniq_found_assigned() {
        let uniqs = vec![("aabb", true), ("ccdd", false)];
        let result = match_uniq_to_gamepad("aabb", &uniqs);
        assert_eq!(result, Some((true, true)));
    }

    #[test]
    fn match_uniq_found_unassigned() {
        let uniqs = vec![("aabb", true), ("ccdd", false)];
        let result = match_uniq_to_gamepad("ccdd", &uniqs);
        assert_eq!(result, Some((true, false)));
    }

    #[test]
    fn match_uniq_not_found() {
        let uniqs = vec![("aabb", true), ("ccdd", false)];
        let result = match_uniq_to_gamepad("xxxx", &uniqs);
        assert_eq!(result, None);
    }

    #[test]
    fn match_uniq_empty_list() {
        let result = match_uniq_to_gamepad("aabb", &[]);
        assert_eq!(result, None);
    }

    // ── match_evdev_to_gamepad ──────────────────────────────────────

    #[test]
    fn match_evdev_found_assigned() {
        let evdevs = vec![("/dev/input/event0", true), ("/dev/input/event1", false)];
        let result = match_evdev_to_gamepad("/dev/input/event0", &evdevs);
        assert_eq!(result, Some((true, true)));
    }

    #[test]
    fn match_evdev_found_unassigned() {
        let evdevs = vec![("/dev/input/event0", true), ("/dev/input/event1", false)];
        let result = match_evdev_to_gamepad("/dev/input/event1", &evdevs);
        assert_eq!(result, Some((true, false)));
    }

    #[test]
    fn match_evdev_not_found() {
        let evdevs = vec![("/dev/input/event0", true)];
        let result = match_evdev_to_gamepad("/dev/input/event99", &evdevs);
        assert_eq!(result, None);
    }

    #[test]
    fn match_evdev_empty_list() {
        let result = match_evdev_to_gamepad("/dev/input/event0", &[]);
        assert_eq!(result, None);
    }

    // ── build_blocking_args ─────────────────────────────────────────

    #[test]
    fn build_blocking_args_empty() {
        let args = build_blocking_args(&[]);
        assert!(args.is_empty());
    }

    #[test]
    fn build_blocking_args_single_path() {
        let args = build_blocking_args(&["/dev/input/event0"]);
        assert_eq!(args, vec!["--bind", "/dev/null", "/dev/input/event0"]);
    }

    #[test]
    fn build_blocking_args_multiple_paths() {
        let args = build_blocking_args(&["/dev/input/event0", "/dev/input/event1", "/dev/input/event2"]);
        assert_eq!(args.len(), 9);
        // Verify the repeating --bind /dev/null pattern
        for chunk in args.chunks(3) {
            assert_eq!(chunk[0], "--bind");
            assert_eq!(chunk[1], "/dev/null");
        }
        assert_eq!(args[2], "/dev/input/event0");
        assert_eq!(args[5], "/dev/input/event1");
        assert_eq!(args[8], "/dev/input/event2");
    }

    // ── filter_unassigned_gamepad_evdev ─────────────────────────────

    #[test]
    fn filter_unassigned_three_gamepads_one_assigned() {
        let devices = vec![
            make_device("/dev/input/event0", DeviceType::Gamepad, ""),
            make_device("/dev/input/event1", DeviceType::Gamepad, ""),
            make_device("/dev/input/event2", DeviceType::Gamepad, ""),
        ];
        let assigned = vec![1];

        let result = filter_unassigned_gamepad_evdev(&devices, &assigned);
        assert_eq!(result, vec!["/dev/input/event0", "/dev/input/event2"]);
    }

    #[test]
    fn filter_unassigned_no_gamepads() {
        let devices = vec![
            make_device("/dev/input/event0", DeviceType::Keyboard, ""),
            make_device("/dev/input/event1", DeviceType::Mouse, ""),
        ];

        let result = filter_unassigned_gamepad_evdev(&devices, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_unassigned_all_assigned() {
        let devices = vec![
            make_device("/dev/input/event0", DeviceType::Gamepad, ""),
            make_device("/dev/input/event1", DeviceType::Gamepad, ""),
        ];
        let assigned = vec![0, 1];

        let result = filter_unassigned_gamepad_evdev(&devices, &assigned);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_unassigned_mixed_device_types() {
        let devices = vec![
            make_device("/dev/input/event0", DeviceType::Gamepad, ""),
            make_device("/dev/input/event1", DeviceType::Keyboard, ""),
            make_device("/dev/input/event2", DeviceType::Gamepad, ""),
            make_device("/dev/input/event3", DeviceType::Mouse, ""),
        ];
        // Assign index 1 (keyboard) — should not affect gamepad filtering
        let assigned = vec![1];

        let result = filter_unassigned_gamepad_evdev(&devices, &assigned);
        assert_eq!(result, vec!["/dev/input/event0", "/dev/input/event2"]);
    }

    // ── filter_assigned_gamepad_paths ───────────────────────────────

    #[test]
    fn filter_assigned_gamepads_returned() {
        let devices = vec![
            make_device("/dev/input/event0", DeviceType::Gamepad, ""),
            make_device("/dev/input/event1", DeviceType::Gamepad, ""),
            make_device("/dev/input/event2", DeviceType::Gamepad, ""),
        ];
        let assigned = vec![0, 2];

        let result = filter_assigned_gamepad_paths(&devices, &assigned);
        assert_eq!(result, vec!["/dev/input/event0", "/dev/input/event2"]);
    }

    #[test]
    fn filter_assigned_non_gamepads_excluded() {
        let devices = vec![
            make_device("/dev/input/event0", DeviceType::Keyboard, ""),
            make_device("/dev/input/event1", DeviceType::Gamepad, ""),
            make_device("/dev/input/event2", DeviceType::Mouse, ""),
        ];
        // Assign keyboard and mouse — neither should appear
        let assigned = vec![0, 2];

        let result = filter_assigned_gamepad_paths(&devices, &assigned);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_assigned_out_of_bounds_handled() {
        let devices = vec![
            make_device("/dev/input/event0", DeviceType::Gamepad, ""),
        ];
        // Index 5 is out of bounds — should be silently skipped
        let assigned = vec![0, 5];

        let result = filter_assigned_gamepad_paths(&devices, &assigned);
        assert_eq!(result, vec!["/dev/input/event0"]);
    }

    #[test]
    fn filter_assigned_empty_inputs() {
        let result = filter_assigned_gamepad_paths(&[], &[]);
        assert!(result.is_empty());
    }
}
