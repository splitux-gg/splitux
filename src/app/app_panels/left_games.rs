use crate::app::app::{FocusPane, MenuPage, Splitux};
use crate::ui::theme;
use crate::Handler;
use crate::handler::{import_handler, scan_handlers};
use crate::util::*;

use eframe::egui::Popup;
use eframe::egui::RichText;
use eframe::egui::{self, Ui};
use egui_phosphor::regular as icons;

impl Splitux {
    /// Left panel content for Games page
    pub(super) fn display_panel_left_games(&mut self, ui: &mut Ui) {
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
