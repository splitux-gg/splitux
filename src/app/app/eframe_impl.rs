//! eframe::App implementation for Splitux

use super::Splitux;
use crate::ui::MenuPage;
use eframe::egui;

impl eframe::App for Splitux {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if !raw_input.focused || self.task.is_some() {
            return;
        }
        match self.cur_page {
            MenuPage::Instances => self.handle_devices_instance_menu(),
            _ => self.handle_gamepad_gui(ctx, raw_input),
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Paint full-screen background to fill any gaps between panels
        let screen_rect = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(screen_rect, 0.0, crate::app::theme::colors::BG_DARK);

        // Poll for device hotplug events
        self.poll_device_events();

        // Poll for monitor changes (hotplug, resolution changes)
        self.poll_monitor_events();

        // Reset focus state at start of frame
        self.focus_manager.begin_frame();

        // Enable keyboard focus navigation
        ctx.options_mut(|opt| {
            opt.input_options.line_scroll_speed = 40.0;
        });

        egui::TopBottomPanel::top("menu_nav_panel")
            .frame(
                egui::Frame::NONE
                    .fill(crate::app::theme::colors::BG_MID)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ctx, |ui| {
                if self.task.is_some() {
                    ui.disable();
                }
                self.display_panel_top(ui);
            });

        // Left panel - Games list (collapsible/resizable)
        if !self.is_lite() {
            let collapsed = self.games_panel_collapsed;
            let (width, width_range) = if collapsed {
                (36.0, 36.0..=36.0)
            } else {
                (self.games_panel_width, 120.0..=280.0)
            };

            egui::SidePanel::left("games_panel")
                .resizable(!collapsed)
                .default_width(width)
                .width_range(width_range)
                .frame(
                    egui::Frame::NONE
                        .fill(crate::app::theme::colors::BG_MID)
                        .inner_margin(if collapsed {
                            egui::Margin::symmetric(4, 8)
                        } else {
                            egui::Margin::same(8)
                        })
                        .stroke(egui::Stroke::new(1.0, crate::app::theme::colors::BG_LIGHT)),
                )
                .show_separator_line(false)
                .show(ctx, |ui| {
                    if self.task.is_some() {
                        ui.disable();
                    }
                    if collapsed {
                        self.display_collapsed_games_panel(ui);
                    } else {
                        // Track width changes for persistence
                        let panel_width = ui.available_width() + 16.0; // Account for margins
                        if (panel_width - self.games_panel_width).abs() > 2.0 {
                            self.games_panel_width = panel_width;
                        }
                        self.display_panel_left(ui);
                    }
                });
        }

        // Right panel - Devices (collapsible/resizable, only on Instances page)
        if self.cur_page == MenuPage::Instances {
            let collapsed = self.devices_panel_collapsed;
            let (width, width_range) = if collapsed {
                (36.0, 36.0..=36.0)
            } else {
                (self.devices_panel_width, 150.0..=320.0)
            };

            egui::SidePanel::right("devices_panel")
                .resizable(!collapsed)
                .default_width(width)
                .width_range(width_range)
                .frame(
                    egui::Frame::NONE
                        .fill(crate::app::theme::colors::BG_MID)
                        .inner_margin(if collapsed {
                            egui::Margin::symmetric(4, 8)
                        } else {
                            egui::Margin {
                                left: 16,
                                right: 8,
                                top: 8,
                                bottom: 8,
                            }
                        })
                        .stroke(egui::Stroke::new(1.0, crate::app::theme::colors::BG_LIGHT)),
                )
                .show_separator_line(false)
                .show(ctx, |ui| {
                    if self.task.is_some() {
                        ui.disable();
                    }
                    if collapsed {
                        self.display_collapsed_devices_panel(ui);
                    } else {
                        // Track width changes for persistence
                        let panel_width = ui.available_width() + 24.0; // Account for margins
                        if (panel_width - self.devices_panel_width).abs() > 2.0 {
                            self.devices_panel_width = panel_width;
                        }
                        self.display_panel_right(ui, ctx);
                    }
                });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(crate::app::theme::colors::BG_DARK)
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 0,
                        bottom: 8,
                    }),
            )
            .show(ctx, |ui| {
                if self.task.is_some() {
                    ui.disable();
                }

                // Show permission banner at top if needed (only on Games/Instances pages)
                if matches!(self.cur_page, MenuPage::Games | MenuPage::Instances) {
                    ui.add_space(8.0);
                    self.display_permission_banner(ui);
                }

                match self.cur_page {
                    MenuPage::Games => self.display_page_games(ui),
                    MenuPage::Registry => self.display_page_registry(ui),
                    MenuPage::Settings => self.display_page_settings(ui),
                    MenuPage::Instances => self.display_page_instances(ui),
                }
            });

        // Edit handler modal
        if self.show_edit_modal {
            self.display_edit_handler_modal(ctx);
        }

        // Profile dropdown overlay
        if self.profile_dropdown_open {
            self.display_profile_dropdown(ctx);
        }

        // New profile dialog
        if self.show_new_profile_dialog {
            self.display_new_profile_dialog(ctx);
        }

        if let Some(handle) = self.task.take() {
            if handle.is_finished() {
                let _ = handle.join();
                self.loading_since = None;
                self.loading_msg = None;
            } else {
                self.task = Some(handle);
            }
        }
        if let Some(start) = self.loading_since {
            if start.elapsed() > std::time::Duration::from_secs(60) {
                // Give up waiting after one minute
                self.loading_msg = Some("Operation timed out".to_string());
            }
        }
        if let Some(msg) = &self.loading_msg {
            egui::Area::new("loading".into())
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 192))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(16, 12))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add(egui::widgets::Spinner::new().size(40.0));
                                ui.add_space(8.0);
                                ui.label(msg);
                            });
                        });
                });
        }
        if ctx.input(|input| input.focused) {
            ctx.request_repaint_after(std::time::Duration::from_millis(33)); // 30 fps
        }
    }
}
