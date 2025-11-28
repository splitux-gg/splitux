use super::app::{MenuPage, PartyApp, SettingsPage};
use super::config::*;
use crate::handler::*;
use crate::input::*;
use crate::paths::*;
use crate::profiles::*;
use crate::util::*;
use crate::monitor::get_monitors_sdl;

use dialog::DialogBox;
use eframe::egui::RichText;
use eframe::egui::{self, Ui};
use rfd::FileDialog;
use std::path::PathBuf;

macro_rules! cur_handler {
    ($self:expr) => {
        &$self.handlers[$self.selected_handler]
    };
}

impl PartyApp {
    pub fn display_page_main(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.heading("Welcome to Splitux");
        ui.add_space(4.0);
        ui.label("Local co-op split-screen gaming for Linux");
        ui.add_space(12.0);
        ui.separator();

        // Quick Start Guide
        ui.add_space(8.0);
        ui.label(RichText::new("Getting Started").strong().size(16.0));
        ui.add_space(8.0);

        egui::Grid::new("quick_start_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("1.").strong());
                ui.label("Select a game from the sidebar, or add one with the ➕ button");
                ui.end_row();

                ui.label(RichText::new("2.").strong());
                ui.label("Click Play to enter the instance setup screen");
                ui.end_row();

                ui.label(RichText::new("3.").strong());
                ui.label("Connect controllers and press A/Right-click to create instances");
                ui.end_row();

                ui.label(RichText::new("4.").strong());
                ui.label("Press Start when ready to launch");
                ui.end_row();
            });

        ui.add_space(16.0);
        ui.separator();

        // Controls Reference
        ui.add_space(8.0);
        ui.label(RichText::new("Controls").strong().size(16.0));
        ui.add_space(8.0);

        egui::Grid::new("controls_grid")
            .num_columns(2)
            .spacing([24.0, 6.0])
            .show(ui, |ui| {
                ui.label("Navigate tabs:");
                ui.label("LB/RB  or  Tab");
                ui.end_row();

                ui.label("Select/Confirm:");
                ui.label("A  or  Enter");
                ui.end_row();

                ui.label("Back:");
                ui.label("B  or  Escape");
                ui.end_row();

                ui.label("Navigate UI:");
                ui.label("D-Pad / Left Stick  or  Arrow Keys");
                ui.end_row();
            });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            ui.label("Based on");
            ui.hyperlink_to("PartyDeck", "https://github.com/wunnr/partydeck");
            ui.label("by @wunnr");
        });
    }

    pub fn display_page_settings(&mut self, ui: &mut Ui) {
        self.infotext.clear();
        ui.add_space(8.0);
        ui.heading("Settings");
        ui.add_space(4.0);
        ui.label("Configure Splitux behavior and game launch options");
        ui.add_space(8.0);

        // Sub-tab buttons
        ui.horizontal(|ui| {
            let general_btn = ui.add(
                egui::Button::new("General")
                    .min_size(egui::vec2(100.0, 28.0))
                    .selected(self.settings_page == SettingsPage::General),
            );
            if general_btn.clicked() {
                self.settings_page = SettingsPage::General;
            }

            let gamescope_btn = ui.add(
                egui::Button::new("Gamescope")
                    .min_size(egui::vec2(100.0, 28.0))
                    .selected(self.settings_page == SettingsPage::Gamescope),
            );
            if gamescope_btn.clicked() {
                self.settings_page = SettingsPage::Gamescope;
            }
        });
        ui.add_space(4.0);
        ui.separator();

        match self.settings_page {
            SettingsPage::General => self.display_settings_general(ui),
            SettingsPage::Gamescope => self.display_settings_gamescope(ui),
        }

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

    pub fn display_page_profiles(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.heading("Profiles");
        ui.add_space(4.0);
        ui.label("Manage player profiles for persistent saves, settings, and stats");
        ui.add_space(8.0);
        ui.separator();

        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 60.0)
            .auto_shrink(false)
            .show(ui, |ui| {
                if self.profiles.is_empty() {
                    ui.label("No profiles yet. Create one below to get started.");
                } else {
                    for profile in &self.profiles {
                        if ui.add(
                            egui::Button::new(format!("👤  {}", profile))
                                .min_size(egui::vec2(200.0, 28.0))
                                .frame(false),
                        ).on_hover_text("Click to open profile folder").clicked() {
                            if let Err(_) = std::process::Command::new("xdg-open")
                                .arg(PATH_PARTY.join("profiles").join(profile))
                                .status()
                            {
                                msg("Error", "Couldn't open profile directory!");
                            }
                        }
                    }
                }
            });

        ui.add_space(8.0);
        if ui.add(
            egui::Button::new("➕ New Profile")
                .min_size(egui::vec2(120.0, 32.0)),
        ).clicked() {
            if let Some(name) = dialog::Input::new("Enter name (must be alphanumeric):")
                .title("New Profile")
                .show()
                .expect("Could not display dialog box")
            {
                if !name.is_empty() && name.chars().all(char::is_alphanumeric) {
                    create_profile(&name).unwrap();
                } else {
                    msg("Error", "Invalid name");
                }
            }
            self.profiles = scan_profiles(false);
        }
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
            if h.is_saved_handler() && ui.button("🖼").clicked() {
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

            ui.checkbox(&mut h.use_goldberg, "Emulate Steam Client");
        });

        h.steam_appid = match &self.installed_steamapps[selected_index] {
            Some(app) => Some(app.app_id),
            None => None,
        };

        if h.steam_appid == None {
            ui.horizontal(|ui| {
                ui.label("Game root folder:");
                ui.add_enabled(false, egui::TextEdit::singleline(&mut h.path_gameroot));
                if ui.button("🗁").clicked() {
                    if let Ok(path) = dir_dialog() {
                        h.path_gameroot = path.to_string_lossy().to_string();
                    }
                }
            });
        }

        ui.horizontal(|ui| {
            ui.label("Executable:");
            ui.add_enabled(false, egui::TextEdit::singleline(&mut h.exec));
            if ui.button("🗁").clicked() {
                if let Ok(base_path) = h.get_game_rootpath()
                    && let Ok(path) = file_dialog_relative(&PathBuf::from(base_path))
                {
                    h.exec = path.to_string_lossy().to_string();
                }
            }
        });

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

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            if ui.button("Save").clicked() {
                if let Err(e) = h.save() {
                    msg("Error saving handler", &format!("{}", e));
                } else {
                    self.handlers = scan_handlers();
                    self.cur_page = MenuPage::Game;
                }
            }
        });
    }

    pub fn display_page_game(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(cur_handler!(self).icon()).max_width(32.0).corner_radius(4));
            ui.add_space(8.0);
            ui.heading(cur_handler!(self).display());
        });
        ui.add_space(8.0);
        ui.separator();

        let h = cur_handler!(self);

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let playbtn = ui.add(
                egui::Button::image_and_text(
                    egui::include_image!("../../res/BTN_START.png"),
                    " Play ",
                )
                .min_size(egui::vec2(100.0, 32.0)),
            );
            if playbtn.clicked() {
                if h.spec_ver != HANDLER_SPEC_CURRENT_VERSION {
                    let mismatch = match h.spec_ver < HANDLER_SPEC_CURRENT_VERSION {
                        true => "an older",
                        false => "a newer",
                    };
                    let mismatch2 = match h.spec_ver < HANDLER_SPEC_CURRENT_VERSION {
                        true => "Up-to-date handlers can be found by clicking the ⮋ button on the top bar of the launcher.",
                        false => "It is recommended to update Splitux to the latest version.",
                    };
                    msg(
                        "Handler version mismatch",
                        &format!("This handler was meant for use with {} version of Splitux; you may experience issues or the game may not work at all. {} If everything still works fine, you can prevent this message appearing in the future by editing the handler, updating the spec version and saving.",
                            mismatch, mismatch2
                        )
                    );
                }
                if h.steam_appid.is_none() && h.path_gameroot.is_empty() {
                    msg(
                        "Game root path not found",
                        "Please specify the game's root folder.",
                    );
                    self.handler_edit = Some(h.clone());
                    self.cur_page = MenuPage::EditHandler;
                } else {
                    self.instances.clear();
                    self.input_devices = scan_input_devices(&self.options.pad_filter_type);
                    self.monitors = get_monitors_sdl();
                    self.profiles = scan_profiles(true);
                    self.instance_add_dev = None;
                    self.cur_page = MenuPage::Instances;
                }
            }

            ui.add(egui::Separator::default().vertical());
            if h.win() {
                ui.label(" Proton");
            } else {
                ui.label("🐧 Native");
            }
            if !h.author.is_empty() {
                ui.add(egui::Separator::default().vertical());
                ui.label(format!("Author: {}", h.author));
            }
            if !h.version.is_empty() {
                ui.add(egui::Separator::default().vertical());
                ui.label(format!("Version: {}", h.version));
            }
        });

        egui::ScrollArea::horizontal()
            .max_width(f32::INFINITY)
            .show(ui, |ui| {
                let available_height = ui.available_height();
                ui.horizontal(|ui| {
                    for img in h.img_paths.iter() {
                        ui.add(
                            egui::Image::new(format!("file://{}", img.display()))
                                .fit_to_exact_size(egui::vec2(
                                    available_height * 1.77,
                                    available_height,
                                ))
                                .maintain_aspect_ratio(true),
                        );
                    }
                });
            });
    }

    pub fn display_page_instances(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.heading("Instance Setup");
        ui.add_space(4.0);
        ui.label("Connect your controllers and assign them to player instances");
        ui.add_space(8.0);
        ui.separator();

        // Controls help bar
        ui.add_space(8.0);
        egui::Frame::NONE
            .fill(ui.visuals().extreme_bg_color)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Add instance control
                    ui.add(egui::Image::new(egui::include_image!("../../res/BTN_SOUTH.png")).max_height(16.0));
                    ui.label(" / Z / Right-Click:");
                    let add_text = match self.instance_add_dev {
                        None => "Add Instance",
                        Some(i) => &format!("Add to P{}", i + 1),
                    };
                    ui.label(RichText::new(add_text).strong());

                    ui.add_space(16.0);
                    ui.add(egui::Separator::default().vertical());
                    ui.add_space(16.0);

                    // Remove/Cancel control
                    ui.add(egui::Image::new(egui::include_image!("../../res/BTN_EAST.png")).max_height(16.0));
                    ui.label(" / X:");
                    let remove_text = match self.instance_add_dev {
                        None => "Remove",
                        Some(_) => "Cancel",
                    };
                    ui.label(RichText::new(remove_text).strong());

                    ui.add_space(16.0);
                    ui.add(egui::Separator::default().vertical());
                    ui.add_space(16.0);

                    // Invite control
                    ui.add(egui::Image::new(egui::include_image!("../../res/BTN_NORTH.png")).max_height(16.0));
                    ui.label(" / A:");
                    ui.label(RichText::new("Invite Device").strong());
                });
            });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        let mut devices_to_remove: Vec<(usize, usize)> = Vec::new();

        if self.instances.is_empty() {
            ui.add_space(16.0);
            ui.label(RichText::new("No instances yet").italics());
            ui.add_space(4.0);
            ui.label("Press A or Right-click on a controller to create a player instance");
        }

        for (i, instance) in &mut self.instances.iter_mut().enumerate() {
            egui::Frame::NONE
                .fill(ui.visuals().faint_bg_color)
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("P{}", i + 1)).strong().size(18.0));
                        ui.add_space(8.0);

                        ui.label("Profile:");
                        egui::ComboBox::from_id_salt(format!("{i}"))
                            .width(120.0)
                            .show_index(
                                ui,
                                &mut instance.profselection,
                                self.profiles.len(),
                                |i| self.profiles[i].clone(),
                            );

                        if self.options.gamescope_sdl_backend {
                            ui.add_space(8.0);
                            ui.label("Monitor:");
                            egui::ComboBox::from_id_salt(format!("monitors{i}"))
                                .width(100.0)
                                .show_index(
                                    ui,
                                    &mut instance.monitor,
                                    self.monitors.len(),
                                    |i| self.monitors[i].name(),
                                );
                        }

                        ui.add_space(8.0);
                        if self.instance_add_dev == None {
                            let invitebtn = ui.add(
                                egui::Button::image_and_text(
                                    egui::include_image!("../../res/BTN_NORTH.png"),
                                    " Invite Device",
                                )
                                .min_size(egui::vec2(0.0, 26.0)),
                            );
                            if invitebtn.clicked() {
                                self.instance_add_dev = Some(i);
                            }
                        } else if self.instance_add_dev == Some(i) {
                            ui.label(RichText::new("Waiting for input...").italics());
                            if ui.add(egui::Button::new("✕").min_size(egui::vec2(26.0, 26.0))).clicked() {
                                self.instance_add_dev = None;
                            }
                        }
                    });

                    // Device list
                    for &dev in instance.devices.iter() {
                        let mut dev_text = RichText::new(format!(
                            "   {} {}",
                            self.input_devices[dev].emoji(),
                            self.input_devices[dev].fancyname()
                        ));

                        if self.input_devices[dev].has_button_held() {
                            dev_text = dev_text.strong();
                        }

                        ui.horizontal(|ui| {
                            ui.label(dev_text);
                            if ui.add(egui::Button::new("🗑").min_size(egui::vec2(24.0, 24.0))).on_hover_text("Remove device").clicked() {
                                devices_to_remove.push((i, dev));
                            }
                        });
                    }
                });
            ui.add_space(4.0);
        }

        for (i, d) in devices_to_remove {
            self.remove_device_instance(i, d);
        }

        if self.instances.len() > 0 {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let start_btn = ui.add(
                    egui::Button::image_and_text(
                        egui::include_image!("../../res/BTN_START.png"),
                        "  Start Game  ",
                    )
                    .min_size(egui::vec2(160.0, 40.0)),
                );
                if start_btn.clicked() {
                    self.prepare_game_launch();
                }
                ui.add_space(4.0);
                ui.separator();
            });
        }
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
