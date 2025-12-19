// Display name generation for input devices

use crate::input::operations::bluetooth::get_bluetooth_alias;
use crate::input::InputDevice;
use std::collections::HashMap;

/// Generate display names for all devices
/// Priority: 1) User-defined alias, 2) Bluetooth alias, 3) evdev name
/// Adds "(1)", "(2)" suffixes for duplicates without unique aliases
/// Returns a Vec of display names in the same order as the input devices
pub fn generate_display_names(
    devices: &[InputDevice],
    user_aliases: &HashMap<String, String>,
) -> Vec<String> {
    // First pass: get display names with priority
    let base_names: Vec<String> = devices
        .iter()
        .map(|dev| {
            let uniq = dev.uniq();
            // 1) Check user-defined alias first
            if let Some(alias) = user_aliases.get(uniq) {
                return alias.clone();
            }
            // 2) Try Bluetooth alias
            if let Some(alias) = get_bluetooth_alias(uniq) {
                return alias;
            }
            // 3) Use type prefix if available (e.g., "Xbox Controller")
            let type_prefix = dev.type_prefix();
            if !type_prefix.is_empty() {
                return format!("{} Controller", type_prefix);
            }
            // 4) Fall back to evdev name
            dev.fancyname().to_string()
        })
        .collect();

    // Count occurrences of each base name
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    for name in &base_names {
        *name_counts.entry(name.clone()).or_insert(0) += 1;
    }

    // Track which index we're on for each duplicate name
    let mut name_indices: HashMap<String, usize> = HashMap::new();

    base_names
        .into_iter()
        .map(|name| {
            let count = *name_counts.get(&name).unwrap_or(&1);

            if count > 1 {
                // Multiple devices with same name - add suffix
                let idx = name_indices.entry(name.clone()).or_insert(0);
                *idx += 1;
                format!("{} ({})", name, idx)
            } else {
                name
            }
        })
        .collect()
}
