use super::app::{FocusPane, MenuPage, SettingsCategory, SettingsFocus, Splitux};
use super::theme;
use crate::Handler;
use crate::handler::{import_handler, scan_handlers};
use crate::util::*;

use eframe::egui::Popup;
use eframe::egui::RichText;
use eframe::egui::{self, Ui};
use egui_phosphor::regular as icons;

impl Splitux {
    pub fn display_panel_top(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // === Main Navigation Tabs (2 tabs: Games, Settings) ===
            let games_text = match self.is_lite() {
                true => "Play",
                false => "Games",
            };
            let games_page = match self.is_lite() {
                true => MenuPage::Instances,
                false => MenuPage::Games,
            };

            // Games/Play tab
            let games_selected = self.cur_page == MenuPage::Games || (self.is_lite() && self.cur_page == MenuPage::Instances);
            ui.add_space(4.0);
            let games_btn = ui.add(
                egui::Button::new(games_text)
                    .min_size(egui::vec2(70.0, 28.0))
                    .selected(games_selected),
            );
            if games_btn.clicked() {
                self.cur_page = games_page;
            }

            // Registry tab (only show in full mode, not lite)
            if !self.is_lite() {
                let registry_btn = ui.add(
                    egui::Button::new("Registry")
                        .min_size(egui::vec2(70.0, 28.0))
                        .selected(self.cur_page == MenuPage::Registry),
                );
                if registry_btn.clicked() {
                    self.cur_page = MenuPage::Registry;
                    // Fetch registry if not already loaded
                    if self.registry_index.is_none() && !self.registry_loading {
                        self.fetch_registry();
                    }
                }
            }

            // Settings tab
            let settings_btn = ui.add(
                egui::Button::new("Settings")
                    .min_size(egui::vec2(70.0, 28.0))
                    .selected(self.cur_page == MenuPage::Settings),
            );
            if settings_btn.clicked() {
                self.cur_page = MenuPage::Settings;
            }

            // === Right Side: Version & Close ===
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let close_btn = ui.add(
                    egui::Button::new(icons::X)
                        .min_size(egui::vec2(28.0, 28.0)),
                ).on_hover_text("Close");
                if close_btn.clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }

                ui.add_space(8.0);
                let version_label = if self.needs_update.load(std::sync::atomic::Ordering::Relaxed) {
                    format!("v{} (update available)", env!("CARGO_PKG_VERSION"))
                } else {
                    format!("v{}", env!("CARGO_PKG_VERSION"))
                };
                ui.label(RichText::new(version_label).small().weak());
            });
        });
    }

    /// Display collapsed left panel (just expand button)
    pub fn display_collapsed_games_panel(&mut self, ui: &mut Ui) {
        let label = match self.cur_page {
            MenuPage::Settings => "Settings",
            _ => "Games",
        };

        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            if ui
                .add(egui::Button::new(icons::CARET_RIGHT).min_size(egui::vec2(24.0, 24.0)))
                .on_hover_text(format!("Expand {} panel", label))
                .clicked()
            {
                if self.cur_page == MenuPage::Settings {
                    self.settings_panel_collapsed = false;
                } else {
                    self.games_panel_collapsed = false;
                }
            }
            ui.add_space(8.0);
            // Vertical label
            for ch in label.chars() {
                ui.label(RichText::new(ch.to_string()).small().weak());
            }
        });
    }

    /// Display collapsed devices panel (just expand button)
    pub fn display_collapsed_devices_panel(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            if ui
                .add(egui::Button::new(icons::CARET_LEFT).min_size(egui::vec2(24.0, 24.0)))
                .on_hover_text("Expand Devices panel")
                .clicked()
            {
                self.devices_panel_collapsed = false;
            }
            ui.add_space(8.0);
            // Vertical label
            for ch in "Devices".chars() {
                ui.label(RichText::new(ch.to_string()).small().weak());
            }
        });
    }

    pub fn display_panel_left(&mut self, ui: &mut Ui) {
        match self.cur_page {
            MenuPage::Settings => self.display_panel_left_settings(ui),
            _ => self.display_panel_left_games(ui),
        }
    }

    /// Left panel content for Games page
    fn display_panel_left_games(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        // Header with collapse toggle
        ui.horizontal(|ui| {
            ui.heading("Games");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(icons::CARET_LEFT).min_size(egui::vec2(20.0, 20.0)).frame(false))
                    .on_hover_text("Collapse panel")
                    .clicked()
                {
                    self.games_panel_collapsed = true;
                }
            });
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Game list
            self.panel_left_game_list(ui);

            // Add Game option at the bottom of the list
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Check if bottom buttons are focused
            let is_game_list_focused = self.focus_pane == FocusPane::GameList;
            let add_focused = self.game_panel_bottom_focused && self.game_panel_bottom_index == 0 && is_game_list_focused;
            let import_focused = self.game_panel_bottom_focused && self.game_panel_bottom_index == 1 && is_game_list_focused;

            // Add Game button with focus indicator
            let add_frame = if add_focused {
                egui::Frame::NONE
                    .fill(theme::colors::SELECTION_BG)
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(4, 2))
                    .stroke(theme::focus_stroke())
            } else {
                egui::Frame::NONE.inner_margin(egui::Margin::symmetric(4, 2))
            };
            let add_resp = add_frame.show(ui, |ui| {
                ui.add(
                    egui::Button::new("+ Add Game")
                        .min_size(egui::vec2(ui.available_width(), 28.0))
                        .frame(false),
                )
            });
            if add_resp.inner.clicked() || (add_focused && self.activate_focused) {
                self.handler_edit = Some(Handler::default());
                self.show_edit_modal = true;
            }
            if add_focused {
                add_resp.response.scroll_to_me(Some(egui::Align::Center));
            }

            // Import Handler button with focus indicator
            let import_frame = if import_focused {
                egui::Frame::NONE
                    .fill(theme::colors::SELECTION_BG)
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(4, 2))
                    .stroke(theme::focus_stroke())
            } else {
                egui::Frame::NONE.inner_margin(egui::Margin::symmetric(4, 2))
            };
            let import_resp = import_frame.show(ui, |ui| {
                ui.add(
                    egui::Button::new("Import Handler")
                        .min_size(egui::vec2(ui.available_width(), 28.0))
                        .frame(false),
                )
            });
            if import_resp.inner.clicked() || (import_focused && self.activate_focused) {
                if let Err(e) = import_handler() {
                    msg("Error", &format!("Error importing handler: {}", e));
                } else {
                    self.handlers = scan_handlers();
                }
            }
            if import_focused {
                import_resp.response.scroll_to_me(Some(egui::Align::Center));
            }
        });
    }

    /// Left panel content for Settings page
    fn display_panel_left_settings(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        // Header with collapse toggle
        ui.horizontal(|ui| {
            ui.heading("Settings");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(icons::CARET_LEFT).min_size(egui::vec2(20.0, 20.0)).frame(false))
                    .on_hover_text("Collapse panel")
                    .clicked()
                {
                    self.settings_panel_collapsed = true;
                }
            });
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Category list
        let categories = [
            SettingsCategory::General,
            SettingsCategory::Audio,
            SettingsCategory::Profiles,
            SettingsCategory::Controllers,
            SettingsCategory::ProfileBuilder,
        ];

        for cat in categories {
            let is_selected = self.settings_category == cat;
            let is_focused = self.settings_focus == SettingsFocus::CategoryList
                && self.settings_category == cat;

            let frame = if is_selected {
                egui::Frame::NONE
                    .fill(theme::colors::SELECTION_BG)
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .stroke(if is_focused {
                        theme::focus_stroke()
                    } else {
                        egui::Stroke::new(1.0, theme::colors::ACCENT_DIM)
                    })
            } else {
                egui::Frame::NONE
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .stroke(if is_focused {
                        theme::focus_stroke()
                    } else {
                        egui::Stroke::NONE
                    })
            };

            let frame_resp = frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(
                    egui::Label::new(cat.label())
                        .selectable(false)
                        .sense(egui::Sense::click()),
                )
            });

            // Click on inner label or the frame
            if frame_resp.response.clicked() || frame_resp.inner.clicked() {
                self.settings_category = cat;
                self.settings_focus = SettingsFocus::Options;
                self.settings_option_index = 0;
            }
        }

        // Bottom buttons
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(8.0);
            self.display_settings_bottom_buttons_panel(ui);
        });
    }

    /// Settings bottom buttons for left panel
    fn display_settings_bottom_buttons_panel(&mut self, ui: &mut Ui) {
        use super::config::{save_cfg, SplituxConfig};
        use crate::input::scan_input_devices;

        let is_buttons_focused = self.settings_focus == SettingsFocus::BottomButtons;

        // Restore Defaults button (bottom-up, so this appears at bottom)
        let mut restore_btn = egui::Button::new("Restore Defaults");
        if is_buttons_focused && self.settings_button_index == 1 {
            restore_btn = restore_btn.stroke(theme::focus_stroke());
        }
        let restore_response = ui.add_sized([ui.available_width(), 24.0], restore_btn);
        if restore_response.clicked()
            || (is_buttons_focused && self.settings_button_index == 1 && self.activate_focused)
        {
            self.options = SplituxConfig::default();
            self.input_devices = scan_input_devices(&self.options.pad_filter_type);
            self.refresh_device_display_names();
        }

        ui.add_space(4.0);

        // Save Settings button
        let mut save_btn = egui::Button::new("Save Settings");
        if is_buttons_focused && self.settings_button_index == 0 {
            save_btn = save_btn.stroke(theme::focus_stroke());
        }
        let save_response = ui.add_sized([ui.available_width(), 24.0], save_btn);
        if save_response.clicked()
            || (is_buttons_focused && self.settings_button_index == 0 && self.activate_focused)
        {
            if let Err(e) = save_cfg(&self.options) {
                msg("Error", &format!("Couldn't save settings: {}", e));
            }
        }

        ui.add_space(4.0);
        ui.separator();
    }

    pub fn display_panel_right(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.add_space(8.0);
        // Header with collapse toggle
        ui.horizontal(|ui| {
            ui.heading("Devices");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(icons::CARET_RIGHT).min_size(egui::vec2(20.0, 20.0)).frame(false))
                    .on_hover_text("Collapse panel")
                    .clicked()
                {
                    self.devices_panel_collapsed = true;
                }
            });
        });
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
                    for (idx, pad) in self.input_devices.iter().enumerate() {
                        let display_name = self.device_display_name(idx);
                        let mut dev_text = RichText::new(format!(
                            "{} {}",
                            pad.emoji(),
                            display_name,
                        ));

                        if !pad.enabled() {
                            dev_text = dev_text.weak();
                        } else if pad.has_button_held() {
                            dev_text = dev_text.strong();
                        }

                        ui.label(dev_text);
                    }
                });
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(RichText::new(format!("{} Controller issues?", icons::INFO)).small())
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
                    if ui.add(egui::Button::new(icons::CLIPBOARD).min_size(egui::vec2(24.0, 24.0))).on_hover_text("Copy").clicked() {
                        ctx.copy_text("sudo usermod -aG input $USER".to_string());
                    }
                });
            });
            ui.separator();
        });
    }

    pub fn panel_left_game_list(&mut self, ui: &mut Ui) {
        if self.handlers.is_empty() {
            ui.label(RichText::new("No games yet").italics().color(theme::colors::TEXT_MUTED));
            ui.add_space(4.0);
            ui.label(RichText::new("Add a game below to get started").small().color(theme::colors::TEXT_MUTED));
            return;
        }

        let is_game_list_focused = self.focus_pane == FocusPane::GameList;

        for i in 0..self.handlers.len() {
            // Skip if index is out of bounds to catch for removing/rescanning handlers
            if i >= self.handlers.len() {
                return;
            }

            let is_selected = self.selected_handler == i;
            let show_focus = is_selected && is_game_list_focused;

            // Use card styling for each game entry
            let frame = if is_selected {
                egui::Frame::NONE
                    .fill(theme::colors::SELECTION_BG)
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .stroke(if show_focus {
                        theme::focus_stroke()
                    } else {
                        egui::Stroke::new(1.0, theme::colors::ACCENT_DIM)
                    })
            } else {
                egui::Frame::NONE
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(6, 4))
            };

            frame.show(ui, |ui| {
                let response = ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::new(self.handlers[i].icon())
                            .max_width(18.0)
                            .corner_radius(3),
                    );
                    ui.add_space(4.0);

                    let label = ui.add(
                        egui::Label::new(self.handlers[i].display_clamp())
                            .selectable(false)
                            .sense(egui::Sense::click()),
                    );
                    label
                }).inner;

                if response.clicked() {
                    self.selected_handler = i;
                }
                if response.has_focus() || is_selected {
                    response.scroll_to_me(None);
                }
                Popup::context_menu(&response).show(|ui| self.handler_ctx_menu(ui, i));
            });
            ui.add_space(2.0);
        }
    }

    pub fn handler_ctx_menu(&mut self, ui: &mut Ui, i: usize) {
        if ui.button("Edit").clicked() {
            self.handler_edit = Some(self.handlers[i].clone());
            self.show_edit_modal = true;
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
                    self.cur_page = MenuPage::Games;
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
