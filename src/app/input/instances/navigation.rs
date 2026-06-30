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
                        InstanceCardFocus::AudioPreference
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
                                InstanceCardFocus::AudioPreference
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
                    // First device sits below Monitor / SetMaster / Profile.
                    InstanceCardFocus::Device(0) => {
                        if self.can_assign_displays() {
                            InstanceCardFocus::Monitor
                        } else if set_master_visible {
                            InstanceCardFocus::SetMaster
                        } else {
                            InstanceCardFocus::Profile
                        }
                    }
                    InstanceCardFocus::Device(d) => InstanceCardFocus::Device(d - 1),
                    InstanceCardFocus::AudioOverride => {
                        let dev_count = self.instances.get(idx).map(|inst| inst.devices.len()).unwrap_or(0);
                        if dev_count > 0 {
                            InstanceCardFocus::Device(dev_count - 1)
                        } else if self.can_assign_displays() {
                            InstanceCardFocus::Monitor
                        } else if set_master_visible {
                            InstanceCardFocus::SetMaster
                        } else {
                            InstanceCardFocus::Profile
                        }
                    }
                    InstanceCardFocus::AudioPreference => InstanceCardFocus::AudioOverride,
                };
                self.instance_focus = InstanceFocus::InstanceCard(idx, new_element);
            }
            InstanceFocus::Devices => {}
            // In the games sidebar, Up moves the game selection (launch tracks it).
            InstanceFocus::GamesSidebar => self.sidebar_select_prev(),
        }
    }

    pub(super) fn handle_instance_down(&mut self) {
        match &self.instance_focus {
            InstanceFocus::Devices => {
                if self.instances.len() > 0 {
                    self.instance_focus = InstanceFocus::InstanceCard(0, InstanceCardFocus::Profile);
                }
            }
            InstanceFocus::GamesSidebar => self.sidebar_select_next(),
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
                        } else if self.can_assign_displays() {
                            InstanceCardFocus::Monitor
                        } else if dev_count > 0 {
                            InstanceCardFocus::Device(0)
                        } else {
                            InstanceCardFocus::AudioOverride
                        }
                    }
                    InstanceCardFocus::SetMaster => {
                        if self.can_assign_displays() {
                            InstanceCardFocus::Monitor
                        } else if dev_count > 0 {
                            InstanceCardFocus::Device(0)
                        } else {
                            InstanceCardFocus::AudioOverride
                        }
                    }
                    InstanceCardFocus::Monitor => {
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
                    InstanceCardFocus::AudioPreference => {
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
                // The carousel is the only launch option, so Left just cycles it.
                if self.show_layout_carousel() {
                    self.options.layout_presets.cycle_prev(self.instances.len());
                }
            }
            InstanceFocus::InstanceCard(idx, element) => {
                if *idx > 0 {
                    self.instance_focus = InstanceFocus::InstanceCard(idx - 1, element.clone());
                } else {
                    // Left off the first card → step into the games sidebar.
                    self.enter_games_sidebar();
                }
            }
            // Devices strip / Start button are the left edge of the setup; the
            // always-visible games sidebar sits further left.
            InstanceFocus::Devices | InstanceFocus::StartButton => self.enter_games_sidebar(),
            InstanceFocus::GamesSidebar => {} // already far-left
        }
    }

    pub(super) fn handle_instance_right(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions => {
                // The carousel is the only launch option, so Right just cycles it.
                if self.show_layout_carousel() {
                    self.options.layout_presets.cycle_next(self.instances.len());
                }
            }
            InstanceFocus::InstanceCard(idx, element) => {
                if *idx + 1 < self.instances.len() {
                    self.instance_focus = InstanceFocus::InstanceCard(idx + 1, element.clone());
                }
            }
            // Right out of the sidebar returns to the setup content.
            InstanceFocus::GamesSidebar => self.instance_focus = InstanceFocus::Devices,
            _ => {}
        }
    }

    /// Move focus into the always-visible left games sidebar (Instances page) so a
    /// controller can change the game in place. `prepare_game_launch` re-pins the
    /// session to `selected_handler` at launch, so just moving the selection here
    /// is enough — no need to rebuild the setup.
    fn enter_games_sidebar(&mut self) {
        if self.handlers.is_empty() {
            return;
        }
        self.instance_focus = InstanceFocus::GamesSidebar;
        self.games_scrolled_idx = None; // re-center the list on the selection
    }

    fn sidebar_select_prev(&mut self) {
        if self.selected_handler > 0 {
            self.selected_handler -= 1;
            self.games_scrolled_idx = None;
        }
    }

    fn sidebar_select_next(&mut self) {
        if self.selected_handler + 1 < self.handlers.len() {
            self.selected_handler += 1;
            self.games_scrolled_idx = None;
        }
    }
}
