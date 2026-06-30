// Instance device management functions

use super::app::Splitux;

impl Splitux {
    /// Whether to expose the per-player Display picker (and its focus slot).
    ///
    /// Shown when there are multiple monitors AND the active stack can actually
    /// place a window on a chosen display: niri (via IPC — works regardless of
    /// the SDL backend) or the gamescope SDL backend (via `--display-index`).
    /// Used by BOTH the card render and the focus chain so focus never lands on
    /// a hidden Monitor element. Replaces the old bare `gamescope_sdl_backend`
    /// gate, which hid display assignment on niri even though niri IPC can place.
    pub(crate) fn can_assign_displays(&self) -> bool {
        self.monitors.len() > 1
            && (std::env::var_os("NIRI_SOCKET").is_some() || self.options.gamescope_sdl_backend)
    }

    pub(super) fn is_device_in_any_instance(&self, dev: usize) -> bool {
        for instance in &self.instances {
            if instance.devices.contains(&dev) {
                return true;
            }
        }
        false
    }

    pub(super) fn is_device_in_instance(&self, instance_index: usize, dev: usize) -> bool {
        if self.instances[instance_index].devices.contains(&dev) {
            return true;
        }
        false
    }

    pub(super) fn find_device_in_instance(&mut self, dev: usize) -> Option<(usize, usize)> {
        for (i, instance) in self.instances.iter().enumerate() {
            for (d, device) in instance.devices.iter().enumerate() {
                if device == &dev {
                    return Some((i, d));
                }
            }
        }
        None
    }

    fn find_device_in_instance_from_end(&mut self, dev: usize) -> Option<(usize, usize)> {
        for (i, instance) in self.instances.iter().enumerate().rev() {
            for (d, device) in instance.devices.iter().enumerate() {
                if device == &dev {
                    return Some((i, d));
                }
            }
        }
        None
    }

    /// Remove the WHOLE player (instance) that owns `dev`.
    ///
    /// kb/mouse seats hold the mouse PLUS every keyboard — they join as one I/O
    /// unit (see `join_kbm_partners`), so dropping a single device would leave the
    /// seat alive with its partners. "X" on the device strip means "remove this
    /// player", so tear down the entire instance. A gamepad seat holds one pad, so
    /// for pads this is identical to removing that device. (Per-device removal from
    /// a card still goes through `remove_device_instance`.)
    pub fn remove_player_by_device(&mut self, dev: usize) {
        if let Some((instance_index, _)) = self.find_device_in_instance_from_end(dev) {
            self.instances.remove(instance_index);
        }
    }

    pub fn remove_device_instance(&mut self, instance_index: usize, dev: usize) {
        let device_index = self.instances[instance_index]
            .devices
            .iter()
            .position(|device| device == &dev);

        if let Some(d) = device_index {
            self.instances[instance_index].devices.remove(d);

            if self.instances[instance_index].devices.is_empty() {
                self.instances.remove(instance_index);
            }
        }
    }
}
