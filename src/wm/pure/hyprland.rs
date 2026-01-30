// Pure functions for Hyprland command building (no I/O)

/// Build setprop commands for visual properties on a window address.
pub fn build_window_prop_commands(address: &str) -> Vec<String> {
    vec![
        format!("setprop address:{} forcenoblur 1 lock", address),
        format!("setprop address:{} forceopaque 1 lock", address),
        format!("setprop address:{} forcenoanims 1 lock", address),
        format!("setprop address:{} forcenoborder 1 lock", address),
        format!("setprop address:{} forcenoshadow 1 lock", address),
        format!("setprop address:{} alpha 1.0 lock", address),
        format!("setprop address:{} alphainactive 1.0 lock", address),
    ]
}

/// Build window rules for gamescope windows on a target monitor.
pub fn build_window_rules(target_monitor: &str) -> Vec<String> {
    let class_match = "class:^([Gg]amescope.*)$";
    vec![
        format!("keyword windowrulev2 float,{}", class_match),
        format!("keyword windowrulev2 noborder,{}", class_match),
        format!("keyword windowrulev2 noblur,{}", class_match),
        format!("keyword windowrulev2 noshadow,{}", class_match),
        format!("keyword windowrulev2 noanim,{}", class_match),
        format!("keyword windowrulev2 opaque,{}", class_match),
        format!("keyword windowrulev2 nodim,{}", class_match),
        format!("keyword windowrulev2 forcergbx,{}", class_match),
        format!("keyword windowrulev2 pin,{}", class_match),
        format!(
            "keyword windowrulev2 monitor {},{}",
            target_monitor, class_match
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prop_commands_count_is_seven() {
        let cmds = build_window_prop_commands("0xabc123");
        assert_eq!(cmds.len(), 7);
    }

    #[test]
    fn prop_commands_embed_address() {
        let addr = "0xdeadbeef";
        let cmds = build_window_prop_commands(addr);
        for cmd in &cmds {
            assert!(
                cmd.contains(&format!("address:{}", addr)),
                "command missing address: {}",
                cmd
            );
        }
    }

    #[test]
    fn prop_commands_contain_all_properties() {
        let cmds = build_window_prop_commands("0x1");
        let joined = cmds.join("\n");
        for prop in &[
            "forcenoblur",
            "forceopaque",
            "forcenoanims",
            "forcenoborder",
            "forcenoshadow",
            "alpha",
            "alphainactive",
        ] {
            assert!(joined.contains(prop), "missing property: {}", prop);
        }
    }

    #[test]
    fn window_rules_count_is_ten() {
        let rules = build_window_rules("DP-1");
        assert_eq!(rules.len(), 10);
    }

    #[test]
    fn window_rules_monitor_includes_target() {
        let rules = build_window_rules("HDMI-A-1");
        let monitor_rule = rules.iter().find(|r| r.contains("monitor HDMI-A-1"));
        assert!(monitor_rule.is_some(), "no monitor rule with target monitor");
    }

    #[test]
    fn window_rules_all_contain_class_match() {
        let rules = build_window_rules("DP-2");
        for rule in &rules {
            assert!(
                rule.contains("class:^([Gg]amescope.*)$"),
                "rule missing class match: {}",
                rule
            );
        }
    }

    #[test]
    fn window_rules_contain_all_rule_types() {
        let rules = build_window_rules("DP-1");
        let joined = rules.join("\n");
        for keyword in &[
            "float",
            "noborder",
            "noblur",
            "noshadow",
            "noanim",
            "opaque",
            "nodim",
            "forcergbx",
            "pin",
            "monitor",
        ] {
            assert!(joined.contains(keyword), "missing rule type: {}", keyword);
        }
    }
}
