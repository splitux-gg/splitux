// InputDevice struct and poll implementation (I/O: calls fetch_events)

use crate::input::types::{DeviceInfo, DeviceType, PadButton, PollResult, StickDirection};
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
    stick_hold_start: Option<std::time::Instant>,
    stick_hold_direction: Option<StickDirection>,
    stick_last_repeat: std::time::Instant,
    // Axis range info (center and threshold for stick navigation)
    stick_center: i32,
    stick_threshold: i32,
    // Unique identifier (Bluetooth MAC or USB serial) for distinguishing identical controllers
    uniq: String,
}

impl InputDevice {
    pub fn new(
        path: String,
        dev: Device,
        enabled: bool,
        device_type: DeviceType,
        stick_center: i32,
        stick_threshold: i32,
        uniq: String,
    ) -> Self {
        Self {
            path,
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
        }
    }

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
    pub fn poll(&mut self) -> PollResult {
        // Quick check: if device node is gone, disable immediately
        if !std::path::Path::new(&self.path).exists() {
            self.enabled = false;
            return PollResult::DeviceDisabled(format!("device node gone: {}", self.path));
        }

        let mut btn: Option<PadButton> = None;

        // Hold-to-repeat timing constants
        const INITIAL_DELAY_MS: u128 = 300;
        const REPEAT_RATE_MS: u128 = 80;

        const MAX_EVENTS_PER_POLL: usize = 256;

        let mut disabled_reason: Option<String> = None;

        // Process events - update stored stick positions
        match self.dev.fetch_events() {
            Ok(events) => {
                let mut count = 0;
                for event in events {
                    count += 1;
                    if count > MAX_EVENTS_PER_POLL {
                        self.enabled = false;
                        disabled_reason = Some(format!(
                            "exceeded max events for {}, disabling device", self.path
                        ));
                        break;
                    }

                    let summary = event.destructure();

                    match summary {
                        EventSummary::Key(_, _, 1) => {
                            self.has_button_held = true;
                        }
                        EventSummary::Key(_, _, 0) => {
                            self.has_button_held = false;
                        }
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
            Err(e) if e.raw_os_error() == Some(19) => {
                // ENODEV — device disconnected
                self.enabled = false;
                disabled_reason = Some(format!("device disconnected: {}", self.path));
            }
            Err(_) => {}
        }

        // If device was disabled during this poll, report it
        if let Some(reason) = disabled_reason {
            return PollResult::DeviceDisabled(reason);
        }

        // Prioritize button press from event loop
        if let Some(b) = btn {
            return PollResult::Button(b);
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
            return PollResult::Button(result);
        }

        // Handle right stick Y-axis for scrolling (simple cooldown)
        let scroll_up = self.scroll_y < self.stick_center - self.stick_threshold;
        let scroll_down = self.scroll_y > self.stick_center + self.stick_threshold;

        const SCROLL_COOLDOWN_MS: u128 = 100;
        if self.stick_nav_cooldown.elapsed().as_millis() > SCROLL_COOLDOWN_MS {
            if scroll_up {
                self.stick_nav_cooldown = std::time::Instant::now();
                return PollResult::Button(PadButton::ScrollUp);
            } else if scroll_down {
                self.stick_nav_cooldown = std::time::Instant::now();
                return PollResult::Button(PadButton::ScrollDown);
            }
        }

        PollResult::None
    }
}
