// Input device types

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

#[derive(Clone, Copy, PartialEq)]
pub enum StickDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Result of polling an input device
pub enum PollResult {
    /// A button was pressed
    Button(PadButton),
    /// Device was disabled (with reason for logging at the app layer)
    DeviceDisabled(String),
    /// No input
    None,
}
