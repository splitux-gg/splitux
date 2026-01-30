// Pure validation functions for Handler data (no I/O)

use std::error::Error;

/// Trim whitespace from all string fields of a handler.
/// Takes mutable references to each field to trim.
pub fn trim_field(field: &mut String) {
    *field = field.trim().to_string();
}

/// Validate that required handler fields are present
pub fn validate_handler(name: &str, exec: &str) -> Result<(), Box<dyn Error>> {
    if name.is_empty() {
        return Err("Handler 'name' is required".into());
    }
    if exec.is_empty() {
        return Err("Handler 'exec' (executable path) is required".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── trim_field ──────────────────────────────────────────────

    #[test]
    fn trim_field_leading_spaces() {
        let mut s = "  hello".to_string();
        trim_field(&mut s);
        assert_eq!(s, "hello");
    }

    #[test]
    fn trim_field_trailing_spaces() {
        let mut s = "hello  ".to_string();
        trim_field(&mut s);
        assert_eq!(s, "hello");
    }

    #[test]
    fn trim_field_both_sides() {
        let mut s = "  hello  ".to_string();
        trim_field(&mut s);
        assert_eq!(s, "hello");
    }

    #[test]
    fn trim_field_tabs_and_newlines() {
        let mut s = "\t\n hello \n\t".to_string();
        trim_field(&mut s);
        assert_eq!(s, "hello");
    }

    #[test]
    fn trim_field_already_trimmed() {
        let mut s = "hello".to_string();
        trim_field(&mut s);
        assert_eq!(s, "hello");
    }

    #[test]
    fn trim_field_empty_string() {
        let mut s = String::new();
        trim_field(&mut s);
        assert_eq!(s, "");
    }

    // ── validate_handler ────────────────────────────────────────

    #[test]
    fn validate_handler_both_valid() {
        assert!(validate_handler("My Game", "/usr/bin/game").is_ok());
    }

    #[test]
    fn validate_handler_empty_name() {
        let err = validate_handler("", "/usr/bin/game").unwrap_err();
        assert_eq!(err.to_string(), "Handler 'name' is required");
    }

    #[test]
    fn validate_handler_empty_exec() {
        let err = validate_handler("My Game", "").unwrap_err();
        assert_eq!(err.to_string(), "Handler 'exec' (executable path) is required");
    }

    #[test]
    fn validate_handler_both_empty_name_error_first() {
        let err = validate_handler("", "").unwrap_err();
        assert_eq!(err.to_string(), "Handler 'name' is required");
    }

    #[test]
    fn validate_handler_whitespace_only_name_passes() {
        // trim is not called inside validate_handler, so whitespace-only passes
        assert!(validate_handler("  ", "/usr/bin/game").is_ok());
    }

    #[test]
    fn validate_handler_whitespace_only_exec_passes() {
        assert!(validate_handler("My Game", "  ").is_ok());
    }
}
