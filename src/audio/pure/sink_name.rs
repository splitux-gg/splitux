//! Sink name generation and parsing
//!
//! Pure functions for working with audio sink names.

/// Generate a virtual sink name for an instance within a launch namespace.
///
/// The `ns` (the launch_id, `<pid>_<counter>`) is embedded so CONCURRENT splitux
/// processes never collide on `splitux_instance_0`, and so [`parse_sink_owner_pid`]
/// can recover the owning launch's pid to reap a crashed session's leftovers
/// without touching a live concurrent session's sinks.
pub fn generate_virtual_sink_name(ns: &str, instance_idx: usize) -> String {
    format!("splitux_instance_{}_{}", ns, instance_idx)
}

/// Recover the owning launch's pid from a splitux sink name. The name carries
/// `splitux_instance_<pid>_<counter>_<idx>` (ns = `<pid>_<counter>`), so the pid
/// is the first segment after the prefix. None if `name` isn't a splitux sink or
/// has no numeric leading segment.
pub fn parse_sink_owner_pid(name: &str) -> Option<u32> {
    name.strip_prefix("splitux_instance_")?
        .split('_')
        .next()?
        .parse()
        .ok()
}

/// Generate a human-readable description for a virtual sink
pub fn generate_virtual_sink_description(instance_idx: usize) -> String {
    format!("Splitux Instance {} Audio", instance_idx + 1)
}

/// Parse module ID from pactl load-module output
///
/// pactl load-module returns just the module ID number on success
pub fn parse_module_id(output: &str) -> Option<String> {
    let trimmed = output.trim();
    if trimmed.chars().all(|c| c.is_ascii_digit()) && !trimmed.is_empty() {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Check if a sink name is a splitux virtual sink
pub fn is_splitux_sink(name: &str) -> bool {
    name.starts_with("splitux_instance_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_virtual_sink_name() {
        assert_eq!(
            generate_virtual_sink_name("12345_0", 0),
            "splitux_instance_12345_0_0"
        );
        assert_eq!(
            generate_virtual_sink_name("12345_0", 3),
            "splitux_instance_12345_0_3"
        );
    }

    #[test]
    fn test_parse_sink_owner_pid() {
        // ns = "<pid>_<counter>" → name carries the pid as the first segment.
        assert_eq!(
            parse_sink_owner_pid("splitux_instance_12345_0_3"),
            Some(12345)
        );
        // ns fallback is a bare pid (no counter).
        assert_eq!(parse_sink_owner_pid("splitux_instance_999_2"), Some(999));
        // Non-splitux sinks and malformed names yield None.
        assert_eq!(parse_sink_owner_pid("alsa_output.pci-0000"), None);
        assert_eq!(parse_sink_owner_pid("splitux_instance_abc_0"), None);
    }

    #[test]
    fn test_generate_virtual_sink_description() {
        assert_eq!(
            generate_virtual_sink_description(0),
            "Splitux Instance 1 Audio"
        );
        assert_eq!(
            generate_virtual_sink_description(1),
            "Splitux Instance 2 Audio"
        );
    }

    #[test]
    fn test_parse_module_id() {
        assert_eq!(parse_module_id("42"), Some("42".to_string()));
        assert_eq!(parse_module_id("42\n"), Some("42".to_string()));
        assert_eq!(parse_module_id("  123  "), Some("123".to_string()));
        assert_eq!(parse_module_id("invalid"), None);
        assert_eq!(parse_module_id(""), None);
    }

    #[test]
    fn test_is_splitux_sink() {
        assert!(is_splitux_sink("splitux_instance_0"));
        assert!(is_splitux_sink("splitux_instance_12"));
        assert!(!is_splitux_sink("alsa_output.pci-0000"));
        assert!(!is_splitux_sink("splitux_other"));
    }
}
