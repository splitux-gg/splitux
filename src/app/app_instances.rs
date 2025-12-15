// Instance device management functions

use super::app::Splitux;

impl Splitux {
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

    pub fn remove_device(&mut self, dev: usize) {
        if let Some((instance_index, device_index)) = self.find_device_in_instance_from_end(dev) {
            self.instances[instance_index].devices.remove(device_index);
            if self.instances[instance_index].devices.is_empty() {
                self.instances.remove(instance_index);
            }
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
