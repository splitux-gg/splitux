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
                let player_count = self.instances.len();
                let has_carousel = player_count >= 2;
                let max_options = if has_carousel { 2 } else { 1 };
                match self.launch_option_index {
                    0 if has_carousel => {
                        // Enter/A on carousel cycles to next preset
                        self.options.layout_presets.cycle_next(player_count);
                    }
                    idx if idx == max_options - 1 => {
                        self.options.input_holding = !self.options.input_holding;
                    }
                    _ => {}
                }
            }
            InstanceFocus::StartButton => {
                if self.instances.len() > 0 {
                    self.prepare_game_launch();
                }
            }
            InstanceFocus::InstanceCard(_, _) => {
                self.activate_focused = true;
            }
            InstanceFocus::Devices => {}
        }
    }

    /// Process keyboard back for instance page
    pub(crate) fn process_instance_back_key(&mut self) {
        match &self.instance_focus {
            InstanceFocus::LaunchOptions | InstanceFocus::StartButton => {
                if self.instances.len() > 0 {
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
        }
    }
}
