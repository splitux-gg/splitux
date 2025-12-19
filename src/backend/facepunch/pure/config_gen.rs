//! Splitux config generation
//!
//! Pure functions for generating splitux.cfg content.
//! Format matches bepinex-facepunch-splitux/src/SplituxConfig.cs parser.

use super::super::types::FacepunchConfig;

/// Generate the splitux.cfg content for an instance
/// Uses INI format with sections: [Identity], [Facepunch], [RuntimePatches]
pub fn generate_config_content(config: &FacepunchConfig) -> String {
    let mut content = String::new();

    // [Identity] section
    content.push_str("[Identity]\n");
    content.push_str(&format!("player_index={}\n", config.player_index));
    content.push_str(&format!("account_name={}\n", config.account_name));
    content.push_str(&format!("steam_id={}\n", config.steam_id));
    content.push('\n');

    // [Facepunch] section
    content.push_str("[Facepunch]\n");
    content.push_str(&format!("spoof_identity={}\n", config.settings.spoof_identity));
    content.push_str(&format!("force_valid={}\n", config.settings.force_valid));
    content.push_str(&format!("photon_bypass={}\n", config.settings.photon_bypass));
    content.push('\n');

    // [RuntimePatches] section (only if there are patches)
    if !config.runtime_patches.is_empty() {
        content.push_str("[RuntimePatches]\n");
        for (i, patch) in config.runtime_patches.iter().enumerate() {
            content.push_str(&format!("patch.{}.class={}\n", i, patch.class));
            if !patch.method.is_empty() {
                content.push_str(&format!("patch.{}.method={}\n", i, patch.method));
            }
            if !patch.property.is_empty() {
                content.push_str(&format!("patch.{}.property={}\n", i, patch.property));
            }
            content.push_str(&format!("patch.{}.action={}\n", i, patch.action));
        }
    }

    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::FacepunchSettings;
    use crate::handler::RuntimePatch;

    #[test]
    fn test_generate_config_basic() {
        let config = FacepunchConfig {
            player_index: 0,
            account_name: "TestPlayer".to_string(),
            steam_id: 76561198000000000,
            settings: FacepunchSettings::default(),
            runtime_patches: vec![],
        };
        let content = generate_config_content(&config);

        assert!(content.contains("[Identity]"));
        assert!(content.contains("player_index=0"));
        assert!(content.contains("account_name=TestPlayer"));
        assert!(content.contains("steam_id=76561198000000000"));
        assert!(content.contains("[Facepunch]"));
        assert!(content.contains("spoof_identity=false"));
    }

    #[test]
    fn test_generate_config_with_patches() {
        let config = FacepunchConfig {
            player_index: 1,
            account_name: "Player2".to_string(),
            steam_id: 76561198000000002,
            settings: FacepunchSettings {
                spoof_identity: true,
                force_valid: true,
                photon_bypass: false,
            },
            runtime_patches: vec![RuntimePatch {
                class: "SteamManager".to_string(),
                method: "DoSteam".to_string(),
                property: String::new(),
                action: "force_steam_loaded".to_string(),
            }],
        };
        let content = generate_config_content(&config);

        assert!(content.contains("[RuntimePatches]"));
        assert!(content.contains("patch.0.class=SteamManager"));
        assert!(content.contains("patch.0.method=DoSteam"));
        assert!(content.contains("patch.0.action=force_steam_loaded"));
        assert!(content.contains("spoof_identity=true"));
    }
}
