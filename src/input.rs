// Input device management module

pub mod operations;
pub mod pipelines;
pub mod pure;
pub mod types;

// Re-export types
pub use types::{DeviceInfo, DeviceType, PadButton, StickDirection};

// Re-export operations
pub use operations::{DeviceEvent, DeviceMonitor};

// Re-export pure functions
pub use pure::generate_display_names;

// Re-export pipelines
pub use pipelines::{check_permissions, install_udev_rules, PermissionStatus};

use crate::app::PadFilterType;
use egui_phosphor::regular as icons;
use evdev::*;

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
        self.dev.name().unwrap_or("")
    }
    pub fn emoji(&self) -> &str {
        match self.device_type() {
            DeviceType::Gamepad => icons::GAME_CONTROLLER,
            DeviceType::Keyboard => icons::KEYBOARD,
            DeviceType::Mouse => icons::MOUSE,
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
        const REPEAT_RATE_MS: u128 = 80; // Time between repeats

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

        if let Some(result) =
            self.handle_stick_direction(new_dir, INITIAL_DELAY_MS, REPEAT_RATE_MS)
        {
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
                    println!(
                        "[splitux] evdev: {} stick range: {}-{}, center={}, threshold={}",
                        dev.0.display(),
                        min,
                        max,
                        center,
                        threshold
                    );
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
                            println!(
                                "[splitux] evdev: Failed to open {} after {} attempts: {}",
                                path, attempts, e
                            );
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
        println!(
            "[splitux] evdev: Skipping {} - not a gamepad/keyboard/mouse",
            path
        );
        return None; // Not an input device we care about
    };

    if dev.set_nonblocking(true).is_err() {
        println!(
            "[splitux] evdev: Failed to set non-blocking mode for {}",
            path
        );
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
    devices
        .iter()
        .position(|d| d.uniq == uniq && !d.uniq.is_empty())
}

// get_bluetooth_alias moved to operations/bluetooth.rs
// generate_display_names moved to pure/display_names.rs

/// Check if a device is already assigned to any instance
pub fn is_device_assigned(device_idx: usize, instances: &[crate::instance::Instance]) -> bool {
    instances
        .iter()
        .any(|inst| inst.devices.contains(&device_idx))
}
