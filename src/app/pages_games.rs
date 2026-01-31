//! Games page display functions
//!
//! Submodules:
//! - `welcome` - Welcome screen when no games configured
//! - `game_info` - Game detail view with action bar and metadata

mod game_info;
mod welcome;

use super::app::Splitux;
use crate::handler::HANDLER_SPEC_CURRENT_VERSION;
use crate::util::msg;
use eframe::egui::Ui;

impl Splitux {
    pub fn display_page_games(&mut self, ui: &mut Ui) {
        // If no handlers or in lite mode, show welcome screen
        if self.handlers.is_empty() && !self.is_lite() {
            self.display_welcome_screen(ui);
            return;
        }

        // Show selected game info
        self.display_game_info(ui);
    }

    fn check_and_start_game(&mut self) {
        let h = self.cur_handler();
        if h.spec_ver != HANDLER_SPEC_CURRENT_VERSION {
            let mismatch = match h.spec_ver < HANDLER_SPEC_CURRENT_VERSION {
                true => "an older",
                false => "a newer",
            };
            let mismatch2 = match h.spec_ver < HANDLER_SPEC_CURRENT_VERSION {
                true => "Up-to-date handlers can be found by clicking the download button on the top bar of the launcher.",
                false => "It is recommended to update Splitux to the latest version.",
            };
            msg(
                "Handler version mismatch",
                &format!("This handler was meant for use with {} version of Splitux; you may experience issues or the game may not work at all. {} If everything still works fine, you can prevent this message appearing in the future by editing the handler, updating the spec version and saving.",
                    mismatch, mismatch2
                )
            );
        }
        self.start_game_setup();
    }
}
