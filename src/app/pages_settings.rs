// Settings page display functions

use super::app::PartyApp;
use super::config::{save_cfg, PartyConfig, PadFilterType, WindowManagerType};
use crate::input::scan_input_devices;
use crate::paths::PATH_PARTY;
use crate::util::{msg, yesno};
use eframe::egui::{self, RichText, Ui};

impl PartyApp {
    pub fn display_page_settings(&mut self, ui: &mut Ui) {
        self.infotext.clear();
        ui.add_space(8.0);
        ui.heading("Settings");
        ui.add_space(4.0);
        ui.label("Configure Splitux behavior and game launch options");
        ui.add_space(8.0);
        ui.separator();

        // Single scrollable settings page with sections
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(ui.available_height() - 60.0)
            .show(ui, |ui| {
                // === General Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("General").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_general(ui);

                ui.add_space(16.0);
                ui.separator();

                // === Gamescope Section ===
                ui.add_space(8.0);
                ui.label(RichText::new("Gamescope").strong().size(16.0));
                ui.add_space(4.0);
                self.display_settings_gamescope(ui);

                ui.add_space(8.0);
            });

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.horizontal(|ui| {
                if ui.button("Save Settings").clicked() {
                    if let Err(e) = save_cfg(&self.options) {
                        msg("Error", &format!("Couldn't save settings: {}", e));
                    }
                }
                if ui.button("Restore Defaults").clicked() {
                    self.options = PartyConfig::default();
                    self.input_devices = scan_input_devices(&self.options.pad_filter_type);
                }
            });
            ui.separator();
        });
    }

    pub fn display_settings_general(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let wm_label = ui.label("Window Manager");
            let r1 = ui.radio_value(
                &mut self.options.window_manager,
                WindowManagerType::Auto,
                "Auto",
            );
            let r2 = ui.radio_value(
                &mut self.options.window_manager,
                WindowManagerType::KWin,
                "KWin",
            );
            let r3 = ui.radio_value(
                &mut self.options.window_manager,
                WindowManagerType::Hyprland,
                "Hyprland",
            );
            let r4 = ui.radio_value(
                &mut self.options.window_manager,
                WindowManagerType::GamescopeOnly,
                "None",
            );

            if wm_label.hovered() || r1.hovered() || r2.hovered() || r3.hovered() || r4.hovered() {
                self.infotext = "DEFAULT: Auto\n\nSelect window manager for positioning game windows. Auto detects your WM. Use 'None' for manual positioning or Gamescope-only mode.".to_string();
            }
        });

        ui.horizontal(|ui| {
            let split_style_label = ui.label("Split style");
            let r1 = ui.radio_value(
                &mut self.options.vertical_two_player,
                false,
                "Horizontal",
            );
            let r2 = ui.radio_value(
                &mut self.options.vertical_two_player,
                true,
                "Vertical",
            );
            if split_style_label.hovered() || r1.hovered() || r2.hovered() {
                self.infotext =
                    "DEFAULT: Horizontal\n\nChoose whether to split two-player games horizontally (above/below) instead of vertically (side by side).".to_string();
            }
        });

        ui.horizontal(|ui| {
            let filter_label = ui.label("Controller filter");
            let r1 = ui.radio_value(
                &mut self.options.pad_filter_type,
                PadFilterType::All,
                "All controllers",
            );
            let r2 = ui.radio_value(
                &mut self.options.pad_filter_type,
                PadFilterType::NoSteamInput,
                "No Steam Input",
            );
            let r3 = ui.radio_value(
                &mut self.options.pad_filter_type,
                PadFilterType::OnlySteamInput,
                "Only Steam Input",
            );

            if filter_label.hovered() || r1.hovered() || r2.hovered() || r3.hovered() {
                self.infotext = "DEFAULT: No Steam Input\n\nSelect which controllers to filter out. If you use Steam Input to remap controllers, you may want to select \"Only Steam Input\", but be warned that this option is experimental and is known to break certain Proton games.".to_string();
            }

            if r1.clicked() || r2.clicked() || r3.clicked() {
                self.input_devices = scan_input_devices(&self.options.pad_filter_type);
            }
        });

        ui.horizontal(|ui| {
        let proton_ver_label = ui.label("Proton version");
        let proton_ver_editbox = ui.add(
            egui::TextEdit::singleline(&mut self.options.proton_version)
                .hint_text("GE-Proton"),
        );
        if proton_ver_label.hovered() || proton_ver_editbox.hovered() {
            self.infotext = "DEFAULT: GE-Proton\n\nSpecify a Proton version. This can be a path, e.g. \"/path/to/proton\" or just a name, e.g. \"GE-Proton\" for the latest version of Proton-GE. If left blank, this will default to \"GE-Proton\". If unsure, leave this blank.".to_string();
        }
        });

        let proton_separate_pfxs_check = ui.checkbox(
            &mut self.options.proton_separate_pfxs,
            "Run instances in separate Proton prefixes",
        );
        if proton_separate_pfxs_check.hovered() {
            self.infotext = "DEFAULT: Enabled\n\nRuns each instance in separate Proton prefixes. If unsure, leave this checked. Multiple prefixes takes up more disk space, but generally provides better compatibility and fewer issues with Proton-based games.".to_string();
        }

        let allow_multiple_instances_on_same_device_check = ui.checkbox(
            &mut self.options.allow_multiple_instances_on_same_device,
            "(Debug) Allow multiple instances from one gamepad",
        );
        if allow_multiple_instances_on_same_device_check.hovered() {
            self.infotext = "DEFAULT: Disabled\n\nAllow multiple instances on the same device. This can be useful for testing or when one person wants to control multiple instances.".to_string();
        }

        let disable_mount_gamedirs_check = ui.checkbox(
            &mut self.options.disable_mount_gamedirs,
            "(Debug) Force run instances from original game directory",
        );
        if disable_mount_gamedirs_check.hovered() {
            self.infotext = "DEFAULT: Disabled\n\nBy default, Splitux mounts game directories using fuse-overlayfs to let each instance write to the game's directory without conflicting with each other or affecting the game's installation. In addition, this lets handlers overlay content like mods or config files onto the game directory. Enabling this forces instances to launch from the original game directory without mounting, which will prevent handlers from using built-in mods, but may be useful for diagnosing issues.".to_string();
        }

        ui.separator();

        if ui.button("Erase All Proton Prefix Data").clicked() {
            if yesno(
                "Erase Prefix?",
                "This will erase all Proton prefixes used by Splitux. This shouldn't erase profile/game-specific data, but exercise caution. Are you sure?",
            ) && PATH_PARTY.join("prefixes").exists()
            {
                if let Err(err) = std::fs::remove_dir_all(PATH_PARTY.join("prefixes")) {
                    msg("Error", &format!("Couldn't erase pfx data: {}", err));
                } else {
                    msg("Data Erased", "Proton prefix data successfully erased.");
                }
            }
        }

        if ui.button("Open Splitux Data Folder").clicked() {
            if let Err(_) = std::process::Command::new("xdg-open")
                .arg(PATH_PARTY.clone())
                .status()
            {
                msg("Error", "Couldn't open Splitux Data Folder!");
            }
        }

        // Photon Networking section
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("Photon Networking").strong());
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            let pun_label = ui.label("PUN App ID:");
            let pun_edit = ui.add(
                egui::TextEdit::singleline(&mut self.options.photon_app_ids.pun_app_id)
                    .password(true)
                    .desired_width(280.0)
                    .hint_text("Enter your Photon PUN App ID"),
            );
            if pun_label.hovered() || pun_edit.hovered() {
                self.infotext = "Get a free Photon PUN App ID from dashboard.photonengine.com. Required for games using Photon networking (like PEAK). Free tier: 20 CCU.".to_string();
            }
        });

        ui.horizontal(|ui| {
            let voice_label = ui.label("Voice App ID:");
            let voice_edit = ui.add(
                egui::TextEdit::singleline(&mut self.options.photon_app_ids.voice_app_id)
                    .password(true)
                    .desired_width(280.0)
                    .hint_text("Optional - for voice chat"),
            );
            if voice_label.hovered() || voice_edit.hovered() {
                self.infotext = "Optional: Photon Voice App ID for games that use voice chat. Also available free from dashboard.photonengine.com.".to_string();
            }
        });

        ui.horizontal(|ui| {
            ui.add_space(80.0);
            ui.hyperlink_to("Get Photon App IDs (free)", "https://dashboard.photonengine.com");
        });
    }

    pub fn display_settings_gamescope(&mut self, ui: &mut Ui) {
        let gamescope_lowres_fix_check = ui.checkbox(
            &mut self.options.gamescope_fix_lowres,
            "Automatically fix low resolution instances",
        );
        let gamescope_sdl_backend_check =
            ui.checkbox(&mut self.options.gamescope_sdl_backend, "Use SDL backend");
        let kbm_support_check = ui.checkbox(
            &mut self.options.kbm_support,
            "Enable keyboard and mouse support through custom Gamescope",
        );
        let gamescope_force_grab_cursor_check = ui.checkbox(
            &mut self.options.gamescope_force_grab_cursor,
            "Force grab cursor for Gamescope",
        );

        if gamescope_lowres_fix_check.hovered() {
            self.infotext = "Many games have graphical problems or even crash when running at resolutions below 600p. If this is enabled, any instances below 600p will automatically be resized before launching.".to_string();
        }
        if gamescope_sdl_backend_check.hovered() {
            self.infotext = "Runs gamescope sessions using the SDL backend. This is required for multi-monitor support. If unsure, leave this checked. If gamescope sessions only show a black screen or give an error (especially on Nvidia + Wayland), try disabling this.".to_string();
        }
        if kbm_support_check.hovered() {
            self.infotext = "Runs a custom Gamescope build with support for holding keyboards and mice. If you want to use your own Gamescope installation, uncheck this.".to_string();
        }
        if gamescope_force_grab_cursor_check.hovered() {
            self.infotext = "Sets the \"--force-grab-cursor\" flag in Gamescope. This keeps the cursor within the Gamescope window. If unsure, leave this unchecked.".to_string();
        }
    }
}
