use super::app::{MenuPage, PartyApp};
use crate::Handler;
use crate::handler::import_handler;
use crate::handler::scan_handlers;
use crate::input::*;
use crate::monitor::get_monitors_sdl;
use crate::profiles::scan_profiles;
use crate::util::*;

use eframe::egui::Popup;
use eframe::egui::RichText;
use eframe::egui::{self, Ui};

macro_rules! cur_handler {
    ($self:expr) => {
        &$self.handlers[$self.selected_handler]
    };
}

impl PartyApp {
    pub fn display_panel_top(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // === Main Navigation Tabs ===
            let hometext = match self.is_lite() {
                true => " Play  [B]",
                false => " Home  [B]",
            };
            let homepage = match self.is_lite() {
                true => MenuPage::Instances,
                false => MenuPage::Home,
            };

            // Home/Play tab
            let home_selected = self.cur_page == MenuPage::Home || (self.is_lite() && self.cur_page == MenuPage::Instances);
            ui.add_space(4.0);
            let homebtn = ui.add(
                egui::Button::image_and_text(
                    egui::include_image!("../../res/BTN_EAST.png"),
                    hometext,
                )
                .min_size(egui::vec2(100.0, 28.0))
                .selected(home_selected),
            );
            if homebtn.clicked() {
                self.cur_page = homepage;
            }

            // Profiles tab
            let profilesbtn = ui.add(
                egui::Button::image_and_text(
                    egui::include_image!("../../res/BTN_WEST.png"),
                    " Profiles  [X]",
                )
                .min_size(egui::vec2(110.0, 28.0))
                .selected(self.cur_page == MenuPage::Profiles),
            );
            if profilesbtn.clicked() {
                self.profiles = scan_profiles(false);
                self.cur_page = MenuPage::Profiles;
            }

            // Settings tab
            let settingsbtn = ui.add(
                egui::Button::image_and_text(
                    egui::include_image!("../../res/BTN_NORTH.png"),
                    " Settings  [Y]",
                )
                .min_size(egui::vec2(110.0, 28.0))
                .selected(self.cur_page == MenuPage::Settings),
            );
            if settingsbtn.clicked() {
                self.cur_page = MenuPage::Settings;
            }

            ui.add(egui::Separator::default().vertical());

            // === Utility Actions ===
            let refresh_devs = ui.add(
                egui::Button::new("🎮")
                    .min_size(egui::vec2(28.0, 28.0)),
            ).on_hover_text("Refresh Controllers");
            if refresh_devs.clicked() {
                self.instances.clear();
                self.input_devices = scan_input_devices(&self.options.pad_filter_type);
            }

            let refresh_monitors = ui.add(
                egui::Button::new("🖵")
                    .min_size(egui::vec2(28.0, 28.0)),
            ).on_hover_text("Refresh Monitors");
            if refresh_monitors.clicked() {
                self.instances.clear();
                self.monitors = get_monitors_sdl();
            }

            // === Right Side: Links & Close ===
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let close_btn = ui.add(
                    egui::Button::new("✕")
                        .min_size(egui::vec2(28.0, 28.0)),
                ).on_hover_text("Close");
                if close_btn.clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                ui.add(egui::Separator::default().vertical());
                let version_label = match self.needs_update {
                    true => format!("v{} 🆕", env!("CARGO_PKG_VERSION")),
                    false => format!("v{}", env!("CARGO_PKG_VERSION")),
                };
                ui.hyperlink_to(version_label, "https://github.com/gabrielgad/splitux/releases")
                    .on_hover_text("View Releases");

                ui.add(egui::Separator::default().vertical());
                ui.hyperlink_to("⮋", "https://drive.proton.me/urls/D9HBKM18YR#zG8XC8yVy9WL")
                    .on_hover_text("Download Game Handlers");
                ui.hyperlink_to("♥", "https://ko-fi.com/wunner")
                    .on_hover_text("Support Development");
                ui.hyperlink_to("📋", "https://github.com/gabrielgad/splitux/tree/main?tab=License-2-ov-file")
                    .on_hover_text("Licenses");
                ui.hyperlink_to("", "https://github.com/gabrielgad/splitux")
                    .on_hover_text("GitHub");
            });
        });
    }

    pub fn display_panel_left(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading("Games");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let add_btn = ui.add(
                    egui::Button::new("➕")
                        .min_size(egui::vec2(26.0, 26.0)),
                ).on_hover_text("Add new game handler");
                if add_btn.clicked() {
                    self.handler_edit = Some(Handler::default());
                    self.cur_page = MenuPage::EditHandler;
                }

                let import_btn = ui.add(
                    egui::Button::new("⬇")
                        .min_size(egui::vec2(26.0, 26.0)),
                ).on_hover_text("Import handler (.spx)");
                if import_btn.clicked() {
                    if let Err(e) = import_handler() {
                        msg("Error", &format!("Error importing handler: {}", e));
                    } else {
                        self.handlers = scan_handlers();
                    }
                }

                let refresh_btn = ui.add(
                    egui::Button::new("🔄")
                        .min_size(egui::vec2(26.0, 26.0)),
                ).on_hover_text("Refresh handlers");
                if refresh_btn.clicked() {
                    self.handlers = scan_handlers();
                }
            });
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        if self.handlers.is_empty() {
            ui.label(RichText::new("No games yet").italics().weak());
            ui.add_space(4.0);
            ui.label(RichText::new("Click ➕ to add a game").small().weak());
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.panel_left_game_list(ui);
            });
        }
    }

    pub fn display_panel_bottom(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("info_panel")
            .exact_height(100.0)
            .show(ctx, |ui| {
                if self.task.is_some() {
                    ui.disable();
                }
                match self.cur_page {
                    MenuPage::Game => {
                        self.infotext = cur_handler!(self).info.to_owned();
                    }
                    MenuPage::Profiles => {
                        self.infotext = "Create profiles to persistently store game save data, settings, and stats.".to_string();
                    }
                    _ => {}
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.cur_page == MenuPage::EditHandler && let Some(handler) = &mut self.handler_edit {
                        ui.add(egui::TextEdit::multiline(&mut handler.info).hint_text("Put game info/instructions here"));
                    } else {
                        ui.label(&self.infotext);
                    }
                });
            });
    }

    pub fn display_panel_right(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.add_space(8.0);
        ui.heading("Devices");
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        let enabled_count = self.input_devices.iter().filter(|d| d.enabled()).count();
        if enabled_count == 0 {
            ui.label(RichText::new("No devices detected").italics().weak());
            ui.add_space(4.0);
            ui.label(RichText::new("Connect a controller").small().weak());
        } else {
            ui.label(RichText::new(format!("{} device(s) ready", enabled_count)).small());
            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - 80.0)
                .show(ui, |ui| {
                    for pad in self.input_devices.iter() {
                        let event_num = pad.path().trim_start_matches("/dev/input/event");
                        let mut dev_text = RichText::new(format!(
                            "{} {}",
                            pad.emoji(),
                            pad.fancyname(),
                        ));

                        if !pad.enabled() {
                            dev_text = dev_text.weak();
                        } else if pad.has_button_held() {
                            dev_text = dev_text.strong();
                        }

                        ui.horizontal(|ui| {
                            ui.label(dev_text);
                            ui.label(RichText::new(format!("({})", event_num)).small().weak());
                        });
                    }
                });
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(RichText::new("ℹ Controller issues?").small())
                    .selectable(false)
                    .sense(egui::Sense::click()),
            ).on_hover_ui(|ui| {
                ui.set_max_width(250.0);
                ui.label(RichText::new("Incorrect mappings?").strong());
                ui.label("Edit the handler and change SDL2 Override to \"Steam Runtime\" (32-bit) or \"System Installation\" (64-bit).");
                ui.add_space(8.0);
                ui.label(RichText::new("Devices not detected?").strong());
                ui.label("Add your user to the input group:");
                ui.horizontal(|ui| {
                    ui.code("sudo usermod -aG input $USER");
                    if ui.add(egui::Button::new("📋").min_size(egui::vec2(24.0, 24.0))).on_hover_text("Copy").clicked() {
                        ctx.copy_text("sudo usermod -aG input $USER".to_string());
                    }
                });
            });
            ui.separator();
        });
    }

    pub fn panel_left_game_list(&mut self, ui: &mut Ui) {
        for i in 0..self.handlers.len() {
            // Skip if index is out of bounds to catch for removing/rescanning handlers
            if i >= self.handlers.len() {
                return;
            }

            ui.horizontal(|ui| {
                ui.add(
                    egui::Image::new(self.handlers[i].icon())
                        .max_width(16.0)
                        .corner_radius(2),
                );

                let btn = ui.selectable_value(
                    &mut self.selected_handler,
                    i,
                    self.handlers[i].display_clamp(),
                );
                if btn.has_focus() {
                    btn.scroll_to_me(None);
                }
                if btn.clicked() {
                    self.cur_page = MenuPage::Game;
                };

                Popup::context_menu(&btn).show(|ui| self.handler_ctx_menu(ui, i));
            });
        }
    }

    pub fn handler_ctx_menu(&mut self, ui: &mut Ui, i: usize) {
        if ui.button("Edit").clicked() {
            self.handler_edit = Some(self.handlers[i].clone());
            self.cur_page = MenuPage::EditHandler;
        }

        if ui.button("Open Folder").clicked() {
            if let Err(_) = std::process::Command::new("xdg-open")
                .arg(self.handlers[i].path_handler.clone())
                .status()
            {
                msg("Error", "Couldn't open handler folder!");
            }
        }

        if ui.button("Remove").clicked() {
            if yesno(
                "Remove handler?",
                &format!(
                    "Are you sure you want to remove {}?",
                    self.handlers[i].display()
                ),
            ) {
                if let Err(err) = self.handlers[i].remove_handler() {
                    println!("[splitux] Failed to remove handler: {}", err);
                    msg("Error", &format!("Failed to remove handler: {}", err));
                }

                self.handlers = scan_handlers();
                if self.handlers.is_empty() {
                    self.cur_page = MenuPage::Home;
                }
                if i >= self.handlers.len() {
                    self.selected_handler = 0;
                }
            }
        }

        if ui.button("Export").clicked() {
            if let Err(err) = self.handlers[i].export() {
                println!("[splitux] Failed to export handler: {}", err);
                msg("Error", &format!("Failed to export handler: {}", err));
            }
        }
    }
}
