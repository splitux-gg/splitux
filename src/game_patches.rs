//! Game config file patching module
//!
//! Handles modifying game configuration files based on handler settings.
//! Supports multiple config formats with auto-detection.

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;

/// Detected config file format
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfigFormat {
    /// `set key "value"` or `set key 'value'` - Riftbreaker style
    SetStyle,
    /// `key=value` or `key = value` - INI style
    IniStyle,
    /// `key value` - space separated
    SpaceStyle,
    /// Unknown format - will use line search/replace
    Unknown,
}

/// Detect the config format by scanning file content
pub fn detect_format(content: &str) -> ConfigFormat {
    let mut set_count = 0;
    let mut ini_count = 0;
    let mut space_count = 0;

    for line in content.lines().take(50) {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        // Check for set-style: set key "value" or set key 'value'
        if trimmed.starts_with("set ") && (trimmed.contains('"') || trimmed.contains('\'')) {
            set_count += 1;
            continue;
        }

        // Check for ini-style: key=value
        if trimmed.contains('=') && !trimmed.starts_with('=') {
            ini_count += 1;
            continue;
        }

        // Check for space-style: key value (at least two words)
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 && !parts[0].contains('=') {
            space_count += 1;
        }
    }

    // Return the most common format
    if set_count > 0 && set_count >= ini_count && set_count >= space_count {
        ConfigFormat::SetStyle
    } else if ini_count > 0 && ini_count >= space_count {
        ConfigFormat::IniStyle
    } else if space_count > 0 {
        ConfigFormat::SpaceStyle
    } else {
        ConfigFormat::Unknown
    }
}

/// Apply patches to config content based on detected format
pub fn apply_patches(
    content: &str,
    patches: &HashMap<String, String>,
    format: ConfigFormat,
) -> Result<String, Box<dyn Error>> {
    let mut result = content.to_string();
    let mut applied: Vec<&str> = Vec::new();

    for (key, value) in patches {
        let patched = match format {
            ConfigFormat::SetStyle => apply_set_style_patch(&result, key, value),
            ConfigFormat::IniStyle => apply_ini_style_patch(&result, key, value),
            ConfigFormat::SpaceStyle => apply_space_style_patch(&result, key, value),
            ConfigFormat::Unknown => apply_line_search_replace(&result, key, value),
        };

        if let Some(new_content) = patched {
            result = new_content;
            applied.push(key);
        } else {
            return Err(format!(
                "Key '{}' not found in config file. Handler configuration may be incorrect.",
                key
            )
            .into());
        }
    }

    Ok(result)
}

/// Apply patch for set-style: `set key "value"`
fn apply_set_style_patch(content: &str, key: &str, value: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut found = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Match: set key "..." or set key '...'
        if trimmed.starts_with("set ")
            && trimmed
                .strip_prefix("set ")
                .map(|rest| rest.trim().starts_with(key))
                .unwrap_or(false)
        {
            // Check if the key matches exactly (followed by space or quote)
            let after_set = trimmed.strip_prefix("set ").unwrap().trim();
            if after_set.starts_with(key)
                && after_set[key.len()..]
                    .chars()
                    .next()
                    .map(|c| c.is_whitespace() || c == '"' || c == '\'')
                    .unwrap_or(false)
            {
                // Preserve original indentation
                let indent = line.len() - line.trim_start().len();
                let indent_str: String = line.chars().take(indent).collect();
                lines.push(format!("{}set {} \"{}\"", indent_str, key, value));
                found = true;
                continue;
            }
        }
        lines.push(line.to_string());
    }

    if found {
        Some(lines.join("\n"))
    } else {
        None
    }
}

/// Apply patch for ini-style: `key=value`
fn apply_ini_style_patch(content: &str, key: &str, value: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut found = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Match: key=... or key = ...
        if let Some(eq_pos) = trimmed.find('=') {
            let line_key = trimmed[..eq_pos].trim();
            if line_key == key {
                // Preserve original indentation and spacing style
                let indent = line.len() - line.trim_start().len();
                let indent_str: String = line.chars().take(indent).collect();

                // Detect if original used spaces around =
                let has_spaces = trimmed.contains(" = ");
                if has_spaces {
                    lines.push(format!("{}{} = {}", indent_str, key, value));
                } else {
                    lines.push(format!("{}{}={}", indent_str, key, value));
                }
                found = true;
                continue;
            }
        }
        lines.push(line.to_string());
    }

    if found {
        Some(lines.join("\n"))
    } else {
        None
    }
}

/// Apply patch for space-style: `key value`
fn apply_space_style_patch(content: &str, key: &str, value: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut found = false;

    for line in content.lines() {
        let trimmed = line.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();

        if !parts.is_empty() && parts[0] == key {
            // Preserve original indentation
            let indent = line.len() - line.trim_start().len();
            let indent_str: String = line.chars().take(indent).collect();
            lines.push(format!("{}{} {}", indent_str, key, value));
            found = true;
            continue;
        }
        lines.push(line.to_string());
    }

    if found {
        Some(lines.join("\n"))
    } else {
        None
    }
}

/// Fallback: simple line search/replace for unknown formats
fn apply_line_search_replace(content: &str, key: &str, value: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut found = false;

    for line in content.lines() {
        if line.contains(key) {
            // Replace the entire line with key and value
            let indent = line.len() - line.trim_start().len();
            let indent_str: String = line.chars().take(indent).collect();
            lines.push(format!("{}{} {}", indent_str, key, value));
            found = true;
            continue;
        }
        lines.push(line.to_string());
    }

    if found {
        Some(lines.join("\n"))
    } else {
        None
    }
}

/// Generate patched config file content
/// If the original file doesn't exist, creates new content
pub fn generate_patched_content(
    game_root: &Path,
    file_path: &str,
    patches: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    let full_path = game_root.join(file_path);

    if full_path.exists() {
        // Read existing file and apply patches
        let content = fs::read_to_string(&full_path)?;
        let format = detect_format(&content);
        println!(
            "[game_patches] Detected format {:?} for {}",
            format, file_path
        );
        apply_patches(&content, patches, format)
    } else {
        // Create new file with patches
        // Use INI style for new files as it's most common
        println!(
            "[game_patches] Creating new file {} with {} patches",
            file_path,
            patches.len()
        );
        let mut lines: Vec<String> = Vec::new();
        for (key, value) in patches {
            lines.push(format!("{}={}", key, value));
        }
        Ok(lines.join("\n"))
    }
}

/// Apply all game patches from handler config
/// Writes patched files to the specified output directory (overlay upper)
pub fn apply_game_patches(
    game_root: &Path,
    output_dir: &Path,
    patches: &HashMap<String, HashMap<String, String>>,
) -> Result<(), Box<dyn Error>> {
    if patches.is_empty() {
        return Ok(());
    }

    println!("[game_patches] Applying {} file patches", patches.len());

    for (file_path, file_patches) in patches {
        // Validate path doesn't escape game root
        if file_path.contains("..") {
            return Err(format!(
                "Invalid game patch path '{}': path traversal not allowed",
                file_path
            )
            .into());
        }

        let patched_content = generate_patched_content(game_root, file_path, file_patches)?;

        // Write to output directory (preserving directory structure)
        let output_path = output_dir.join(file_path);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&output_path, patched_content)?;
        println!("[game_patches] Written patched file: {}", output_path.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_set_style() {
        let content = r#"
set platform "win"
set storefront "steam"
set disable_steam "0"
"#;
        assert_eq!(detect_format(content), ConfigFormat::SetStyle);
    }

    #[test]
    fn test_detect_ini_style() {
        let content = r#"
[section]
key1=value1
key2 = value2
"#;
        assert_eq!(detect_format(content), ConfigFormat::IniStyle);
    }

    #[test]
    fn test_detect_space_style() {
        let content = r#"
host localhost
port 8080
debug true
"#;
        assert_eq!(detect_format(content), ConfigFormat::SpaceStyle);
    }

    #[test]
    fn test_apply_set_style_patch() {
        let content = r#"set disable_steam "0"
set platform "win""#;
        let mut patches = HashMap::new();
        patches.insert("disable_steam".to_string(), "1".to_string());

        let result = apply_patches(content, &patches, ConfigFormat::SetStyle).unwrap();
        assert!(result.contains(r#"set disable_steam "1""#));
        assert!(result.contains(r#"set platform "win""#));
    }

    #[test]
    fn test_apply_ini_style_patch() {
        let content = "key1=old\nkey2=value2";
        let mut patches = HashMap::new();
        patches.insert("key1".to_string(), "new".to_string());

        let result = apply_patches(content, &patches, ConfigFormat::IniStyle).unwrap();
        assert!(result.contains("key1=new"));
        assert!(result.contains("key2=value2"));
    }

    #[test]
    fn test_key_not_found_error() {
        let content = "key1=value1";
        let mut patches = HashMap::new();
        patches.insert("nonexistent".to_string(), "value".to_string());

        let result = apply_patches(content, &patches, ConfigFormat::IniStyle);
        assert!(result.is_err());
    }
}
