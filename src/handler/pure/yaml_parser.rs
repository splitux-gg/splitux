// Pure YAML parsing functions for handler configuration
// No side effects - only transforms YAML structures

use serde_yaml::Value;

/// Expand dot-notation keys into nested YAML structure
/// e.g., "goldberg.disable_networking: true" becomes:
/// goldberg:
///   disable_networking: true
///
/// Special handling for settings keys:
/// - "goldberg.settings.force_lobby_type.txt" becomes goldberg.settings["force_lobby_type.txt"]
/// - Only expands keys that start with known backend prefixes (goldberg., photon., facepunch.)
/// - Does NOT recursively expand nested values to avoid mangling game_patches, etc.
pub fn expand_dot_notation(value: Value) -> Value {
    let Value::Mapping(map) = value else {
        return value;
    };

    let mut result = serde_yaml::Mapping::new();
    let known_backends = ["goldberg.", "photon.", "facepunch."];

    for (key, val) in map {
        let Value::String(key_str) = &key else {
            // Non-string key, keep as-is (no recursive expansion)
            result.insert(key, val);
            continue;
        };

        // Only expand keys that start with known backend prefixes
        let should_expand = known_backends.iter().any(|prefix| key_str.starts_with(prefix));

        if should_expand {
            // Smart split for dot notation
            // Handle "backend.settings.filename.ext" specially - settings values are filenames
            let parts = smart_split_dot_notation(key_str);
            insert_nested(&mut result, &parts, val);
        } else {
            // Not a backend key, keep as-is (no recursive expansion)
            result.insert(key, val);
        }
    }

    Value::Mapping(result)
}

/// Smart split for dot notation that preserves filenames in settings
/// - "goldberg.disable_networking" -> ["goldberg", "disable_networking"]
/// - "goldberg.settings.force_lobby_type.txt" -> ["goldberg", "settings", "force_lobby_type.txt"]
/// - "photon.config_path" -> ["photon", "config_path"]
pub fn smart_split_dot_notation(key: &str) -> Vec<&str> {
    // Check for the pattern: backend.settings.* where everything after settings. is the key
    let known_backends = ["goldberg", "photon", "facepunch"];

    for backend in known_backends {
        let settings_prefix = format!("{}.", backend);
        if key.starts_with(&settings_prefix) {
            let rest = &key[settings_prefix.len()..];

            // Check if this is a settings.* key
            if rest.starts_with("settings.") {
                let settings_key = &rest["settings.".len()..];
                // Return [backend, "settings", "everything.else.as.one.key"]
                return vec![backend, "settings", settings_key];
            } else {
                // Normal two-level split: backend.field
                if rest.find('.').is_some() {
                    // More dots, but not under settings - do normal split
                    // This handles cases like "goldberg.something.else" -> split all
                    return key.split('.').collect();
                } else {
                    return vec![backend, rest];
                }
            }
        }
    }

    // Not a known backend prefix, do normal split
    key.split('.').collect()
}

/// Insert a value at a nested path in the mapping
/// e.g., insert_nested(map, ["goldberg", "settings", "force_lobby_type"], "2")
/// creates: goldberg: { settings: { force_lobby_type: "2" } }
pub fn insert_nested(map: &mut serde_yaml::Mapping, parts: &[&str], value: Value) {
    if parts.is_empty() {
        return;
    }

    let key = Value::String(parts[0].to_string());

    if parts.len() == 1 {
        // Base case: insert the value at this key
        map.insert(key, value);
    } else {
        // Recursive case: get or create nested mapping
        let nested = map
            .entry(key.clone())
            .or_insert_with(|| Value::Mapping(serde_yaml::Mapping::new()));

        if let Value::Mapping(nested_map) = nested {
            insert_nested(nested_map, &parts[1..], value);
        } else {
            // Key exists but isn't a mapping - replace with mapping
            let mut new_map = serde_yaml::Mapping::new();
            insert_nested(&mut new_map, &parts[1..], value);
            *nested = Value::Mapping(new_map);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_notation_expansion() {
        let yaml = r#"
name: Test
goldberg.disable_networking: false
goldberg.settings.force_lobby_type.txt: "2"
goldberg.settings.invite_all.txt: ""
"#;
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let expanded = expand_dot_notation(raw);

        let map = expanded.as_mapping().unwrap();
        let goldberg = map
            .get(&serde_yaml::Value::String("goldberg".to_string()))
            .expect("goldberg key should exist");
        let goldberg_map = goldberg.as_mapping().expect("goldberg should be a mapping");

        let disable_net = goldberg_map
            .get(&serde_yaml::Value::String("disable_networking".to_string()))
            .expect("disable_networking should exist");
        assert_eq!(disable_net.as_bool(), Some(false));
    }

    #[test]
    fn test_smart_split_preserves_settings_filenames() {
        assert_eq!(
            smart_split_dot_notation("goldberg.settings.force_lobby_type.txt"),
            vec!["goldberg", "settings", "force_lobby_type.txt"]
        );
        assert_eq!(
            smart_split_dot_notation("goldberg.disable_networking"),
            vec!["goldberg", "disable_networking"]
        );
        assert_eq!(
            smart_split_dot_notation("photon.config_path"),
            vec!["photon", "config_path"]
        );
    }
}
