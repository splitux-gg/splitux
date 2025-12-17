use crate::app::PadFilterType;

use evdev::*;
use std::os::unix::io::AsRawFd;

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum DeviceType {
    Gamepad,
    Keyboard,
    Mouse,
    Other,
}

pub enum PadButton {
    Left,
    Right,
    Up,
    Down,
    ABtn,
    BBtn,
    XBtn,
    YBtn,
    StartBtn,
    SelectBtn,
    LB, // Left bumper (BTN_TL)
    RB, // Right bumper (BTN_TR)
    LT, // Left trigger (BTN_TL2 or ABS_Z)
    RT, // Right trigger (BTN_TR2 or ABS_RZ)

    ScrollUp,   // Right stick up
    ScrollDown, // Right stick down

    AKey,
    RKey,
    XKey,
    ZKey,

    RightClick,
}

/// Snapshot of device state for passing to launch functions
#[derive(Clone)]
pub struct DeviceInfo {
    pub path: String,
    #[allow(dead_code)] // Used by bwrap for device filtering
    pub enabled: bool,
    pub device_type: DeviceType,
    pub uniq: String, // Unique identifier (Bluetooth MAC or USB serial)
}

pub struct InputDevice {
    path: String,
    dev: Device,
    enabled: bool,
    device_type: DeviceType,
    has_button_held: bool,
    // Track analog stick state for navigation (with deadzone/cooldown)
    stick_nav_cooldown: std::time::Instant,
    // Current stick positions (updated by events, persisted between polls)
    stick_x: i32,
    stick_y: i32,
    scroll_y: i32, // Right stick Y
    // Track stick hold state for repeat behavior
    stick_hold_start: Option<std::time::Instant>, // When stick was first pushed (None = not held)
    stick_hold_direction: Option<StickDirection>, // Current held direction
    stick_last_repeat: std::time::Instant,        // Time of last repeat event
    // Axis range info (center and threshold for stick navigation)
    stick_center: i32,
    stick_threshold: i32,
    // Unique identifier (Bluetooth MAC or USB serial) for distinguishing identical controllers
    uniq: String,
}

#[derive(Clone, Copy, PartialEq)]
enum StickDirection {
    Up,
    Down,
    Left,
    Right,
}

impl InputDevice {
    /// Handle stick direction with hold-to-repeat behavior
    fn handle_stick_direction(
        &mut self,
        new_dir: Option<StickDirection>,
        initial_delay_ms: u128,
        repeat_rate_ms: u128,
    ) -> Option<PadButton> {
        let now = std::time::Instant::now();

        match (new_dir, self.stick_hold_direction) {
            // Started holding a new direction
            (Some(dir), None) | (Some(dir), Some(_)) if new_dir != self.stick_hold_direction => {
                self.stick_hold_start = Some(now);
                self.stick_hold_direction = Some(dir);
                self.stick_last_repeat = now;
                Some(Self::direction_to_button(dir))
            }
            // Still holding the same direction - check for repeat
            (Some(dir), Some(held_dir)) if dir == held_dir => {
                if let Some(hold_start) = self.stick_hold_start {
                    let hold_duration = now.duration_since(hold_start).as_millis();
                    let since_last_repeat = now.duration_since(self.stick_last_repeat).as_millis();

                    // After initial delay, use repeat rate
                    if hold_duration > initial_delay_ms && since_last_repeat > repeat_rate_ms {
                        self.stick_last_repeat = now;
                        Some(Self::direction_to_button(dir))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            // Released stick (back to center)
            (None, Some(_)) => {
                self.stick_hold_start = None;
                self.stick_hold_direction = None;
                None
            }
            // Already at center, nothing held
            (None, None) => None,
            // Catch-all
            _ => None,
        }
    }

    fn direction_to_button(dir: StickDirection) -> PadButton {
        match dir {
            StickDirection::Up => PadButton::Up,
            StickDirection::Down => PadButton::Down,
            StickDirection::Left => PadButton::Left,
            StickDirection::Right => PadButton::Right,
        }
    }

    pub fn name(&self) -> &str {
        self.dev.name().unwrap_or_else(|| "")
    }
    pub fn emoji(&self) -> &str {
        match self.device_type() {
            DeviceType::Gamepad => "🎮",
            DeviceType::Keyboard => "🖮",
            DeviceType::Mouse => "🖱",
            DeviceType::Other => "",
        }
    }
    /// Returns the actual evdev device name (e.g., "Microsoft X-Box One S pad")
    pub fn fancyname(&self) -> &str {
        self.name()
    }

    /// Returns a short type prefix for the controller type
    pub fn type_prefix(&self) -> &str {
        match self.dev.input_id().vendor() {
            0x045e => "Xbox",
            0x054c => "PS",
            0x057e => "Switch",
            0x28de => "Steam",
            _ => "",
        }
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn device_type(&self) -> DeviceType {
        self.device_type
    }
    pub fn has_button_held(&self) -> bool {
        self.has_button_held
    }
    #[allow(dead_code)] // API for future device matching
    pub fn uniq(&self) -> &str {
        &self.uniq
    }
    pub fn info(&self) -> DeviceInfo {
        DeviceInfo {
            path: self.path().to_string(),
            enabled: self.enabled(),
            device_type: self.device_type(),
            uniq: self.uniq.clone(),
        }
    }
    pub fn poll(&mut self) -> Option<PadButton> {
        let mut btn: Option<PadButton> = None;

        // Hold-to-repeat timing constants
        const INITIAL_DELAY_MS: u128 = 300; // Delay before first repeat
        const REPEAT_RATE_MS: u128 = 80;    // Time between repeats

        // Process events - update stored stick positions
        if let Ok(events) = self.dev.fetch_events() {
            for event in events {
                let summary = event.destructure();

                match summary {
                    EventSummary::Key(_, _, 1) => {
                        self.has_button_held = true;
                    }
                    EventSummary::Key(_, _, 0) => {
                        self.has_button_held = false;
                    }
                    // Update stored stick positions (persisted between polls)
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_X, val) => {
                        self.stick_x = val;
                    }
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_Y, val) => {
                        self.stick_y = val;
                    }
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_RY, val) => {
                        self.scroll_y = val;
                    }
                    _ => {}
                }

                btn = match summary {
                    EventSummary::Key(_, KeyCode::BTN_SOUTH, 1) => Some(PadButton::ABtn),
                    EventSummary::Key(_, KeyCode::BTN_EAST, 1) => Some(PadButton::BBtn),
                    EventSummary::Key(_, KeyCode::BTN_NORTH, 1) => Some(PadButton::XBtn),
                    EventSummary::Key(_, KeyCode::BTN_WEST, 1) => Some(PadButton::YBtn),
                    EventSummary::Key(_, KeyCode::BTN_START, 1) => Some(PadButton::StartBtn),
                    EventSummary::Key(_, KeyCode::BTN_SELECT, 1) => Some(PadButton::SelectBtn),
                    EventSummary::Key(_, KeyCode::BTN_TL, 1) => Some(PadButton::LB),
                    EventSummary::Key(_, KeyCode::BTN_TR, 1) => Some(PadButton::RB),
                    EventSummary::Key(_, KeyCode::BTN_TL2, 1) => Some(PadButton::LT),
                    EventSummary::Key(_, KeyCode::BTN_TR2, 1) => Some(PadButton::RT),
                    // D-pad
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_HAT0X, -1) => {
                        Some(PadButton::Left)
                    }
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_HAT0X, 1) => {
                        Some(PadButton::Right)
                    }
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_HAT0Y, -1) => {
                        Some(PadButton::Up)
                    }
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_HAT0Y, 1) => {
                        Some(PadButton::Down)
                    }
                    // Keyboard
                    EventSummary::Key(_, KeyCode::KEY_A, 1) => Some(PadButton::AKey),
                    EventSummary::Key(_, KeyCode::KEY_R, 1) => Some(PadButton::RKey),
                    EventSummary::Key(_, KeyCode::KEY_X, 1) => Some(PadButton::XKey),
                    EventSummary::Key(_, KeyCode::KEY_Z, 1) => Some(PadButton::ZKey),
                    // Mouse
                    EventSummary::Key(_, KeyCode::BTN_RIGHT, 1) => Some(PadButton::RightClick),
                    _ => btn,
                };
            }
        }

        // Process stick input with hold-to-repeat (check stored positions every poll)
        // Prioritize any button press from the event loop
        if btn.is_some() {
            return btn;
        }

        // Determine current stick direction from stored position
        let is_left = self.stick_x < self.stick_center - self.stick_threshold;
        let is_right = self.stick_x > self.stick_center + self.stick_threshold;
        let is_up = self.stick_y < self.stick_center - self.stick_threshold;
        let is_down = self.stick_y > self.stick_center + self.stick_threshold;

        let new_dir = if is_up {
            Some(StickDirection::Up)
        } else if is_down {
            Some(StickDirection::Down)
        } else if is_left {
            Some(StickDirection::Left)
        } else if is_right {
            Some(StickDirection::Right)
        } else {
            None
        };

        if let Some(result) = self.handle_stick_direction(new_dir, INITIAL_DELAY_MS, REPEAT_RATE_MS) {
            return Some(result);
        }

        // Handle right stick Y-axis for scrolling (simple cooldown)
        let scroll_up = self.scroll_y < self.stick_center - self.stick_threshold;
        let scroll_down = self.scroll_y > self.stick_center + self.stick_threshold;

        const SCROLL_COOLDOWN_MS: u128 = 100;
        if self.stick_nav_cooldown.elapsed().as_millis() > SCROLL_COOLDOWN_MS {
            if scroll_up {
                self.stick_nav_cooldown = std::time::Instant::now();
                return Some(PadButton::ScrollUp);
            } else if scroll_down {
                self.stick_nav_cooldown = std::time::Instant::now();
                return Some(PadButton::ScrollDown);
            }
        }

        btn
    }
}

pub fn scan_input_devices(filter: &PadFilterType) -> Vec<InputDevice> {
    let mut pads: Vec<InputDevice> = Vec::new();
    for dev in evdev::enumerate() {
        let enabled = match filter {
            PadFilterType::All => true,
            PadFilterType::NoSteamInput => dev.1.input_id().vendor() != 0x28de,
            PadFilterType::OnlySteamInput => dev.1.input_id().vendor() == 0x28de,
        };

        let device_type = if dev
            .1
            .supported_keys()
            .map_or(false, |keys| keys.contains(KeyCode::BTN_SOUTH))
        {
            DeviceType::Gamepad
        } else if dev
            .1
            .supported_keys()
            .map_or(false, |keys| keys.contains(KeyCode::BTN_LEFT))
        {
            DeviceType::Mouse
        } else if dev
            .1
            .supported_keys()
            .map_or(false, |keys| keys.contains(KeyCode::KEY_SPACE))
        {
            DeviceType::Keyboard
        } else {
            DeviceType::Other
        };

        if device_type != DeviceType::Other {
            if dev.1.set_nonblocking(true).is_err() {
                println!(
                    "[splitux] evdev: Failed to set non-blocking mode for {}",
                    dev.0.display()
                );
                continue;
            }

            // Detect stick axis range from device info
            let (stick_center, stick_threshold) = if let Ok(abs_info) = dev.1.get_abs_state() {
                // Try to get ABS_X info for stick range
                if let Some(x_info) = abs_info.get(AbsoluteAxisCode::ABS_X.0 as usize) {
                    let min = x_info.minimum;
                    let max = x_info.maximum;
                    let center = (min + max) / 2;
                    let range = max - min;
                    let threshold = range / 4; // 25% deadzone
                    println!("[splitux] evdev: {} stick range: {}-{}, center={}, threshold={}",
                        dev.0.display(), min, max, center, threshold);
                    (center, threshold)
                } else {
                    // Default to signed 16-bit range
                    (0, 8000)
                }
            } else {
                // Default to signed 16-bit range
                (0, 8000)
            };

            // Get the unique identifier (Bluetooth MAC or USB serial)
            let uniq = dev.1.unique_name().unwrap_or("").to_string();

            pads.push(InputDevice {
                path: dev.0.to_str().unwrap().to_string(),
                dev: dev.1,
                enabled,
                device_type,
                has_button_held: false,
                stick_nav_cooldown: std::time::Instant::now(),
                stick_x: stick_center,
                stick_y: stick_center,
                scroll_y: stick_center,
                stick_hold_start: None,
                stick_hold_direction: None,
                stick_last_repeat: std::time::Instant::now(),
                stick_center,
                stick_threshold,
                uniq,
            });
        }
    }
    pads.sort_by_key(|pad| pad.path().to_string());
    pads
}

/// Event types for device hotplug
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Added(String),   // Device path added (e.g., "/dev/input/event5")
    Removed(String), // Device path removed
}

/// Monitors for input device connect/disconnect events via udev
pub struct DeviceMonitor {
    socket: udev::MonitorSocket,
}

impl DeviceMonitor {
    /// Create a new device monitor watching for input device events
    pub fn new() -> Result<Self, std::io::Error> {
        let socket = udev::MonitorBuilder::new()?
            .match_subsystem("input")?
            .listen()?;

        // Set non-blocking mode using libc
        unsafe {
            let fd = socket.as_raw_fd();
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        Ok(Self { socket })
    }

    /// Poll for device events (non-blocking)
    /// Returns a list of events that occurred since last poll
    pub fn poll_events(&mut self) -> Vec<DeviceEvent> {
        let mut events = Vec::new();

        // Use iter() which returns events non-blockingly
        for event in self.socket.iter() {
            // Only care about "event" devices (not js*, mouse*, etc.)
            if let Some(devnode) = event.devnode() {
                let path = devnode.to_string_lossy().to_string();
                if path.contains("/dev/input/event") {
                    match event.event_type() {
                        udev::EventType::Add => {
                            events.push(DeviceEvent::Added(path));
                        }
                        udev::EventType::Remove => {
                            events.push(DeviceEvent::Removed(path));
                        }
                        _ => {}
                    }
                }
            }
        }

        events
    }
}

/// Try to open a single device by path and create an InputDevice
pub fn open_device(path: &str, filter: &PadFilterType) -> Option<InputDevice> {
    // Retry with exponential backoff - udev events can arrive before device is ready
    // and permission changes (udev rules) may take time to apply
    let dev = {
        let mut attempts = 0;
        let max_attempts = 8;
        let mut delay_ms = 50u64;
        loop {
            match Device::open(path) {
                Ok(d) => break d,
                Err(e) => {
                    attempts += 1;
                    let is_permission_error = e.kind() == std::io::ErrorKind::PermissionDenied;

                    if attempts >= max_attempts {
                        if is_permission_error {
                            println!("[splitux] evdev: Permission denied for {} - ensure your user is in the 'input' group (run: sudo usermod -aG input $USER)", path);
                        } else {
                            println!("[splitux] evdev: Failed to open {} after {} attempts: {}", path, attempts, e);
                        }
                        return None;
                    }

                    // Use longer delays for permission errors (udev rules may be slow)
                    let wait_time = if is_permission_error {
                        delay_ms * 2
                    } else {
                        delay_ms
                    };
                    std::thread::sleep(std::time::Duration::from_millis(wait_time));
                    delay_ms = (delay_ms * 2).min(500); // Exponential backoff, max 500ms
                }
            }
        }
    };

    let enabled = match filter {
        PadFilterType::All => true,
        PadFilterType::NoSteamInput => dev.input_id().vendor() != 0x28de,
        PadFilterType::OnlySteamInput => dev.input_id().vendor() == 0x28de,
    };

    let device_type = if dev
        .supported_keys()
        .map_or(false, |keys| keys.contains(KeyCode::BTN_SOUTH))
    {
        DeviceType::Gamepad
    } else if dev
        .supported_keys()
        .map_or(false, |keys| keys.contains(KeyCode::BTN_LEFT))
    {
        DeviceType::Mouse
    } else if dev
        .supported_keys()
        .map_or(false, |keys| keys.contains(KeyCode::KEY_SPACE))
    {
        DeviceType::Keyboard
    } else {
        println!("[splitux] evdev: Skipping {} - not a gamepad/keyboard/mouse", path);
        return None; // Not an input device we care about
    };

    if dev.set_nonblocking(true).is_err() {
        println!("[splitux] evdev: Failed to set non-blocking mode for {}", path);
        return None;
    }

    // Detect stick axis range from device info
    let (stick_center, stick_threshold) = if let Ok(abs_info) = dev.get_abs_state() {
        if let Some(x_info) = abs_info.get(AbsoluteAxisCode::ABS_X.0 as usize) {
            let min = x_info.minimum;
            let max = x_info.maximum;
            let center = (min + max) / 2;
            let range = max - min;
            let threshold = range / 4;
            println!(
                "[splitux] evdev: {} stick range: {}-{}, center={}, threshold={}",
                path, min, max, center, threshold
            );
            (center, threshold)
        } else {
            (0, 8000)
        }
    } else {
        (0, 8000)
    };

    let uniq = dev.unique_name().unwrap_or("").to_string();

    Some(InputDevice {
        path: path.to_string(),
        dev,
        enabled,
        device_type,
        has_button_held: false,
        stick_nav_cooldown: std::time::Instant::now(),
        stick_x: stick_center,
        stick_y: stick_center,
        scroll_y: stick_center,
        stick_hold_start: None,
        stick_hold_direction: None,
        stick_last_repeat: std::time::Instant::now(),
        stick_center,
        stick_threshold,
        uniq,
    })
}

/// Find a device index by its unique identifier (Bluetooth MAC or USB serial)
/// Returns None if no device matches or the uniq is empty
pub fn find_device_by_uniq(devices: &[InputDevice], uniq: &str) -> Option<usize> {
    if uniq.is_empty() {
        return None;
    }
    devices.iter().position(|d| d.uniq == uniq && !d.uniq.is_empty())
}

/// Get the Bluetooth alias for a device by its MAC address
/// Returns None if not a Bluetooth device or no alias is set
fn get_bluetooth_alias(mac_address: &str) -> Option<String> {
    // MAC address format check (e.g., "e4:17:d8:27:2d:f0")
    if mac_address.len() != 17 || mac_address.chars().filter(|&c| c == ':').count() != 5 {
        return None;
    }

    // Try to get alias from bluetoothctl
    let output = std::process::Command::new("bluetoothctl")
        .args(["info", mac_address])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the Alias line
    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("Alias:") {
            let alias = line.trim_start_matches("Alias:").trim();
            // Only return if alias differs from the generic device name
            // (Bluetooth sets Alias = Name by default if user hasn't customized it)
            if !alias.is_empty() {
                return Some(alias.to_string());
            }
        }
    }

    None
}

/// Generate display names for all devices
/// Priority: 1) User-defined alias, 2) Bluetooth alias, 3) evdev name
/// Adds "(1)", "(2)" suffixes for duplicates without unique aliases
/// Returns a Vec of display names in the same order as the input devices
pub fn generate_display_names(
    devices: &[InputDevice],
    user_aliases: &std::collections::HashMap<String, String>,
) -> Vec<String> {
    use std::collections::HashMap;

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
            // 3) Fall back to evdev name
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

/// Check if a device is already assigned to any instance
pub fn is_device_assigned(device_idx: usize, instances: &[crate::instance::Instance]) -> bool {
    instances.iter().any(|inst| inst.devices.contains(&device_idx))
}

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
    if Command::new("which").arg("pkexec").output().map(|o| !o.status.success()).unwrap_or(true) {
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
        println!("[splitux] pkexec stdout: {}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        println!("[splitux] pkexec stderr: {}", String::from_utf8_lossy(&output.stderr));
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
