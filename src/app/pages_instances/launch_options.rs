//! Launch options bar for instance page

use crate::app::app::{InstanceFocus, Splitux};
use crate::ui::theme;
use crate::ui::components::layout_carousel::{
    navigate_preset, render_custom_assignment, render_layout_carousel,
};
use crate::wm::presets::{get_preset_by_id, get_presets_for_count};
use eframe::egui::{self, RichText, Ui};

impl Splitux {
    /// True when EVERY game in the current session is local-split (couch co-op).
    /// Such games collapse to ONE fullscreen instance at launch, so the layout /
    /// split-style option is meaningless and is hidden. A `handler_lite` one-off
    /// launch checks that handler; otherwise the session's `selected_games`.
    pub(crate) fn is_local_coop_session(&self) -> bool {
        let is_local = |h: &crate::handler::Handler| h.is_local_split();
        if let Some(h) = &self.handler_lite {
            return is_local(h);
        }
        let games: Vec<usize> = if self.selected_games.is_empty() {
            vec![self.selected_handler]
        } else {
            self.selected_games.clone()
        };
        !games.is_empty()
            && games
                .iter()
                .all(|&i| self.handlers.get(i).map(is_local).unwrap_or(false))
    }

    /// Whether to show the split-screen layout carousel.
    ///
    /// The carousel only makes sense when 2+ players share ONE local screen that
    /// gets tiled between them. So it shows only when there are 2+ LOCAL seats
    /// (Together/remote seats stream their own fullscreen window to a browser and
    /// never participate in the local split), and the game isn't a local-split
    /// couch title (those collapse to a single fullscreen instance).
    pub(crate) fn show_layout_carousel(&self) -> bool {
        let local_seats = self.instances.iter().filter(|i| !i.together).count();
        local_seats >= 2 && !self.is_local_coop_session()
    }

    /// Display the bottom bar with launch options and start button
    pub(super) fn display_launch_options(&mut self, ui: &mut Ui) {
        if self.instances.is_empty() {
            return;
        }

        let player_count = self.instances.len();
        // Carousel only for 2+ local (non-Together) seats sharing one screen.
        let show_layout = self.show_layout_carousel();
        let preset_id = self
            .options
            .layout_presets
            .get_for_count(player_count)
            .to_string();

        // Handle custom mode rendering (never for local-coop — no layout to edit)
        if self.layout_custom_mode && show_layout {
            self.display_custom_layout_mode(ui, player_count, &preset_id);
            return;
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(12.0);
            let start_btn = ui.add(
                egui::Button::image_and_text(
                    egui::Image::new(egui::include_image!("../../../assets/BTN_START_NEW.png"))
                        .fit_to_exact_size(egui::vec2(24.0, 24.0)),
                    "  Start Game  ",
                )
                .min_size(egui::vec2(180.0, 48.0))
                .corner_radius(10)
                .fill(theme::colors::ACCENT_DIM),
            );
            if start_btn.clicked() {
                self.prepare_game_launch();
            }
            ui.add_space(8.0);

            // Launch options
            let is_launch_options_focused = self.instance_focus == InstanceFocus::LaunchOptions;
            let frame_stroke = if is_launch_options_focused {
                egui::Stroke::new(2.0, theme::colors::ACCENT)
            } else {
                egui::Stroke::NONE
            };

            // The split-screen layout carousel is the only launch option (the
            // KB/mouse-support toggle was removed — input holding is derived from
            // whether a keyboard/mouse seat exists). Show the bar only when the
            // carousel is relevant: 2+ local seats sharing one screen.
            if show_layout {
                theme::card_frame()
                    .fill(theme::colors::BG_DARK)
                    .stroke(frame_stroke)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Layout").strong());
                            ui.add_space(16.0);

                            let layout_focused = is_launch_options_focused;

                            let current_index =
                                self.options.layout_presets.get_index_for_count(player_count);

                            // Get custom order if it exists (for display in carousel)
                            let custom_order = if self.options.layout_presets.has_custom_order(&preset_id) {
                                Some(self.options.layout_presets.get_order(&preset_id, player_count))
                            } else {
                                None
                            };

                            let response = render_layout_carousel(
                                ui,
                                player_count,
                                current_index,
                                layout_focused,
                                custom_order.as_deref(),
                            );

                            // Handle mouse-based navigation from carousel
                            if response.changed {
                                let presets = get_presets_for_count(player_count);
                                let new_index =
                                    navigate_preset(current_index, response.direction, presets.len());
                                if let Some(preset) = presets.get(new_index) {
                                    self.options
                                        .layout_presets
                                        .set_for_count(player_count, preset.id.to_string());
                                }
                            }

                            // Fullscreen gives every player a full-screen surface,
                            // so per-region instance assignment is meaningless.
                            let is_fullscreen = crate::wm::types::get_layout_type(&preset_id)
                                == crate::wm::types::LayoutType::Fullscreen;

                            // Handle enter custom mode (not applicable to fullscreen)
                            if response.enter_custom_mode && !is_fullscreen {
                                self.enter_custom_layout_mode(player_count, &preset_id);
                            }

                            if layout_focused {
                                self.infotext = if is_fullscreen {
                                    "Left/Right: cycle presets | Each player gets a full-screen window".to_string()
                                } else {
                                    "Left/Right: cycle presets | Y/Right-click: customize positions".to_string()
                                };
                            }
                        });
                    });
                ui.add_space(8.0);
                ui.separator();
            }
        });
    }

    /// Enter custom layout mode
    pub(crate) fn enter_custom_layout_mode(&mut self, player_count: usize, preset_id: &str) {
        // Fullscreen presets give every player a full-screen surface; there are
        // no regions to assign instances to, so custom mode does not apply.
        if crate::wm::types::get_layout_type(preset_id)
            == crate::wm::types::LayoutType::Fullscreen
        {
            return;
        }
        self.layout_custom_mode = true;
        self.layout_focused_region = 0;
        // Initialize edit order from saved custom order or default
        // If preset_id is empty, just use default sequential order
        self.layout_edit_order = if preset_id.is_empty() {
            (0..player_count).collect()
        } else {
            self.options.layout_presets.get_order(preset_id, player_count)
        };
    }

    /// Display the custom layout assignment mode
    fn display_custom_layout_mode(&mut self, ui: &mut Ui, player_count: usize, preset_id: &str) {
        // Get preset by ID, or fall back to first preset for this player count
        let presets = get_presets_for_count(player_count);
        let preset = match get_preset_by_id(preset_id) {
            Some(p) => p,
            None => {
                // Preset ID not found (possibly empty), use first available preset
                match presets.first() {
                    Some(p) => *p,
                    None => {
                        self.layout_custom_mode = false;
                        return;
                    }
                }
            }
        };

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(12.0);

            theme::card_frame()
                .fill(theme::colors::BG_DARK)
                .stroke(egui::Stroke::new(2.0, theme::colors::ACCENT))
                .show(ui, |ui| {
                    let response = render_custom_assignment(
                        ui,
                        preset,
                        &self.layout_edit_order,
                        self.layout_focused_region,
                        player_count,
                    );

                    // Handle responses
                    if response.exit_custom_mode {
                        // Save the custom order
                        self.options
                            .layout_presets
                            .set_order(preset_id, self.layout_edit_order.clone());
                        self.layout_custom_mode = false;
                    }

                    if let Some(new_region) = response.new_focused_region {
                        self.layout_focused_region = new_region;
                    }

                    if response.cycle_instance {
                        self.cycle_instance_in_region(player_count);
                    }
                });

            ui.add_space(8.0);
            ui.separator();
        });

        self.infotext =
            "D-pad: navigate regions | A: cycle player | B: done".to_string();
    }

    /// Cycle the instance assigned to the currently focused region
    pub(crate) fn cycle_instance_in_region(&mut self, player_count: usize) {
        if self.layout_focused_region >= self.layout_edit_order.len() {
            return;
        }

        let current_instance = self.layout_edit_order[self.layout_focused_region];
        let next_instance = (current_instance + 1) % player_count;
        self.layout_edit_order[self.layout_focused_region] = next_instance;
    }

    /// Exit custom layout mode and save the order
    pub(crate) fn exit_custom_layout_mode(&mut self) {
        let player_count = self.instances.len();
        let preset_id = self
            .options
            .layout_presets
            .get_for_count(player_count)
            .to_string();
        self.options
            .layout_presets
            .set_order(&preset_id, self.layout_edit_order.clone());
        self.layout_custom_mode = false;
    }

    /// Get the current preset, falling back to first available if ID is empty/invalid
    fn get_current_preset(&self) -> Option<&'static crate::wm::presets::LayoutPreset> {
        let player_count = self.instances.len();
        let preset_id = self.options.layout_presets.get_for_count(player_count);

        get_preset_by_id(preset_id)
            .or_else(|| get_presets_for_count(player_count).first().copied())
    }

    /// Navigate up in custom layout mode (find region above current)
    pub(crate) fn navigate_custom_layout_up(&mut self) {
        if let Some(preset) = self.get_current_preset() {
            if let Some(new_region) = find_region_in_direction(preset, self.layout_focused_region, Direction::Up) {
                self.layout_focused_region = new_region;
            }
        }
    }

    /// Navigate down in custom layout mode
    pub(crate) fn navigate_custom_layout_down(&mut self) {
        if let Some(preset) = self.get_current_preset() {
            if let Some(new_region) = find_region_in_direction(preset, self.layout_focused_region, Direction::Down) {
                self.layout_focused_region = new_region;
            }
        }
    }

    /// Navigate left in custom layout mode
    pub(crate) fn navigate_custom_layout_left(&mut self) {
        if let Some(preset) = self.get_current_preset() {
            if let Some(new_region) = find_region_in_direction(preset, self.layout_focused_region, Direction::Left) {
                self.layout_focused_region = new_region;
            }
        }
    }

    /// Navigate right in custom layout mode
    pub(crate) fn navigate_custom_layout_right(&mut self) {
        if let Some(preset) = self.get_current_preset() {
            if let Some(new_region) = find_region_in_direction(preset, self.layout_focused_region, Direction::Right) {
                self.layout_focused_region = new_region;
            }
        }
    }
}

/// Direction for spatial navigation
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Find the region in a given direction from the current region
/// Uses center points of regions to determine spatial relationships
fn find_region_in_direction(
    preset: &crate::wm::presets::LayoutPreset,
    current_idx: usize,
    direction: Direction,
) -> Option<usize> {
    let regions = preset.regions;
    if current_idx >= regions.len() {
        return None;
    }

    let [cx, cy, cw, ch] = regions[current_idx];
    let current_center_x = cx + cw / 2.0;
    let current_center_y = cy + ch / 2.0;

    let mut best_idx: Option<usize> = None;
    let mut best_distance = f32::MAX;

    for (idx, &[rx, ry, rw, rh]) in regions.iter().enumerate() {
        if idx == current_idx {
            continue;
        }

        let region_center_x = rx + rw / 2.0;
        let region_center_y = ry + rh / 2.0;

        // Check if region is in the right direction
        let is_valid = match direction {
            Direction::Up => region_center_y < current_center_y,
            Direction::Down => region_center_y > current_center_y,
            Direction::Left => region_center_x < current_center_x,
            Direction::Right => region_center_x > current_center_x,
        };

        if !is_valid {
            continue;
        }

        // Calculate distance (prefer regions more aligned with the direction)
        let distance = match direction {
            Direction::Up | Direction::Down => {
                let dy = (region_center_y - current_center_y).abs();
                let dx = (region_center_x - current_center_x).abs();
                dy + dx * 0.5 // Prefer vertical alignment
            }
            Direction::Left | Direction::Right => {
                let dx = (region_center_x - current_center_x).abs();
                let dy = (region_center_y - current_center_y).abs();
                dx + dy * 0.5 // Prefer horizontal alignment
            }
        };

        if distance < best_distance {
            best_distance = distance;
            best_idx = Some(idx);
        }
    }

    best_idx
}
