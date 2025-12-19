//! General settings section (options 0-12)
//!
//! Includes: Window Manager, Controller filter, Proton settings, Photon networking, Gamescope

use crate::app::app::Splitux;
use crate::app::config::{PadFilterType, WindowManagerType};
use crate::input::scan_input_devices;
use crate::paths::PATH_PARTY;
use crate::ui::responsive::LayoutMode;
use crate::util::{msg, yesno};
use eframe::egui::{self, RichText, Ui};

impl Splitux {
    pub fn display_settings_general(&mut self, ui: &mut Ui) {
        let layout_mode = LayoutMode::from_ui(ui);
        let is_narrow = layout_mode.is_narrow();

        // Option 0: Window Manager
        let frame_resp = self.settings_option_frame(0).show(ui, |ui| {
            let show_radios = |ui: &mut Ui| -> (egui::Response, egui::Response, egui::Response, egui::Response) {
                let r1 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::Auto, "Auto");
                let r2 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::KWin, "KWin");
                let r3 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::Hyprland, "Hyprland");
                let r4 = ui.radio_value(&mut self.options.window_manager, WindowManagerType::GamescopeOnly, "None");
                (r1, r2, r3, r4)
            };

            let wm_label = ui.label("Window Manager");
            let (r1, r2, r3, r4) = if is_narrow {
                ui.horizontal_wrapped(show_radios)
            } else {
                ui.horizontal(show_radios)
            }.inner;

            if wm_label.hovered() || r1.hovered() || r2.hovered() || r3.hovered() || r4.hovered() || self.is_settings_option_focused(0) {
                self.infotext = "DEFAULT: Auto\n\nSelect window manager for positioning game windows. Auto detects your WM. Use 'None' for manual positioning or Gamescope-only mode.".to_string();
            }
        });
        self.scroll_to_settings_option_if_needed(0, &frame_resp.response);

        // Option 1: Controller filter
        let r = self.settings_option_frame(1).show(ui, |ui| {
            let filter_label = ui.label("Controller filter");
            let (r1, r2, r3) = if is_narrow {
                ui.horizontal_wrapped(|ui| {
                    let r1 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::All, "All");
                    let r2 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::NoSteamInput, "No Steam");
                    let r3 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::OnlySteamInput, "Steam Only");
                    (r1, r2, r3)
                }).inner
            } else {
                ui.horizontal(|ui| {
                    let r1 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::All, "All controllers");
                    let r2 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::NoSteamInput, "No Steam Input");
                    let r3 = ui.radio_value(&mut self.options.pad_filter_type, PadFilterType::OnlySteamInput, "Only Steam Input");
                    (r1, r2, r3)
                }).inner
            };

            if filter_label.hovered() || r1.hovered() || r2.hovered() || r3.hovered() || self.is_settings_option_focused(1) {
                self.infotext = "DEFAULT: No Steam Input\n\nSelect which controllers to filter out.".to_string();
            }

            if r1.clicked() || r2.clicked() || r3.clicked() {
                self.input_devices = scan_input_devices(&self.options.pad_filter_type);
                self.refresh_device_display_names();
            }
        });
        self.scroll_to_settings_option_if_needed(1, &r.response);

        // Option 2: Proton version
        let r = self.settings_option_frame(2).show(ui, |ui| {
            ui.horizontal(|ui| {
                let proton_ver_label = ui.label("Proton version");
                let proton_ver_editbox = ui.add(
                    egui::TextEdit::singleline(&mut self.options.proton_version).hint_text("GE-Proton"),
                );
                if proton_ver_label.hovered() || proton_ver_editbox.hovered() || self.is_settings_option_focused(2) {
                    self.infotext = "DEFAULT: GE-Proton\n\nSpecify a Proton version.".to_string();
                }
            });
        });
        self.scroll_to_settings_option_if_needed(2, &r.response);

        // Option 3: Separate Proton prefixes
        let r = self.settings_option_frame(3).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.proton_separate_pfxs, "Run instances in separate Proton prefixes");
            if check.hovered() || self.is_settings_option_focused(3) {
                self.infotext = "DEFAULT: Enabled\n\nRuns each instance in separate Proton prefixes.".to_string();
            }
            if self.is_settings_option_focused(3) && self.activate_focused {
                self.options.proton_separate_pfxs = !self.options.proton_separate_pfxs;
            }
        });
        self.scroll_to_settings_option_if_needed(3, &r.response);

        // Option 4: Allow multiple instances
        let r = self.settings_option_frame(4).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.allow_multiple_instances_on_same_device, "(Debug) Allow multiple instances from one gamepad");
            if check.hovered() || self.is_settings_option_focused(4) {
                self.infotext = "DEFAULT: Disabled\n\nAllow multiple instances on the same device.".to_string();
            }
            if self.is_settings_option_focused(4) && self.activate_focused {
                self.options.allow_multiple_instances_on_same_device = !self.options.allow_multiple_instances_on_same_device;
            }
        });
        self.scroll_to_settings_option_if_needed(4, &r.response);

        // Option 5: Force run from original directory
        let r = self.settings_option_frame(5).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.disable_mount_gamedirs, "(Debug) Force run instances from original game directory");
            if check.hovered() || self.is_settings_option_focused(5) {
                self.infotext = "DEFAULT: Disabled\n\nForces instances to launch from the original game directory.".to_string();
            }
            if self.is_settings_option_focused(5) && self.activate_focused {
                self.options.disable_mount_gamedirs = !self.options.disable_mount_gamedirs;
            }
        });
        self.scroll_to_settings_option_if_needed(5, &r.response);

        ui.separator();

        // Option 6: Erase Proton Prefix Data
        let r = self.settings_option_frame(6).show(ui, |ui| {
            let btn = ui.button("Erase All Proton Prefix Data");
            if btn.clicked() || (self.is_settings_option_focused(6) && self.activate_focused) {
                if yesno("Erase Prefix?", "This will erase all Proton prefixes. Are you sure?")
                    && PATH_PARTY.join("prefixes").exists()
                {
                    if let Err(err) = std::fs::remove_dir_all(PATH_PARTY.join("prefixes")) {
                        msg("Error", &format!("Couldn't erase pfx data: {}", err));
                    } else {
                        msg("Data Erased", "Proton prefix data successfully erased.");
                    }
                }
            }
        });
        self.scroll_to_settings_option_if_needed(6, &r.response);

        // Option 7: Open Splitux Data Folder
        let r = self.settings_option_frame(7).show(ui, |ui| {
            let btn = ui.button("Open Splitux Data Folder");
            if btn.clicked() || (self.is_settings_option_focused(7) && self.activate_focused) {
                let _ = std::process::Command::new("xdg-open").arg(PATH_PARTY.clone()).status();
            }
        });
        self.scroll_to_settings_option_if_needed(7, &r.response);

        // Photon Networking section
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("Photon Networking").strong());
        ui.add_space(4.0);

        // Option 8: PUN App ID
        let r = self.settings_option_frame(8).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("PUN App ID:");
                ui.add(egui::TextEdit::singleline(&mut self.options.photon_app_ids.pun_app_id)
                    .password(true).desired_width(280.0).hint_text("Enter your Photon PUN App ID"));
            });
            if self.is_settings_option_focused(8) {
                self.infotext = "Get a free Photon PUN App ID from dashboard.photonengine.com.".to_string();
            }
        });
        self.scroll_to_settings_option_if_needed(8, &r.response);

        // Option 9: Voice App ID
        let r = self.settings_option_frame(9).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Voice App ID:");
                ui.add(egui::TextEdit::singleline(&mut self.options.photon_app_ids.voice_app_id)
                    .password(true).desired_width(280.0).hint_text("Optional - for voice chat"));
            });
            if self.is_settings_option_focused(9) {
                self.infotext = "Optional: Photon Voice App ID for games that use voice chat.".to_string();
            }
        });
        self.scroll_to_settings_option_if_needed(9, &r.response);

        ui.horizontal(|ui| {
            ui.add_space(80.0);
            ui.hyperlink_to("Get Photon App IDs (free)", "https://dashboard.photonengine.com");
        });
    }

    pub fn display_settings_gamescope(&mut self, ui: &mut Ui) {
        // Option 10: Fix low resolution
        let r = self.settings_option_frame(10).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.gamescope_fix_lowres, "Automatically fix low resolution instances");
            if check.hovered() || self.is_settings_option_focused(10) {
                self.infotext = "Many games have graphical problems below 600p. This auto-resizes such instances.".to_string();
            }
            if self.is_settings_option_focused(10) && self.activate_focused {
                self.options.gamescope_fix_lowres = !self.options.gamescope_fix_lowres;
            }
        });
        self.scroll_to_settings_option_if_needed(10, &r.response);

        // Option 11: SDL backend
        let r = self.settings_option_frame(11).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.gamescope_sdl_backend, "Use SDL backend");
            if check.hovered() || self.is_settings_option_focused(11) {
                self.infotext = "Required for multi-monitor support. Disable if you see black screens on Nvidia + Wayland.".to_string();
            }
            if self.is_settings_option_focused(11) && self.activate_focused {
                self.options.gamescope_sdl_backend = !self.options.gamescope_sdl_backend;
            }
        });
        self.scroll_to_settings_option_if_needed(11, &r.response);

        // Option 12: Force grab cursor
        let r = self.settings_option_frame(12).show(ui, |ui| {
            let check = ui.checkbox(&mut self.options.gamescope_force_grab_cursor, "Force grab cursor for Gamescope");
            if check.hovered() || self.is_settings_option_focused(12) {
                self.infotext = "Keeps the cursor within the Gamescope window.".to_string();
            }
            if self.is_settings_option_focused(12) && self.activate_focused {
                self.options.gamescope_force_grab_cursor = !self.options.gamescope_force_grab_cursor;
            }
        });
        self.scroll_to_settings_option_if_needed(12, &r.response);
    }
}
