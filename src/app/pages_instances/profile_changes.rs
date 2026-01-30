//! Profile change handling for instance page

use crate::app::app::Splitux;
use crate::input::{find_device_by_uniq, is_device_assigned};
use crate::profile_prefs::ProfilePreferences;

impl Splitux {
    /// Handle profile selection changes - auto-assign preferred controllers and audio
    pub(super) fn handle_profile_changes(&mut self, profile_changes: Vec<(usize, usize)>) {
        self.controller_warnings.clear();
        self.audio_warnings.clear();

        for (instance_idx, new_selection) in profile_changes {
            if instance_idx < self.prev_profile_selections.len() {
                self.prev_profile_selections[instance_idx] = new_selection;
            }

            if new_selection == 0 || new_selection >= self.profiles.len() {
                continue;
            }

            let profile_name = &self.profiles[new_selection];
            if profile_name.starts_with('.') {
                continue;
            }

            let prefs = ProfilePreferences::load(profile_name);

            // Try to auto-assign preferred controller
            if let Some(ref preferred_uniq) = prefs.preferred_controller {
                if let Some(dev_idx) = find_device_by_uniq(&self.input_devices, preferred_uniq) {
                    if !is_device_assigned(dev_idx, &self.instances) {
                        if instance_idx < self.instances.len() {
                            if !self.instances[instance_idx].devices.contains(&dev_idx) {
                                self.instances[instance_idx].devices.push(dev_idx);
                                println!(
                                    "[splitux] Auto-assigned {} to profile '{}'",
                                    self.device_display_name(dev_idx),
                                    profile_name
                                );
                            }
                        }
                    } else {
                        self.controller_warnings.push(format!(
                            "{}'s controller ({}) is assigned to another player",
                            profile_name,
                            prefs.preferred_controller_name.as_deref().unwrap_or("unknown")
                        ));
                    }
                } else {
                    self.controller_warnings.push(format!(
                        "{}'s preferred controller ({}) is not connected",
                        profile_name,
                        prefs.preferred_controller_name.as_deref().unwrap_or("unknown")
                    ));
                }
            }

            // Store audio preference for use at launch
            if let Some(ref preferred_audio) = prefs.preferred_audio {
                self.profile_audio_prefs.insert(instance_idx, preferred_audio.clone());

                let sink_available = self
                    .audio_devices
                    .iter()
                    .any(|d| d.name == *preferred_audio);

                if !sink_available {
                    self.audio_warnings.push(format!(
                        "{}'s preferred audio ({}) is not available",
                        profile_name,
                        prefs.preferred_audio_name.as_deref().unwrap_or("unknown")
                    ));
                }
            }
        }
    }
}
