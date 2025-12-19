// Bluetooth device name resolution

/// Get the Bluetooth alias for a device by its MAC address
/// Returns None if not a Bluetooth device or no alias is set
pub fn get_bluetooth_alias(mac_address: &str) -> Option<String> {
    // MAC address format check (e.g., "e4:17:d8:27:2d:f0")
    if mac_address.len() != 17 || mac_address.chars().filter(|&c| c == ':').count() != 5 {
        return None;
    }

    // Try to get alias from bluetoothctl
    let output = std::process::Command::new("bluetoothctl")
        .args(["info", mac_address])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the Alias line
    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("Alias:") {
            let alias = line.trim_start_matches("Alias:").trim();
            // Only return if alias differs from the generic device name
            // (Bluetooth sets Alias = Name by default if user hasn't customized it)
            if !alias.is_empty() {
                return Some(alias.to_string());
            }
        }
    }

    None
}
