//! Instance page directional navigation

use crate::app::app::{InstanceFocus, Splitux};
use crate::ui::focus::types::InstanceCardFocus;

impl Splitux {
    pub(super) fn handle_instance_up(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions => {
                if self.instances.len() > 0 {
                    self.instance_focus = InstanceFocus::InstanceCard(
                        self.instances.len() - 1,
                        InstanceCardFocus::AudioOverride
                    );
                } else {
                    self.instance_focus = InstanceFocus::Devices;
                }
            }
            InstanceFocus::StartButton => {
                self.instance_focus = InstanceFocus::LaunchOptions;
                self.launch_option_index = 0; // Reset to carousel
            }
            InstanceFocus::InstanceCard(idx, element) => {
                let idx = *idx;

                // Check if SetMaster button is visible (named profile that's not already master)
                let instance = self.instances.get(idx);
                let profile_name = instance.and_then(|i| self.profiles.get(i.profselection));
                let is_named = profile_name.is_some_and(|p| !p.starts_with("Guest"));
                let is_master = profile_name.is_some_and(|p| self.options.master_profile.as_ref() == Some(p));
                let set_master_visible = is_named && !is_master;

                let new_element = match element {
                    InstanceCardFocus::Profile => {
                        if idx > 0 {
                            self.instance_focus = InstanceFocus::InstanceCard(
                                idx - 1,
                                InstanceCardFocus::AudioOverride
                            );
                            return;
                        } else {
                            self.instance_focus = InstanceFocus::Devices;
                            return;
                        }
                    }
                    InstanceCardFocus::SetMaster => InstanceCardFocus::Profile,
                    InstanceCardFocus::Monitor => {
                        if set_master_visible {
                            InstanceCardFocus::SetMaster
                        } else {
                            InstanceCardFocus::Profile
                        }
                    }
                    InstanceCardFocus::InviteDevice => {
                        if self.options.gamescope_sdl_backend {
                            InstanceCardFocus::Monitor
                        } else if set_master_visible {
                            InstanceCardFocus::SetMaster
                        } else {
                            InstanceCardFocus::Profile
                        }
                    }
                    InstanceCardFocus::Device(0) => InstanceCardFocus::InviteDevice,
                    InstanceCardFocus::Device(d) => InstanceCardFocus::Device(d - 1),
                    InstanceCardFocus::AudioOverride => {
                        let dev_count = self.instances.get(idx).map(|inst| inst.devices.len()).unwrap_or(0);
                        if dev_count > 0 {
                            InstanceCardFocus::Device(dev_count - 1)
                        } else {
                            InstanceCardFocus::InviteDevice
                        }
                    }
                    InstanceCardFocus::AudioPreference => InstanceCardFocus::AudioOverride,
                    InstanceCardFocus::GptokeybProfile => InstanceCardFocus::AudioPreference,
                };
                self.instance_focus = InstanceFocus::InstanceCard(idx, new_element);
            }
            InstanceFocus::Devices => {}
        }
    }

    pub(super) fn handle_instance_down(&mut self) {
        match &self.instance_focus {
            InstanceFocus::Devices => {
                if self.instances.len() > 0 {
                    self.instance_focus = InstanceFocus::InstanceCard(0, InstanceCardFocus::Profile);
                }
            }
            InstanceFocus::LaunchOptions => {
                self.instance_focus = InstanceFocus::StartButton;
            }
            InstanceFocus::StartButton => {}
            InstanceFocus::InstanceCard(idx, element) => {
                let idx = *idx;
                let dev_count = self.instances.get(idx).map(|inst| inst.devices.len()).unwrap_or(0);

                // Check if SetMaster button is visible (named profile that's not already master)
                let instance = self.instances.get(idx);
                let profile_name = instance.and_then(|i| self.profiles.get(i.profselection));
                let is_named = profile_name.is_some_and(|p| !p.starts_with("Guest"));
                let is_master = profile_name.is_some_and(|p| self.options.master_profile.as_ref() == Some(p));
                let set_master_visible = is_named && !is_master;

                let new_element = match element {
                    InstanceCardFocus::Profile => {
                        if set_master_visible {
                            InstanceCardFocus::SetMaster
                        } else if self.options.gamescope_sdl_backend {
                            InstanceCardFocus::Monitor
                        } else {
                            InstanceCardFocus::InviteDevice
                        }
                    }
                    InstanceCardFocus::SetMaster => {
                        if self.options.gamescope_sdl_backend {
                            InstanceCardFocus::Monitor
                        } else {
                            InstanceCardFocus::InviteDevice
                        }
                    }
                    InstanceCardFocus::Monitor => InstanceCardFocus::InviteDevice,
                    InstanceCardFocus::InviteDevice => {
                        if dev_count > 0 {
                            InstanceCardFocus::Device(0)
                        } else {
                            InstanceCardFocus::AudioOverride
                        }
                    }
                    InstanceCardFocus::Device(d) => {
                        if *d + 1 < dev_count {
                            InstanceCardFocus::Device(d + 1)
                        } else {
                            InstanceCardFocus::AudioOverride
                        }
                    }
                    InstanceCardFocus::AudioOverride => InstanceCardFocus::AudioPreference,
                    InstanceCardFocus::AudioPreference => InstanceCardFocus::GptokeybProfile,
                    InstanceCardFocus::GptokeybProfile => {
                        if idx + 1 < self.instances.len() {
                            self.instance_focus = InstanceFocus::InstanceCard(
                                idx + 1,
                                InstanceCardFocus::Profile
                            );
                            return;
                        } else {
                            self.instance_focus = InstanceFocus::LaunchOptions;
                            self.launch_option_index = 0;
                            return;
                        }
                    }
                };
                self.instance_focus = InstanceFocus::InstanceCard(idx, new_element);
            }
        }
    }

    pub(super) fn handle_instance_left(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions => {
                let player_count = self.instances.len();
                let has_carousel = player_count >= 2;

                // If on carousel (index 0), cycle preset
                if has_carousel && self.launch_option_index == 0 {
                    self.options.layout_presets.cycle_prev(player_count);
                } else if self.launch_option_index > 0 {
                    self.launch_option_index -= 1;
                }
            }
            InstanceFocus::InstanceCard(idx, element) => {
                if *idx > 0 {
                    self.instance_focus = InstanceFocus::InstanceCard(idx - 1, element.clone());
                }
            }
            _ => {}
        }
    }

    pub(super) fn handle_instance_right(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions => {
                let player_count = self.instances.len();
                let has_carousel = player_count >= 2;
                let max_options = if has_carousel { 2 } else { 1 };

                // If on carousel (index 0), cycle preset
                if has_carousel && self.launch_option_index == 0 {
                    self.options.layout_presets.cycle_next(player_count);
                } else if self.launch_option_index < max_options - 1 {
                    self.launch_option_index += 1;
                }
            }
            InstanceFocus::InstanceCard(idx, element) => {
                if *idx + 1 < self.instances.len() {
                    self.instance_focus = InstanceFocus::InstanceCard(idx + 1, element.clone());
                }
            }
            _ => {}
        }
    }
}
