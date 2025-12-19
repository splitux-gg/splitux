// Device hotplug monitoring via udev

use std::os::unix::io::AsRawFd;

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
