//! Instance page input handling

mod device_dispatch;
mod keyboard;
mod navigation;

use crate::app::app::{ActiveDropdown, Splitux};

impl Splitux {
    /// Check if an instance dropdown is currently open
    fn is_instance_dropdown_open(&self) -> bool {
        matches!(
            self.active_dropdown,
            Some(ActiveDropdown::InstanceProfile(_))
                | Some(ActiveDropdown::InstanceMonitor(_))
                | Some(ActiveDropdown::InstanceAudioOverride(_))
                | Some(ActiveDropdown::InstanceAudioPreference(_))
        )
    }
}
