//! Device classification based on sink properties
//!
//! Pure functions for classifying audio sinks as speakers, headphones, HDMI, etc.

use crate::audio::types::AudioDeviceType;

/// Classify an audio device based on its name and description
///
/// This is a pure function that determines the device type from string patterns.
pub fn classify_device(name: &str, description: &str) -> AudioDeviceType {
    let name_lower = name.to_lowercase();
    let desc_lower = description.to_lowercase();

    // Check for HDMI/DisplayPort first (most specific)
    if name_lower.contains("hdmi") || desc_lower.contains("hdmi") {
        return AudioDeviceType::Hdmi;
    }
    // DisplayPort: match "dp-" but exclude Bluetooth A2DP ("a2dp")
    if name_lower.contains("displayport")
        || (name_lower.contains("dp-") && !name_lower.contains("a2dp"))
    {
        return AudioDeviceType::Hdmi; // Treat DP same as HDMI for display audio
    }

    // Check for Bluetooth
    if name_lower.contains("bluez")
        || name_lower.contains("bluetooth")
        || desc_lower.contains("bluetooth")
    {
        return AudioDeviceType::Bluetooth;
    }

    // Check for headphones/headset
    if name_lower.contains("headphone")
        || desc_lower.contains("headphone")
        || name_lower.contains("headset")
        || desc_lower.contains("headset")
    {
        return AudioDeviceType::Headphone;
    }

    // Check for virtual/null sinks
    if name_lower.starts_with("splitux_") || name_lower.contains("null") {
        return AudioDeviceType::Virtual;
    }

    // USB audio devices without specific type - default to headphones
    // (most USB audio is headphones/DACs, even if PulseAudio reports analog-stereo)
    if name_lower.contains("usb") {
        return AudioDeviceType::Headphone;
    }

    // Check for speakers (analog output is usually speakers)
    if name_lower.contains("analog") || desc_lower.contains("speaker") {
        return AudioDeviceType::Speaker;
    }

    AudioDeviceType::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_hdmi() {
        assert_eq!(
            classify_device("alsa_output.pci-0000_01_00.1.hdmi-stereo", "HDMI Audio"),
            AudioDeviceType::Hdmi
        );
    }

    #[test]
    fn test_classify_analog_as_speaker() {
        assert_eq!(
            classify_device(
                "alsa_output.pci-0000_00_1f.3.analog-stereo",
                "Built-in Audio Analog Stereo"
            ),
            AudioDeviceType::Speaker
        );
    }

    #[test]
    fn test_classify_bluetooth() {
        assert_eq!(
            classify_device("bluez_output.XX_XX_XX_XX_XX_XX.a2dp-sink", "WH-1000XM4"),
            AudioDeviceType::Bluetooth
        );
    }

    #[test]
    fn test_classify_usb_headphones() {
        assert_eq!(
            classify_device(
                "alsa_output.usb-SteelSeries_Arctis_7-00.analog-stereo",
                "Arctis 7"
            ),
            AudioDeviceType::Headphone
        );
    }

    #[test]
    fn test_classify_virtual() {
        assert_eq!(
            classify_device("splitux_instance_0", "Splitux Instance 1 Audio"),
            AudioDeviceType::Virtual
        );
    }
}
