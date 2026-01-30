//! Game info detail view - displays selected game information, action bar, and metadata

use crate::app::app::{FocusPane, Splitux};
use crate::app::theme;
use crate::paths::PATH_HOME;
use crate::ui::responsive::LayoutMode;
use crate::util::msg;
use eframe::egui::{self, RichText, Ui};
use rfd::FileDialog;

impl Splitux {
    pub(super) fn display_game_info(&mut self, ui: &mut Ui) {
        // Extract all handler data we need upfront to avoid borrow issues
        let icon = self.handlers[self.selected_handler].icon();
        let display = self.handlers[self.selected_handler].display();
        let is_win = self.handlers[self.selected_handler].win();
        let author = self.handlers[self.selected_handler].author.clone();
        let version = self.handlers[self.selected_handler].version.clone();
        let img_paths = self.handlers[self.selected_handler].img_paths.clone();
        let info = self.handlers[self.selected_handler].info.clone();
        let readme_path = self.handlers[self.selected_handler].path_handler.join("README.md");
        let readme_content = std::fs::read_to_string(&readme_path).ok();
        let hero_image = self.handlers[self.selected_handler].hero_image();
        let logo_image = self.handlers[self.selected_handler].logo_image();
        let box_art = self.handlers[self.selected_handler].box_art();
        let platform_name = self.handlers[self.selected_handler].platform_name();
        let platform_app_id = self.handlers[self.selected_handler].platform_app_id();
        let backend_display = self.handlers[self.selected_handler].backend_display();

        // Show Steam hero banner with logo overlay if available
        if let Some(hero_url) = hero_image {
            let banner_height = 160.0;
            let banner_width = ui.available_width();

            // Use Area to layer images
            let banner_rect = ui.available_rect_before_wrap();
            let banner_rect = egui::Rect::from_min_size(
                banner_rect.min,
                egui::vec2(banner_width, banner_height),
            );

            // Draw hero background
            ui.add(
                egui::Image::new(&hero_url)
                    .fit_to_exact_size(egui::vec2(banner_width, banner_height))
                    .maintain_aspect_ratio(false)
                    .corner_radius(4),
            );

            // Overlay logo or box art on top (centered)
            let overlay_image = logo_image.as_ref().or(box_art.as_ref());
            if let Some(overlay_url) = overlay_image {
                // Position the logo overlay in the center of where the banner was drawn
                let logo_rect = egui::Rect::from_center_size(
                    egui::pos2(
                        banner_rect.min.x + banner_width / 2.0,
                        banner_rect.min.y + banner_height / 2.0 - 8.0, // Offset up since banner already added
                    ),
                    egui::vec2(banner_width * 0.4, banner_height * 0.7),
                );

                ui.put(
                    logo_rect,
                    egui::Image::new(overlay_url.as_str())
                        .fit_to_exact_size(egui::vec2(banner_width * 0.4, banner_height * 0.7))
                        .maintain_aspect_ratio(true),
                );
            }

            ui.add_space(8.0);
        }

        ui.horizontal(|ui| {
            ui.add(egui::Image::new(icon).max_width(32.0).corner_radius(4));
            ui.add_space(8.0);
            ui.heading(display);
        });
        ui.add_space(8.0);
        ui.separator();

        ui.add_space(8.0);
        let mut play_clicked = false;
        let mut edit_clicked = false;

        // Pane-based focus for action bar
        let is_action_bar_focused = self.focus_pane == FocusPane::ActionBar;
        let activate = self.activate_focused && is_action_bar_focused;
        let focus_stroke = theme::focus_stroke();

        // Responsive layout
        let layout_mode = LayoutMode::from_ui(ui);
        let is_narrow = layout_mode.is_narrow();
        let is_medium = layout_mode == LayoutMode::Medium;

        ui.horizontal_wrapped(|ui| {
            // Play button (action_bar_index = 0)
            let play_focused = is_action_bar_focused && self.action_bar_index == 0;
            let (play_text, play_min_width) = if is_narrow {
                ("", 36.0)
            } else {
                (" Play ", 100.0)
            };
            let playbtn = ui.add(
                egui::Button::image_and_text(
                    egui::Image::new(egui::include_image!("../../../assets/BTN_A.png"))
                        .fit_to_exact_size(egui::vec2(20.0, 20.0)),
                    play_text,
                )
                .min_size(egui::vec2(play_min_width, 36.0))
                .corner_radius(8)
                .stroke(if play_focused {
                    focus_stroke
                } else {
                    egui::Stroke::NONE
                }),
            ).on_hover_text("Play");
            if playbtn.clicked() || (play_focused && activate) {
                play_clicked = true;
            }

            if !is_narrow {
                ui.add(egui::Separator::default().vertical());
            }

            // Profile display (Y button to change) (action_bar_index = 1)
            let current_profile_idx = self.get_current_profile();
            let profile_text = if self.profiles.is_empty() {
                "No profiles".to_string()
            } else if current_profile_idx < self.profiles.len() {
                let name = &self.profiles[current_profile_idx];
                if is_narrow && name.len() > 8 {
                    format!("{}...", &name[..6])
                } else {
                    name.clone()
                }
            } else {
                "Select...".to_string()
            };

            // Show profile button (highlight if focused or dropdown open)
            let profile_focused = is_action_bar_focused && self.action_bar_index == 1;
            let profile_min_width = if is_narrow { 60.0 } else { 100.0 };
            let profile_btn = ui.add(
                egui::Button::image_and_text(
                    egui::Image::new(egui::include_image!("../../../assets/BTN_Y.png"))
                        .fit_to_exact_size(egui::vec2(20.0, 20.0)),
                    format!(" {} ", profile_text),
                )
                .min_size(egui::vec2(profile_min_width, 36.0))
                .corner_radius(8)
                .stroke(if self.profile_dropdown_open || profile_focused {
                    focus_stroke
                } else {
                    egui::Stroke::NONE
                }),
            );
            if profile_btn.clicked() || (profile_focused && activate) {
                self.profile_dropdown_selection = current_profile_idx;
                self.profile_dropdown_open = !self.profile_dropdown_open;
            }
            profile_btn.on_hover_text("Press Y to change profile");

            if !is_narrow {
                ui.add(egui::Separator::default().vertical());
            }

            // Edit button (action_bar_index = 2)
            let edit_focused = is_action_bar_focused && self.action_bar_index == 2;
            let (edit_text, edit_min_width) = if is_narrow {
                ("", 36.0)
            } else {
                (" Edit ", 80.0)
            };
            let editbtn = ui.add(
                egui::Button::image_and_text(
                    egui::Image::new(egui::include_image!("../../../assets/BTN_X.png"))
                        .fit_to_exact_size(egui::vec2(20.0, 20.0)),
                    edit_text,
                )
                .min_size(egui::vec2(edit_min_width, 36.0))
                .corner_radius(8)
                .stroke(if edit_focused {
                    focus_stroke
                } else {
                    egui::Stroke::NONE
                }),
            ).on_hover_text("Edit handler");
            if editbtn.clicked() || (edit_focused && activate) {
                edit_clicked = true;
            }

            // Platform indicator and metadata (hide in narrow mode)
            if !is_narrow {
                ui.add(egui::Separator::default().vertical());
                if is_win {
                    ui.add(egui::Image::new(egui::include_image!("../../../assets/windows-logo.png"))
                        .fit_to_exact_size(egui::vec2(18.0, 18.0)));
                    if !is_medium {
                        ui.label("Proton");
                    }
                } else {
                    ui.add(egui::Image::new(egui::include_image!("../../../assets/linux-logo.png"))
                        .fit_to_exact_size(egui::vec2(18.0, 18.0)));
                    if !is_medium {
                        ui.label("Native");
                    }
                }
                // Platform source (Steam/Manual) and app ID
                if !is_medium {
                    ui.add(egui::Separator::default().vertical());
                    let platform_label = match platform_app_id.as_ref() {
                        Some(app_id) => format!("{} ({})", platform_name, app_id),
                        None => platform_name.clone(),
                    };
                    ui.label(platform_label);
                }
                // Backend type
                if !is_medium {
                    ui.add(egui::Separator::default().vertical());
                    ui.label(&backend_display);
                }
                // Author and version (only in wide mode)
                if !is_medium {
                    if !author.is_empty() {
                        ui.add(egui::Separator::default().vertical());
                        ui.label(format!("Author: {}", author));
                    }
                    if !version.is_empty() {
                        ui.add(egui::Separator::default().vertical());
                        ui.label(format!("Version: {}", version));
                    }
                }
            }
        });

        // Handle button clicks after UI closure
        if play_clicked {
            self.check_and_start_game();
        }
        if edit_clicked {
            self.handler_edit = Some(self.handlers[self.selected_handler].clone());
            self.show_edit_modal = true;
        }

        // Required mods section (only shown if handler has required_mods)
        let required_mods = self.handlers[self.selected_handler].required_mods.clone();
        let handler_path = self.handlers[self.selected_handler].path_handler.clone();

        // Track interactive elements for d-pad navigation
        let is_info_pane_focused = self.focus_pane == FocusPane::InfoPane;
        let activate = self.activate_focused && is_info_pane_focused;
        let mut info_element_idx = 0usize;
        let mut total_info_elements = 0usize;

        // Count total interactive elements first
        for rm in &required_mods {
            total_info_elements += 1; // Install button
            if !rm.url.is_empty() {
                total_info_elements += 1; // Download link
            }
        }

        // Clamp info_pane_index
        if total_info_elements > 0 && self.info_pane_index >= total_info_elements {
            self.info_pane_index = total_info_elements - 1;
        }

        // Wrap entire content area (from Required Mods onward) in a single scroll area
        // Use scroll offset from right stick
        let scroll_offset = self.info_pane_scroll.max(0.0);

        let scroll_area = egui::ScrollArea::vertical()
            .id_salt("game_info_scroll")
            .max_height(ui.available_height() - 8.0)
            .auto_shrink(false)
            .vertical_scroll_offset(scroll_offset);

        let scroll_output = scroll_area.show(ui, |ui| {
            // Required mods section (only shown if handler has required_mods)
            if !required_mods.is_empty() {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Required Mods").strong());

                    // Check if all mods are installed
                    let all_installed = required_mods.iter().all(|m| m.is_installed(&handler_path));
                    if all_installed {
                        ui.label(RichText::new(" (all installed)").weak().small());
                    } else {
                        ui.label(RichText::new(" (setup required)").weak().small().color(theme::colors::WARNING));
                    }
                });

                ui.add_space(4.0);

                for required_mod in &required_mods {
                    let is_installed = required_mod.is_installed(&handler_path);

                    ui.horizontal(|ui| {
                        // Status indicator
                        if is_installed {
                            ui.label(RichText::new("✓").color(theme::colors::SUCCESS));
                        } else {
                            ui.label(RichText::new("✗").color(theme::colors::ERROR));
                        }

                        // Mod name
                        ui.label(&required_mod.name);

                        // Install/Reinstall button with focus highlight
                        let button_text = if is_installed { "Reinstall..." } else { "Install..." };
                        let install_focused = is_info_pane_focused && self.info_pane_index == info_element_idx;
                        let install_btn = ui.add(
                            egui::Button::new(button_text)
                                .small()
                                .stroke(if install_focused { focus_stroke } else { egui::Stroke::NONE })
                        );
                        if install_btn.clicked() || (install_focused && activate) {
                            // Open file picker
                            let dest_path = required_mod.dest_full_path(&handler_path);
                            if let Some(file) = FileDialog::new()
                                .set_title(&format!("Select {} file", required_mod.name))
                                .set_directory(&*PATH_HOME)
                                .add_filter("DLL files", &["dll"])
                                .add_filter("All files", &["*"])
                                .pick_file()
                            {
                                // Create destination directory if needed
                                if let Err(e) = std::fs::create_dir_all(&dest_path) {
                                    msg("Error", &format!("Failed to create directory: {}", e));
                                } else {
                                    // Copy file to destination
                                    let dest_file = dest_path.join(file.file_name().unwrap_or_default());
                                    if let Err(e) = std::fs::copy(&file, &dest_file) {
                                        msg("Error", &format!("Failed to copy file: {}", e));
                                    } else {
                                        msg("Success", &format!("{} installed successfully!", required_mod.name));
                                    }
                                }
                            }
                        }
                        info_element_idx += 1;

                        // Download link if provided (with focus highlight)
                        if !required_mod.url.is_empty() {
                            let download_focused = is_info_pane_focused && self.info_pane_index == info_element_idx;
                            if download_focused {
                                egui::Frame::NONE
                                    .stroke(focus_stroke)
                                    .corner_radius(3)
                                    .inner_margin(egui::Margin::symmetric(2, 0))
                                    .show(ui, |ui| {
                                        ui.hyperlink_to("Download", &required_mod.url);
                                    });
                            } else {
                                ui.hyperlink_to("Download", &required_mod.url);
                            }
                            info_element_idx += 1;
                        }
                    });

                    // Description
                    if !required_mod.description.is_empty() {
                        ui.label(RichText::new(format!("  {}", required_mod.description)).weak().small());
                    }
                }
            }

            // Game images (responsive height)
            if !img_paths.is_empty() {
                ui.add_space(8.0);
                egui::ScrollArea::horizontal()
                    .max_width(f32::INFINITY)
                    .show(ui, |ui| {
                        let img_layout = LayoutMode::from_ui(ui);
                        let available_height = match img_layout {
                            LayoutMode::Wide => 200.0,
                            LayoutMode::Medium => 160.0,
                            LayoutMode::Narrow => 120.0,
                        };
                        ui.horizontal(|ui| {
                            for img in img_paths.iter() {
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

            // README or info text at bottom
            let has_readme = readme_content.is_some();
            let has_info = !info.is_empty();

            if has_readme || has_info {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Show README if available (takes priority)
                if let Some(readme) = &readme_content {
                    // Simple markdown-ish rendering
                    for line in readme.lines() {
                        if line.starts_with("# ") {
                            ui.add_space(4.0);
                            ui.label(RichText::new(&line[2..]).strong().size(18.0));
                            ui.add_space(2.0);
                        } else if line.starts_with("## ") {
                            ui.add_space(6.0);
                            ui.label(RichText::new(&line[3..]).strong().size(15.0));
                            ui.add_space(2.0);
                        } else if line.starts_with("### ") {
                            ui.add_space(4.0);
                            ui.label(RichText::new(&line[4..]).strong());
                            ui.add_space(1.0);
                        } else if line.starts_with("- ") {
                            ui.horizontal(|ui| {
                                ui.label("  •");
                                ui.label(&line[2..]);
                            });
                        } else if line.starts_with("```") {
                            // Skip code fence markers
                        } else if line.trim().is_empty() {
                            ui.add_space(4.0);
                        } else {
                            ui.label(line);
                        }
                    }
                } else if has_info {
                    // Fallback to info field
                    ui.label(&info);
                }
            }

            // Suppress unused variable warning
            let _ = info_element_idx;
        });

        // Sync scroll offset back from the scroll area state
        self.info_pane_scroll = scroll_output.state.offset.y;
    }
}
