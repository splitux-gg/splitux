//! Splitux config generation
//!
//! Pure functions for generating splitux.cfg content.

use super::super::types::FacepunchConfig;

/// Generate the splitux.cfg content for an instance
/// Uses flat format (no sections) to match the working bepinex-test config
pub fn generate_config_content(config: &FacepunchConfig) -> String {
    let mut content = String::new();

    // Flat format matching bepinex-test
    content.push_str(&format!("player_index={}\n", config.player_index));
    content.push_str(&format!("account_name={}\n", config.account_name));
    content.push_str(&format!("steam_id={}\n", config.steam_id));

    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_config() {
        let config = FacepunchConfig::new(0, "TestPlayer".to_string(), 76561198000000000);
        let content = generate_config_content(&config);

        assert!(content.contains("player_index=0"));
        assert!(content.contains("account_name=TestPlayer"));
        assert!(content.contains("steam_id=76561198000000000"));
    }
}
