// Device type classification (pure functions)

use evdev::{AttributeSetRef, KeyCode};

use crate::app::PadFilterType;
use crate::input::types::DeviceType;

/// Classify an evdev device by its supported keys
pub fn classify_device(supported_keys: Option<&AttributeSetRef<KeyCode>>) -> DeviceType {
    if supported_keys.map_or(false, |keys| keys.contains(KeyCode::BTN_SOUTH)) {
        DeviceType::Gamepad
    } else if supported_keys.map_or(false, |keys| keys.contains(KeyCode::BTN_LEFT)) {
        DeviceType::Mouse
    } else if supported_keys.map_or(false, |keys| keys.contains(KeyCode::KEY_SPACE)) {
        DeviceType::Keyboard
    } else {
        DeviceType::Other
    }
}

/// Check if a device should be enabled based on the filter type and vendor ID
pub fn is_device_enabled(filter: &PadFilterType, vendor_id: u16) -> bool {
    match filter {
        PadFilterType::All => true,
        PadFilterType::NoSteamInput => vendor_id != 0x28de,
        PadFilterType::OnlySteamInput => vendor_id == 0x28de,
    }
}

/// Calculate stick center and threshold from axis min/max values
/// Returns (center, threshold) where threshold is 25% of range
pub fn calculate_stick_calibration(min: i32, max: i32) -> (i32, i32) {
    let center = (min + max) / 2;
    let range = max - min;
    let threshold = range / 4; // 25% deadzone
    (center, threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEAM_INPUT_VENDOR: u16 = 0x28de;
    const XBOX_VENDOR: u16 = 0x045e;
    const SONY_VENDOR: u16 = 0x054c;
    const GENERIC_VENDOR: u16 = 0x0001;

    // --- classify_device tests ---

    #[test]
    fn classify_device_none_returns_other() {
        assert!(matches!(classify_device(None), DeviceType::Other));
    }

    // --- is_device_enabled tests ---

    #[test]
    fn filter_all_allows_any_vendor() {
        assert!(is_device_enabled(&PadFilterType::All, STEAM_INPUT_VENDOR));
        assert!(is_device_enabled(&PadFilterType::All, XBOX_VENDOR));
        assert!(is_device_enabled(&PadFilterType::All, SONY_VENDOR));
        assert!(is_device_enabled(&PadFilterType::All, GENERIC_VENDOR));
        assert!(is_device_enabled(&PadFilterType::All, 0x0000));
        assert!(is_device_enabled(&PadFilterType::All, 0xFFFF));
    }

    #[test]
    fn filter_no_steam_input_blocks_steam_vendor() {
        assert!(!is_device_enabled(
            &PadFilterType::NoSteamInput,
            STEAM_INPUT_VENDOR
        ));
    }

    #[test]
    fn filter_no_steam_input_allows_other_vendors() {
        assert!(is_device_enabled(&PadFilterType::NoSteamInput, XBOX_VENDOR));
        assert!(is_device_enabled(&PadFilterType::NoSteamInput, SONY_VENDOR));
        assert!(is_device_enabled(
            &PadFilterType::NoSteamInput,
            GENERIC_VENDOR
        ));
        assert!(is_device_enabled(&PadFilterType::NoSteamInput, 0x0000));
        assert!(is_device_enabled(&PadFilterType::NoSteamInput, 0xFFFF));
    }

    #[test]
    fn filter_only_steam_input_allows_steam_vendor() {
        assert!(is_device_enabled(
            &PadFilterType::OnlySteamInput,
            STEAM_INPUT_VENDOR
        ));
    }

    #[test]
    fn filter_only_steam_input_blocks_other_vendors() {
        assert!(!is_device_enabled(
            &PadFilterType::OnlySteamInput,
            XBOX_VENDOR
        ));
        assert!(!is_device_enabled(
            &PadFilterType::OnlySteamInput,
            SONY_VENDOR
        ));
        assert!(!is_device_enabled(
            &PadFilterType::OnlySteamInput,
            GENERIC_VENDOR
        ));
        assert!(!is_device_enabled(&PadFilterType::OnlySteamInput, 0x0000));
        assert!(!is_device_enabled(&PadFilterType::OnlySteamInput, 0xFFFF));
    }

    // --- calculate_stick_calibration tests ---

    #[test]
    fn calibration_xbox_range() {
        // Standard Xbox unsigned range: 0 to 65535
        let (center, threshold) = calculate_stick_calibration(0, 65535);
        assert_eq!(center, 32767); // (0 + 65535) / 2 with integer division
        assert_eq!(threshold, 16383); // 65535 / 4
    }

    #[test]
    fn calibration_symmetric_signed_range() {
        // Symmetric signed range: -32768 to 32767
        let (center, threshold) = calculate_stick_calibration(-32768, 32767);
        assert_eq!(center, 0); // (-32768 + 32767) = -1, -1 / 2 = 0 (truncates toward zero)
        assert_eq!(threshold, 16383); // (32767 - (-32768)) / 4 = 65535 / 4
    }

    #[test]
    fn calibration_zero_range() {
        let (center, threshold) = calculate_stick_calibration(0, 0);
        assert_eq!(center, 0);
        assert_eq!(threshold, 0);
    }

    #[test]
    fn calibration_small_range() {
        let (center, threshold) = calculate_stick_calibration(0, 100);
        assert_eq!(center, 50);
        assert_eq!(threshold, 25);
    }
}
