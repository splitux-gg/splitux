// Handler editing page display functions

use super::app::Splitux;
use crate::handler::{scan_handlers, SDL2Override, HANDLER_SPEC_CURRENT_VERSION};
use crate::paths::PATH_HOME;
use crate::util::{dir_dialog, file_dialog_relative, msg};
use eframe::egui::{self, RichText, Ui};
use rfd::FileDialog;
use std::path::PathBuf;

impl Splitux {
    pub fn display_edit_handler_modal(&mut self, ctx: &egui::Context) {
        egui::Window::new("Edit Handler")
            .collapsible(false)
            .resizable(true)
            .default_width(600.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                self.display_page_edit_handler(ui);
            });
    }

    pub fn display_page_edit_handler(&mut self, ui: &mut Ui) {
        let h = match &mut self.handler_edit {
            Some(handler) => handler,
            None => {
                return;
            }
        };

        ui.add_space(8.0);
        let header = match h.is_saved_handler() {
            false => "Add Game".to_string(),
            true => format!("Edit Handler: {}", h.display()),
        };
        ui.heading(&header);
        ui.add_space(4.0);
        ui.label("Configure how this game should be launched for split-screen play");
        ui.add_space(8.0);
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.add(egui::TextEdit::singleline(&mut h.name).desired_width(150.0));
            ui.label("Author:");
            ui.add(egui::TextEdit::singleline(&mut h.author).desired_width(50.0));
            ui.label("Version:");
            ui.add(egui::TextEdit::singleline(&mut h.version).desired_width(50.0));
            ui.label("Icon:");
            ui.add(egui::Image::new(h.icon()).max_width(16.0).corner_radius(2));
            if h.is_saved_handler() && ui.button("...").clicked() {
                if let Some(file) = FileDialog::new()
                    .set_title("Choose Icon:")
                    .set_directory(&*PATH_HOME)
                    .add_filter("PNG Image", &["png"])
                    .pick_file()
                    && let Some(extension) = file.extension()
                    && extension == "png"
                {
                    let dest = h.path_handler.join("icon.png");
                    if let Err(e) = std::fs::copy(file, dest) {
                        eprintln!("Failed to copy icon: {}", e);
                        msg("Error copying icon", &format!("{}", e));
                    }
                }
            }
        });

        ui.separator();

        let mut selected_index = self
            .installed_steamapps
            .iter()
            .position(|game_opt| match (game_opt, &h.steam_appid) {
                (Some(game), Some(appid)) => game.app_id == *appid,
                (None, None) => true,
                _ => false,
            })
            .unwrap_or(0);

        ui.horizontal(|ui| {
            ui.label("Steam App:");
            egui::ComboBox::from_id_salt("appid")
                .wrap()
                .width(200.0)
                .show_index(
                    ui,
                    &mut selected_index,
                    self.installed_steamapps.len(),
                    |i| match &self.installed_steamapps[i] {
                        Some(app) => format!("({}) {}", app.app_id, app.install_dir),
                        None => "None".to_string(),
                    },
                );

            ui.add_space(16.0);
            ui.label("Backends:");

            // Goldberg checkbox
            let mut goldberg_enabled = h.has_goldberg();
            if ui.checkbox(&mut goldberg_enabled, "Goldberg (Steam)").changed() {
                if goldberg_enabled {
                    h.enable_goldberg();
                } else {
                    h.disable_goldberg();
                }
            }

            // Photon checkbox
            let mut photon_enabled = h.has_photon();
            if ui.checkbox(&mut photon_enabled, "Photon (BepInEx)").changed() {
                if photon_enabled {
                    h.enable_photon();
                } else {
                    h.disable_photon();
                }
            }

            // Facepunch checkbox
            let mut facepunch_enabled = h.has_facepunch();
            if ui.checkbox(&mut facepunch_enabled, "Facepunch").changed() {
                if facepunch_enabled {
                    h.enable_facepunch();
                } else {
                    h.disable_facepunch();
                }
            }
        });

        h.steam_appid = match &self.installed_steamapps[selected_index] {
            Some(app) => Some(app.app_id),
            None => None,
        };

        if h.steam_appid == None {
            ui.horizontal(|ui| {
                ui.label("Game root folder:");
                ui.add_enabled(false, egui::TextEdit::singleline(&mut h.path_gameroot));
                if ui.button("...").clicked() {
                    if let Ok(path) = dir_dialog() {
                        h.path_gameroot = path.to_string_lossy().to_string();
                    }
                }
            });
        }

        ui.horizontal(|ui| {
            ui.label("Executable:");
            ui.add_enabled(false, egui::TextEdit::singleline(&mut h.exec));
            if ui.button("...").clicked() {
                if let Ok(base_path) = h.get_game_rootpath()
                    && let Ok(path) = file_dialog_relative(&PathBuf::from(base_path))
                {
                    h.exec = path.to_string_lossy().to_string();
                }
            }
        });

        // Photon-specific settings (shown when Photon backend is enabled)
        if let Some(photon_settings) = &mut h.photon {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Photon config path:");
                ui.add(
                    egui::TextEdit::singleline(&mut photon_settings.config_path)
                        .hint_text("AppData/LocalLow/Company/Game/LocalMultiplayer/global.cfg")
                        .desired_width(400.0),
                );
            });
            ui.label(
                RichText::new("  Path where LocalMultiplayer writes its config (relative to profile's windata)")
                    .small()
                    .weak(),
            );
            ui.add_space(4.0);
        }

        // Goldberg-specific settings (shown when Goldberg backend is enabled)
        if let Some(goldberg_settings) = &mut h.goldberg {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut goldberg_settings.disable_networking, "Disable networking");
                ui.label(RichText::new("(Forces LAN discovery mode)").small().weak());
            });
        }

        // Facepunch-specific settings (shown when Facepunch backend is enabled)
        if let Some(facepunch_settings) = &mut h.facepunch {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut facepunch_settings.spoof_identity, "Spoof identity");
                ui.checkbox(&mut facepunch_settings.force_valid, "Force valid");
                ui.checkbox(&mut facepunch_settings.photon_bypass, "Photon bypass");
            });
        }

        ui.horizontal(|ui| {
            ui.label("Environment variables:");
            ui.add(egui::TextEdit::singleline(&mut h.env));
        });

        ui.horizontal(|ui| {
            ui.label("Arguments:");
            ui.add(egui::TextEdit::singleline(&mut h.args));
        });

        if !h.win() {
            ui.horizontal(|ui| {
                ui.label("SDL2 Override:");
                ui.radio_value(&mut h.sdl2_override, SDL2Override::No, "None");
                ui.radio_value(
                    &mut h.sdl2_override,
                    SDL2Override::Srt,
                    "Steam Runtime (32-bit)",
                );
                ui.radio_value(
                    &mut h.sdl2_override,
                    SDL2Override::Sys,
                    "System Installation",
                );
            });
        }

        if !h.win() {
            ui.horizontal(|ui| {
                ui.label("Linux Runtime:");
                ui.radio_value(&mut h.runtime, "".to_string(), "None");
                ui.radio_value(&mut h.runtime, "scout".to_string(), "1.0 (scout)");
                ui.radio_value(&mut h.runtime, "soldier".to_string(), "2.0 (soldier)");
            });
        }

        if h.spec_ver != HANDLER_SPEC_CURRENT_VERSION {
            if ui.button("Update Handler Specification Version").clicked() {
                h.spec_ver = HANDLER_SPEC_CURRENT_VERSION;
                msg("Handler Specification Version Updated", "Remember to save your changes.");
            }
        }

        ui.add_space(8.0);
        let mut save_clicked = false;
        let mut cancel_clicked = false;
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                save_clicked = true;
            }
            if ui.button("Cancel").clicked() {
                cancel_clicked = true;
            }
        });

        // Handle button clicks outside closure to avoid borrow issues
        if save_clicked {
            if let Some(ref mut h) = self.handler_edit {
                if let Err(e) = h.save() {
                    msg("Error saving handler", &format!("{}", e));
                } else {
                    self.handlers = scan_handlers();
                    self.show_edit_modal = false;
                    self.handler_edit = None;
                }
            }
        }
        if cancel_clicked {
            self.show_edit_modal = false;
            self.handler_edit = None;
        }
    }
}
