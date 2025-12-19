// Handler tests

#[cfg(test)]
mod tests {
    use crate::handler::pure::yaml_parser::expand_dot_notation;
    use crate::handler::{scan_handlers, Handler};

    #[test]
    fn test_dot_notation_expansion() {
        let yaml = r#"
name: Test
goldberg.disable_networking: false
goldberg.settings.force_lobby_type.txt: "2"
goldberg.settings.invite_all.txt: ""
"#;
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        println!("Raw YAML: {:#?}", raw);

        let expanded = expand_dot_notation(raw);
        println!("Expanded YAML: {:#?}", expanded);

        // Check the expanded structure
        let map = expanded.as_mapping().unwrap();
        let goldberg = map
            .get(&serde_yaml::Value::String("goldberg".to_string()))
            .expect("goldberg key should exist");
        let goldberg_map = goldberg.as_mapping().expect("goldberg should be a mapping");

        // Print goldberg structure
        println!("Goldberg map: {:#?}", goldberg_map);

        // Check disable_networking
        let disable_net = goldberg_map
            .get(&serde_yaml::Value::String("disable_networking".to_string()))
            .expect("disable_networking should exist");
        assert_eq!(disable_net.as_bool(), Some(false));

        // Check settings - now we need to look at what actually exists
        for (k, v) in goldberg_map.iter() {
            println!("Key: {:?}, Value: {:?}", k, v);
        }
    }

    #[test]
    fn test_handler_load_with_dot_notation() {
        let yaml = r#"
name: TestHandler
exec: test.exe
spec_ver: 3
goldberg.disable_networking: false
goldberg.settings.force_lobby_type.txt: "2"
"#;
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let expanded = expand_dot_notation(raw);
        let handler: Handler = serde_yaml::from_value(expanded).unwrap();

        assert!(handler.goldberg.is_some(), "goldberg should be Some");
        let goldberg = handler.goldberg.unwrap();
        assert_eq!(goldberg.disable_networking, false);
        assert_eq!(
            goldberg.settings.get("force_lobby_type.txt"),
            Some(&"2".to_string())
        );

        println!("Handler loaded: {:?}", handler.name);
    }

    #[test]
    fn test_photon_handler_dot_notation() {
        let yaml = r#"
name: TestPhoton
exec: test.exe
spec_ver: 3
photon.config_path: "AppData/LocalLow/Test/Game/config.cfg"
photon.shared_files:
  - "AppData/LocalLow/Test/Game/shared"
"#;
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let expanded = expand_dot_notation(raw);
        let handler: Handler = serde_yaml::from_value(expanded).unwrap();

        assert!(handler.photon.is_some(), "photon should be Some");
        let photon = handler.photon.unwrap();
        assert_eq!(photon.config_path, "AppData/LocalLow/Test/Game/config.cfg");
        assert_eq!(photon.shared_files.len(), 1);

        println!("Photon handler loaded: {:?}", handler.name);
    }

    #[test]
    fn test_facepunch_handler_dot_notation() {
        let yaml = r#"
name: TestFacepunch
exec: test.x86_64
spec_ver: 3
facepunch.spoof_identity: true
facepunch.force_valid: true
facepunch.photon_bypass: true
"#;
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let expanded = expand_dot_notation(raw);
        let handler: Handler = serde_yaml::from_value(expanded).unwrap();

        assert!(handler.facepunch.is_some(), "facepunch should be Some");
        let facepunch = handler.facepunch.unwrap();
        assert!(facepunch.spoof_identity);
        assert!(facepunch.force_valid);
        assert!(facepunch.photon_bypass);

        println!("Facepunch handler loaded: {:?}", handler.name);
    }

    #[test]
    fn test_load_all_installed_handlers() {
        let handlers = scan_handlers();
        println!("\n=== Loaded {} handlers ===", handlers.len());

        for h in &handlers {
            println!("\n--- {} ---", h.name);
            if let Some(ref goldberg) = h.goldberg {
                println!(
                    "  goldberg.disable_networking: {}",
                    goldberg.disable_networking
                );
                println!("  goldberg.settings: {:?}", goldberg.settings);
            }
            if let Some(ref photon) = h.photon {
                println!("  photon.config_path: {}", photon.config_path);
                println!("  photon.shared_files: {:?}", photon.shared_files);
            }
            if let Some(ref facepunch) = h.facepunch {
                println!("  facepunch.spoof_identity: {}", facepunch.spoof_identity);
                println!("  facepunch.force_valid: {}", facepunch.force_valid);
                println!("  facepunch.photon_bypass: {}", facepunch.photon_bypass);
            }
            if h.goldberg.is_none() && h.photon.is_none() && h.facepunch.is_none() {
                println!("  (no backend configured)");
            }
        }

        // All handlers should load without errors
        assert!(!handlers.is_empty(), "Should have at least one handler");
    }

    #[test]
    fn test_riftbreaker_handler() {
        // Test TheRiftbreaker handler specifically - it only has goldberg.settings.* without goldberg.disable_networking
        let yaml = r#"
name: The Riftbreaker
spec_ver: 3
steam_appid: 780310
exec: bin/riftbreaker_win_release.exe
goldberg.settings.force_lobby_type.txt: "2"
goldberg.settings.invite_all.txt: ""
"#;
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        println!("Raw YAML: {:#?}", raw);

        let expanded = expand_dot_notation(raw);
        println!("Expanded YAML: {:#?}", expanded);

        let handler: Handler = serde_yaml::from_value(expanded).unwrap();

        assert!(
            handler.goldberg.is_some(),
            "goldberg should be Some even with only settings"
        );
        let goldberg = handler.goldberg.unwrap();
        println!("goldberg: {:?}", goldberg);
        assert_eq!(
            goldberg.settings.get("force_lobby_type.txt"),
            Some(&"2".to_string())
        );
    }

    #[test]
    fn test_load_actual_riftbreaker_file() {
        use std::fs::File;
        use std::io::BufReader;
        use std::path::PathBuf;

        let yaml_path = PathBuf::from(std::env::var("HOME").unwrap())
            .join(".local/share/splitux/handlers/TheRiftbreaker/handler.yaml");

        if yaml_path.exists() {
            // Debug: read and print the raw and expanded YAML
            let file = File::open(&yaml_path).unwrap();
            let raw: serde_yaml::Value = serde_yaml::from_reader(BufReader::new(file)).unwrap();
            println!("Raw YAML from file: {:#?}", raw);

            let expanded = expand_dot_notation(raw);
            println!("Expanded YAML: {:#?}", expanded);

            match serde_yaml::from_value::<Handler>(expanded) {
                Ok(handler) => {
                    println!("Loaded TheRiftbreaker: {}", handler.name);
                    println!("  goldberg: {:?}", handler.goldberg);
                    assert!(handler.goldberg.is_some());
                }
                Err(e) => {
                    panic!("Failed to deserialize TheRiftbreaker handler: {}", e);
                }
            }
        } else {
            println!(
                "TheRiftbreaker handler not found at {:?}, skipping test",
                yaml_path
            );
        }
    }
}
