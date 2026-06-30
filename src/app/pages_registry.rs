// Registry page - browse and download handlers from online registry

use super::app::{RegistryFocus, Splitux};
use crate::ui::theme;
use crate::handler::scan_handlers;
use crate::registry::{download_handler, fetch_registry, RegistryEntry};
use crate::ui::responsive::LayoutMode;
use eframe::egui::{self, RichText, Ui};

impl Splitux {
    pub fn display_page_registry(&mut self, ui: &mut Ui) {
        // Show loading state
        if self.registry_loading {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.add(egui::widgets::Spinner::new().size(40.0));
                ui.add_space(8.0);
                ui.label("Loading registry...");
            });
            return;
        }

        // Show error state
        if let Some(error) = self.registry_error.clone() {
            ui.add_space(16.0);
            ui.label(RichText::new("Failed to load registry").strong().color(theme::colors::ERROR));
            ui.add_space(4.0);
            ui.label(RichText::new(&error).small().color(theme::colors::TEXT_MUTED));
            ui.add_space(12.0);
            if ui.button("Retry").clicked() {
                self.fetch_registry();
            }
            return;
        }

        // Show empty state if no registry loaded
        let Some(index) = &self.registry_index else {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.label("Registry not loaded");
                ui.add_space(8.0);
                if ui.button("Load Registry").clicked() {
                    self.fetch_registry();
                }
            });
            return;
        };

        // Clone handlers list for display to avoid borrow issues
        let handlers: Vec<RegistryEntry> = index.handlers.clone();

        if handlers.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No handlers available").italics());
                ui.add_space(4.0);
                ui.label(RichText::new("Check back later or contribute your own!").small().color(theme::colors::TEXT_MUTED));
            });
            return;
        }

        // Filter handlers based on search
        let search_lower = self.registry_search.to_lowercase();
        let filtered_handlers: Vec<(usize, &RegistryEntry)> = handlers
            .iter()
            .enumerate()
            .filter(|(_, h)| {
                search_lower.is_empty()
                    || h.name.to_lowercase().contains(&search_lower)
                    || h.description.to_lowercase().contains(&search_lower)
                    || h.author.to_lowercase().contains(&search_lower)
            })
            .collect();

        // Calculate available height for the scroll area
        let available_height = ui.available_height();
        let layout_mode = LayoutMode::from_ui(ui);
        let is_narrow = layout_mode.is_narrow();

        // Responsive layout - stacked in narrow mode, columns in wide/medium
        if is_narrow {
            // Single column stacked layout
            // Search box
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.add(
                    egui::TextEdit::singleline(&mut self.registry_search)
                        .desired_width(ui.available_width() - 30.0)
                        .hint_text("Filter..."),
                );
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Handler list (compact height in narrow mode)
            let is_list_focused = self.registry_focus == RegistryFocus::HandlerList;
            let scroll_height = 150.0;
            egui::ScrollArea::vertical()
                .id_salt("handler_list_narrow")
                .max_height(scroll_height)
                .show(ui, |ui| {
                    self.display_registry_handler_list(ui, &filtered_handlers, is_list_focused);
                });

            ui.add_space(8.0);
            ui.separator();

            // Details below (scrollable)
            egui::ScrollArea::vertical()
                .id_salt("handler_details_narrow")
                .max_height(available_height - scroll_height - 100.0)
                .show(ui, |ui| {
                    if let Some(selected_idx) = self.registry_selected {
                        if let Some(entry) = handlers.get(selected_idx) {
                            self.display_registry_handler_details(ui, entry);
                        }
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label(RichText::new("Select a handler").italics().color(theme::colors::TEXT_MUTED));
                        });
                    }
                });
        } else {
            // Two-column layout
            ui.columns(2, |columns| {
                // Left column: handler list
                columns[0].vertical(|ui| {
                    // Search box
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Search:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.registry_search)
                                .desired_width(140.0)
                                .hint_text("Filter..."),
                        );
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Handler list with explicit height
                    let is_list_focused = self.registry_focus == RegistryFocus::HandlerList;
                    let scroll_height = (available_height - 80.0).max(200.0);
                    egui::ScrollArea::vertical()
                        .max_height(scroll_height)
                        .show(ui, |ui| {
                            self.display_registry_handler_list(ui, &filtered_handlers, is_list_focused);
                        });
                });

                // Right column: selected handler details
                columns[1].vertical(|ui| {
                    ui.add_space(8.0);

                    if let Some(selected_idx) = self.registry_selected {
                        if let Some(entry) = handlers.get(selected_idx) {
                            self.display_registry_handler_details(ui, entry);
                        }
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.label(RichText::new("Select a handler from the list").italics().color(theme::colors::TEXT_MUTED));
                        });
                    }
                });
            });
        }
    }

    /// Helper to display the handler list (used by both layouts)
    /// Find the locally-installed handler matching a registry entry.
    ///
    /// Registry ids are kebab-case (`deep-rock-galactic`) but local handler dirs
    /// are often PascalCase (`DeepRockGalactic`), so a plain id/dir comparison
    /// misses. Match on the stable **Steam appid** first, then a
    /// separator/case-insensitive id fallback. This is what lets the registry use
    /// the REAL local art + correct "installed" state (your machine is the source
    /// of truth) instead of depending on the remote CDN by id.
    fn registry_local_handler(&self, entry: &RegistryEntry) -> Option<usize> {
        if let Some(appid) = entry.steam_appid
            && let Some(i) = self.handlers.iter().position(|h| h.steam_appid == Some(appid)) {
                return Some(i);
            }
        let norm = |s: &str| -> String {
            s.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .map(|c| c.to_ascii_lowercase())
                .collect()
        };
        let want = norm(&entry.id);
        self.handlers
            .iter()
            .position(|h| norm(h.handler_dir_name()) == want)
    }

    fn display_registry_handler_list(
        &mut self,
        ui: &mut Ui,
        filtered_handlers: &[(usize, &RegistryEntry)],
        is_list_focused: bool,
    ) {
        for (original_idx, entry) in filtered_handlers {
            let is_selected = self.registry_selected == Some(*original_idx);
            let local = self.registry_local_handler(entry);
            let is_installed = local.is_some() || entry.is_installed();
            let show_focus = is_selected && is_list_focused;

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

            // The whole highlighted row is the click target (not just the text),
            // so there's no dead gap between entries: force the frame full-width,
            // keep the inner widgets non-interactive, and sense clicks on the
            // frame's full rect.
            let frame_resp = frame
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        // Icon: prefer the LOCAL handler's art (source of truth, always
                        // present once installed), else the registry CDN, else the
                        // app's generic icon (never egui's broken-image ⚠️).
                        if let Some(li) = local {
                            ui.add(
                                egui::Image::new(self.handlers[li].icon())
                                    .max_width(18.0)
                                    .corner_radius(3),
                            );
                        } else {
                            let icon_url = entry.icon_url();
                            let icon_ready = matches!(
                                ui.ctx().try_load_texture(
                                    &icon_url,
                                    egui::TextureOptions::default(),
                                    egui::SizeHint::Scale(1.0.into()),
                                ),
                                Ok(egui::load::TexturePoll::Ready { .. })
                            );
                            if icon_ready {
                                ui.add(
                                    egui::Image::new(&icon_url).max_width(18.0).corner_radius(3),
                                );
                            } else {
                                ui.add(
                                    egui::Image::new(egui::include_image!(
                                        "../../assets/executable_icon.png"
                                    ))
                                    .max_width(18.0)
                                    .corner_radius(3),
                                );
                            }
                        }
                        ui.add_space(4.0);

                        // Name with installed indicator
                        let mut name_text = RichText::new(&entry.name);
                        if is_installed {
                            name_text = name_text.color(theme::colors::SUCCESS);
                        }
                        ui.add(egui::Label::new(name_text).selectable(false));
                    });
                })
                .response
                .interact(egui::Sense::click());

            if frame_resp.clicked() {
                self.registry_selected = Some(*original_idx);
            }
            // Auto-scroll to keep focused handler visible — but ONLY when the
            // selection actually changed (keyboard/gamepad nav), not every frame.
            // Otherwise the constant re-centering fights the mouse wheel and the
            // list feels locked to the selection.
            if show_focus && self.registry_scrolled_idx != Some(*original_idx) {
                frame_resp.scroll_to_me(Some(egui::Align::Center));
                self.registry_scrolled_idx = Some(*original_idx);
            }
            // No inter-row spacer: contiguous rows mean a click can't fall
            // "between" two entries and select neither.
        }
    }

    fn display_registry_handler_details(&mut self, ui: &mut Ui, entry: &RegistryEntry) {
        let local = self.registry_local_handler(entry);
        let is_installed = local.is_some() || entry.is_installed();
        let is_installing = self.registry_installing.as_ref() == Some(&entry.id);

        // Wide banner (hero): prefer the LOCAL handler's banner; else the CDN.
        // Probe the load and only draw when ready, so a not-yet-present banner
        // shows nothing rather than egui's broken-image ⚠️.
        let local_hero = local.and_then(|li| self.handlers[li].hero_image());
        let hero_uri = local_hero.unwrap_or_else(|| entry.hero_url());
        let hero_ready = matches!(
            ui.ctx().try_load_texture(
                &hero_uri,
                egui::TextureOptions::default(),
                egui::SizeHint::Scale(1.0.into()),
            ),
            Ok(egui::load::TexturePoll::Ready { .. })
        );
        if hero_ready {
            ui.add(
                egui::Image::new(&hero_uri)
                    .max_width(ui.available_width())
                    .max_height(140.0)
                    .corner_radius(4),
            );
            ui.add_space(8.0);
        }

        // Name and metadata
        ui.heading(&entry.name);
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("by {}", entry.author)).small().color(theme::colors::TEXT_MUTED));

            if let Some(appid) = entry.steam_appid {
                ui.add_space(8.0);
                ui.label(RichText::new(format!("Steam: {}", appid)).small().color(theme::colors::TEXT_MUTED));
            }

            if let Some(backend) = &entry.backend {
                ui.add_space(8.0);
                let backend_color = match backend.as_str() {
                    "goldberg" => theme::colors::ACCENT,
                    "photon" => theme::colors::SUCCESS,
                    _ => theme::colors::TEXT_MUTED,
                };
                ui.label(RichText::new(backend).small().color(backend_color));
            }
        });

        ui.add_space(8.0);

        // Description
        if !entry.description.is_empty() {
            ui.label(&entry.description);
            ui.add_space(8.0);
        }

        // Updated date
        if !entry.updated.is_empty() {
            ui.label(RichText::new(format!("Updated: {}", entry.updated)).small().color(theme::colors::TEXT_MUTED));
            ui.add_space(12.0);
        }

        // Install/Installed button
        let is_button_focused = self.registry_focus == RegistryFocus::InstallButton;
        ui.horizontal(|ui| {
            if is_installed {
                let mut btn = egui::Button::new("Installed").min_size(egui::vec2(100.0, 32.0));
                if is_button_focused {
                    btn = btn.stroke(theme::focus_stroke());
                }
                ui.add_enabled(false, btn);
                ui.add_space(8.0);
                ui.label(RichText::new("This handler is already installed").small().color(theme::colors::SUCCESS));
            } else if is_installing {
                ui.add_enabled(false, egui::Button::new("Installing...").min_size(egui::vec2(100.0, 32.0)));
            } else {
                let mut btn = egui::Button::new("Install").min_size(egui::vec2(100.0, 32.0));
                if is_button_focused {
                    btn = btn.stroke(theme::focus_stroke());
                }
                let install_btn = ui.add(btn);
                if install_btn.clicked() || (is_button_focused && self.activate_focused) {
                    self.install_registry_handler(entry.clone());
                }
            }
        });
    }

    /// Fetch the registry index from GitHub
    pub fn fetch_registry(&mut self) {
        self.registry_loading = true;
        self.registry_error = None;

        // We can't use spawn_task because we need to update registry_index
        // Instead, we'll do a blocking fetch in a thread and poll for completion
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let result = fetch_registry();
            let _ = tx.send(result);
        });

        // Store the receiver to check later - for now do blocking wait
        // TODO: Make this async in a future improvement
        match rx.recv() {
            Ok(Ok(index)) => {
                self.registry_index = Some(index);
                self.registry_loading = false;
            }
            Ok(Err(e)) => {
                self.registry_error = Some(e.to_string());
                self.registry_loading = false;
            }
            Err(_) => {
                self.registry_error = Some("Failed to receive registry data".to_string());
                self.registry_loading = false;
            }
        }
    }

    /// Install a handler from the registry
    fn install_registry_handler(&mut self, entry: RegistryEntry) {
        let entry_id = entry.id.clone();
        self.registry_installing = Some(entry_id.clone());

        self.spawn_task(&format!("Installing {}", entry.name), move || {
            if let Err(e) = download_handler(&entry) {
                eprintln!("[splitux] Failed to install handler: {}", e);
            }
        });

        // Refresh handlers list after installation
        // Note: This happens immediately but the spawn_task is async
        // The UI will update on next frame after task completes
        self.handlers = scan_handlers();
        self.registry_installing = None;
    }
}
