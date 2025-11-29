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
    // Axis range info (center and threshold for stick navigation)
    stick_center: i32,
    stick_threshold: i32,
    // Unique identifier (Bluetooth MAC or USB serial) for distinguishing identical controllers
    uniq: String,
}
impl InputDevice {
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
    pub fn fancyname(&self) -> &str {
        match self.dev.input_id().vendor() {
            0x045e => "Xbox Controller",
            0x054c => "PS Controller",
            0x057e => "NT Pro Controller",
            0x28de => "Steam Input",
            _ => self.name(),
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

        // Cooldown for analog stick navigation (150ms between inputs)
        const STICK_NAV_COOLDOWN_MS: u128 = 150;

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
                    // Left analog stick (with deadzone and cooldown)
                    // Uses per-device center and threshold detected at scan time
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_X, val) => {
                        let is_left = val < self.stick_center - self.stick_threshold;
                        let is_right = val > self.stick_center + self.stick_threshold;

                        if self.stick_nav_cooldown.elapsed().as_millis() > STICK_NAV_COOLDOWN_MS {
                            if is_left {
                                self.stick_nav_cooldown = std::time::Instant::now();
                                Some(PadButton::Left)
                            } else if is_right {
                                self.stick_nav_cooldown = std::time::Instant::now();
                                Some(PadButton::Right)
                            } else {
                                btn
                            }
                        } else {
                            btn
                        }
                    }
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_Y, val) => {
                        let is_up = val < self.stick_center - self.stick_threshold;
                        let is_down = val > self.stick_center + self.stick_threshold;

                        if self.stick_nav_cooldown.elapsed().as_millis() > STICK_NAV_COOLDOWN_MS {
                            if is_up {
                                self.stick_nav_cooldown = std::time::Instant::now();
                                Some(PadButton::Up)
                            } else if is_down {
                                self.stick_nav_cooldown = std::time::Instant::now();
                                Some(PadButton::Down)
                            } else {
                                btn
                            }
                        } else {
                            btn
                        }
                    }
                    // Right analog stick Y-axis for scrolling
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_RY, val) => {
                        let is_up = val < self.stick_center - self.stick_threshold;
                        let is_down = val > self.stick_center + self.stick_threshold;

                        if self.stick_nav_cooldown.elapsed().as_millis() > STICK_NAV_COOLDOWN_MS {
                            if is_up {
                                self.stick_nav_cooldown = std::time::Instant::now();
                                Some(PadButton::ScrollUp)
                            } else if is_down {
                                self.stick_nav_cooldown = std::time::Instant::now();
                                Some(PadButton::ScrollDown)
                            } else {
                                btn
                            }
                        } else {
                            btn
                        }
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
    // Retry a few times - udev events can arrive before device is ready
    let dev = {
        let mut attempts = 0;
        loop {
            match Device::open(path) {
                Ok(d) => break d,
                Err(e) => {
                    attempts += 1;
                    if attempts >= 5 {
                        println!("[splitux] evdev: Failed to open {} after {} attempts: {}", path, attempts, e);
                        return None;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
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
        stick_center,
        stick_threshold,
        uniq,
    })
}
