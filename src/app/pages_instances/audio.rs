//! Audio helper functions for instance page

use crate::app::app::Splitux;
use std::collections::{HashMap, HashSet};

impl Splitux {
    /// Detect audio conflicts - returns set of instance indices that have conflicts
    pub(super) fn detect_audio_conflicts(&self) -> HashSet<usize> {
        let mut sink_usage: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, _instance) in self.instances.iter().enumerate() {
            // Check session override first
            if let Some(override_opt) = self.audio_session_overrides.get(&i) {
                if let Some(sink) = override_opt {
                    sink_usage.entry(sink.clone()).or_default().push(i);
                }
                continue; // Session override takes precedence
            }

            // Check profile_audio_prefs (populated from ProfilePreferences)
            if let Some(sink) = self.profile_audio_prefs.get(&i) {
                sink_usage.entry(sink.clone()).or_default().push(i);
            }
        }

        // Find instances with conflicts (same sink used by multiple instances)
        let mut conflicts = HashSet::new();
        for (_sink, instances) in sink_usage {
            if instances.len() > 1 {
                for idx in instances {
                    conflicts.insert(idx);
                }
            }
        }
        conflicts
    }

    /// Get the effective audio sink for an instance (session override or profile preference)
    /// Returns: (sink_name, display_name, is_override)
    pub(super) fn get_effective_audio(&self, instance_idx: usize) -> Option<(String, String, bool)> {
        // Check session override first
        if let Some(override_opt) = self.audio_session_overrides.get(&instance_idx) {
            return match override_opt {
                Some(sink) => {
                    let name = self
                        .audio_devices
                        .iter()
                        .find(|d| &d.name == sink)
                        .map(|d| d.description.clone())
                        .unwrap_or_else(|| sink.clone());
                    Some((sink.clone(), name, true))
                }
                None => Some(("".to_string(), "Muted".to_string(), true)), // Explicit mute
            };
        }

        // Check profile preference
        if let Some(sink) = self.profile_audio_prefs.get(&instance_idx) {
            let name = self
                .audio_devices
                .iter()
                .find(|d| &d.name == sink)
                .map(|d| d.description.clone())
                .unwrap_or_else(|| sink.clone());
            return Some((sink.clone(), name, false));
        }

        None
    }
}
