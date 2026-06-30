//! Keyboard input handling for instance page

use crate::app::app::{InstanceFocus, Splitux};
use crate::input::PadButton;
use crate::ui::focus::types::InstanceCardFocus;

impl Splitux {
    /// Process keyboard navigation for instance page
    pub(crate) fn process_instance_nav_key(&mut self, btn: PadButton) {
        match btn {
            PadButton::Up => self.handle_instance_up(),
            PadButton::Down => self.handle_instance_down(),
            PadButton::Left => self.handle_instance_left(),
            PadButton::Right => self.handle_instance_right(),
            _ => {}
        }
    }

    /// Process keyboard activation for instance page
    pub(crate) fn process_instance_activate_key(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions => {
                // The carousel is the only launch option; activate cycles it.
                if self.show_layout_carousel() {
                    self.options.layout_presets.cycle_next(self.instances.len());
                }
            }
            InstanceFocus::StartButton => {
                if !self.instances.is_empty() {
                    self.prepare_game_launch();
                }
            }
            InstanceFocus::InstanceCard(_, _) => {
                self.activate_focused = true;
            }
            InstanceFocus::Devices => {}
            // Enter confirms the picked game and returns to the setup content.
            InstanceFocus::GamesSidebar => self.instance_focus = InstanceFocus::Devices,
        }
    }

    /// Process keyboard back for instance page
    pub(crate) fn process_instance_back_key(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions | InstanceFocus::StartButton => {
                if !self.instances.is_empty() {
                    self.instance_focus = InstanceFocus::InstanceCard(
                        self.instances.len() - 1,
                        InstanceCardFocus::Profile
                    );
                } else {
                    self.instance_focus = InstanceFocus::Devices;
                }
            }
            InstanceFocus::InstanceCard(_, _) => {
                self.instance_focus = InstanceFocus::Devices;
            }
            InstanceFocus::Devices => {}
            InstanceFocus::GamesSidebar => self.instance_focus = InstanceFocus::Devices,
        }
    }
}
