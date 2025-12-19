// Permission checking and udev rules installation

/// Result of checking input device permissions
#[derive(Debug, Clone, Default)]
pub struct PermissionStatus {
    /// Number of devices we couldn't access due to permissions
    pub denied_count: usize,
    /// Example paths that were denied (for error messages)
    pub denied_paths: Vec<String>,
    /// Whether udev rules appear to be installed
    pub rules_installed: bool,
}

/// Check for permission issues with gamepad devices specifically
pub fn check_permissions() -> PermissionStatus {
    let mut status = PermissionStatus::default();

    // Check if our udev rules are installed
    status.rules_installed =
        std::path::Path::new("/etc/udev/rules.d/99-splitux-gamepads.rules").exists();

    // Use udevadm to find actual joystick/gamepad devices
    if let Ok(output) = std::process::Command::new("udevadm")
        .args(["info", "--export-db"])
        .output()
    {
        let db = String::from_utf8_lossy(&output.stdout);
        let mut current_node: Option<String> = None;
        let mut is_joystick = false;

        for line in db.lines() {
            if line.starts_with("P: ") {
                // New device entry - check previous one
                if is_joystick {
                    if let Some(ref node) = current_node {
                        let path = format!("/dev/{}", node);
                        match std::fs::File::open(&path) {
                            Ok(_) => {} // Access OK
                            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                                status.denied_count += 1;
                                if status.denied_paths.len() < 3 {
                                    status.denied_paths.push(path);
                                }
                            }
                            Err(_) => {} // Other errors are fine
                        }
                    }
                }
                current_node = None;
                is_joystick = false;
            } else if line.starts_with("N: ") && line.contains("input/event") {
                // Device node name (e.g., "input/event259")
                current_node = Some(line[3..].to_string());
            } else if line == "E: ID_INPUT_JOYSTICK=1" {
                is_joystick = true;
            }
        }

        // Check last device
        if is_joystick {
            if let Some(ref node) = current_node {
                let path = format!("/dev/{}", node);
                match std::fs::File::open(&path) {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        status.denied_count += 1;
                        if status.denied_paths.len() < 3 {
                            status.denied_paths.push(path);
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }

    status
}

/// Embedded udev rules content
pub const UDEV_RULES: &str = r#"# Splitux - Gamepad access rules
# Install: sudo cp 99-splitux-gamepads.rules /etc/udev/rules.d/
# Reload: sudo udevadm control --reload-rules && sudo udevadm trigger

# Generic rule: Allow access to any device with gamepad/joystick capabilities
SUBSYSTEM=="input", KERNEL=="event*", ENV{ID_INPUT_JOYSTICK}=="1", MODE="0666"

# 8BitDo controllers (various models)
SUBSYSTEM=="input", ATTRS{idVendor}=="2dc8", MODE="0666"

# Sony DualShock 3/4 and DualSense
SUBSYSTEM=="input", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="0268", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="05c4", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="09cc", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="0ce6", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="0df2", MODE="0666"

# Microsoft Xbox controllers
SUBSYSTEM=="input", ATTRS{idVendor}=="045e", ATTRS{idProduct}=="028e", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="045e", ATTRS{idProduct}=="028f", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="045e", ATTRS{idProduct}=="02d1", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="045e", ATTRS{idProduct}=="02dd", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="045e", ATTRS{idProduct}=="02e3", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="045e", ATTRS{idProduct}=="02ea", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="045e", ATTRS{idProduct}=="0b12", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="045e", ATTRS{idProduct}=="0b13", MODE="0666"

# Nintendo Switch Pro Controller and Joy-Cons
SUBSYSTEM=="input", ATTRS{idVendor}=="057e", ATTRS{idProduct}=="2009", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="057e", ATTRS{idProduct}=="2006", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="057e", ATTRS{idProduct}=="2007", MODE="0666"

# Logitech, Steam, Google Stadia, PowerA, HORI, Razer, GuliKit
SUBSYSTEM=="input", ATTRS{idVendor}=="046d", ENV{ID_INPUT_JOYSTICK}=="1", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="28de", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="18d1", ATTRS{idProduct}=="9400", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="20d6", ENV{ID_INPUT_JOYSTICK}=="1", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="0f0d", ENV{ID_INPUT_JOYSTICK}=="1", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="1532", ENV{ID_INPUT_JOYSTICK}=="1", MODE="0666"
SUBSYSTEM=="input", ATTRS{idVendor}=="342d", MODE="0666"
"#;

/// Install udev rules using pkexec (graphical sudo prompt)
/// Returns Ok(true) if installed, Ok(false) if user cancelled, Err on failure
pub fn install_udev_rules() -> Result<bool, String> {
    use std::io::Write;
    use std::process::Command;

    // Check if pkexec is available
    if Command::new("which")
        .arg("pkexec")
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        return Err("pkexec not found. Install polkit or run: sudo cp /tmp/99-splitux-gamepads.rules /etc/udev/rules.d/".to_string());
    }

    // Write rules to a temp file
    let temp_path = "/tmp/99-splitux-gamepads.rules";
    println!("[splitux] Writing udev rules to {}", temp_path);
    let mut file = std::fs::File::create(temp_path)
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    file.write_all(UDEV_RULES.as_bytes())
        .map_err(|e| format!("Failed to write rules: {}", e))?;
    drop(file); // Ensure file is flushed and closed

    // Use pkexec to copy to /etc/udev/rules.d/ and reload
    let script = format!(
        "cp {} /etc/udev/rules.d/ && udevadm control --reload-rules && udevadm trigger",
        temp_path
    );
    println!("[splitux] Running: pkexec sh -c '{}'", script);

    let output = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| format!("Failed to run pkexec: {}", e))?;

    println!("[splitux] pkexec exit code: {:?}", output.status.code());
    if !output.stdout.is_empty() {
        println!(
            "[splitux] pkexec stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    if !output.stderr.is_empty() {
        println!(
            "[splitux] pkexec stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Clean up temp file
    let _ = std::fs::remove_file(temp_path);

    if output.status.success() {
        Ok(true)
    } else if output.status.code() == Some(126) || output.status.code() == Some(127) {
        // 126 = user cancelled, 127 = command not found
        Ok(false)
    } else {
        Err(format!(
            "Installation failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
