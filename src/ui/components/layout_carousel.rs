//! Layout carousel widget for selecting splitscreen layout presets
//!
//! Displays a miniature preview of the selected layout with left/right
//! navigation arrows. Supports both mouse and gamepad input.
//!
//! Also supports a "custom mode" where users can assign specific instances
//! to specific regions within the selected preset.

use eframe::egui::{self, Color32, RichText, Stroke, StrokeKind};
use egui_phosphor::regular as icons;

use crate::ui::theme;
use crate::wm::presets::{get_presets_for_count, LayoutPreset};

/// Player colors for the preview regions
pub const PLAYER_COLORS: [Color32; 4] = [
    Color32::from_rgb(80, 180, 255),  // P1: Cyan/blue (accent)
    Color32::from_rgb(80, 200, 120),  // P2: Green
    Color32::from_rgb(255, 180, 60),  // P3: Orange/yellow
    Color32::from_rgb(200, 100, 200), // P4: Purple
];

/// Response from rendering the layout carousel
pub struct LayoutCarouselResponse {
    /// User navigated to a different preset
    pub changed: bool,
    /// Direction of navigation: -1 for left, 1 for right, 0 for no change
    pub direction: i32,
    /// User right-clicked to enter custom mode
    pub enter_custom_mode: bool,
}

impl LayoutCarouselResponse {
    fn none() -> Self {
        Self {
            changed: false,
            direction: 0,
            enter_custom_mode: false,
        }
    }
}

/// Response from rendering the custom assignment UI
pub struct CustomAssignResponse {
    /// User exited custom mode (B button or click outside)
    pub exit_custom_mode: bool,
    /// New focused region (for navigation)
    pub new_focused_region: Option<usize>,
    /// Cycle instance in focused region (for A button)
    pub cycle_instance: bool,
}

/// Render a miniature preview of a layout preset
///
/// # Arguments
/// * `instance_order` - Optional custom instance order. If None, uses default (0, 1, 2, ...)
fn draw_layout_preview(
    ui: &mut egui::Ui,
    preset: &LayoutPreset,
    size: egui::Vec2,
    is_focused: bool,
    instance_order: Option<&[usize]>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);

        // Draw monitor outline/background
        let bg_color = theme::colors::BG_DARK;
        let stroke = if is_focused {
            theme::focus_stroke()
        } else {
            Stroke::new(1.5, theme::colors::TEXT_MUTED)
        };

        painter.rect_filled(rect, 4.0, bg_color);
        painter.rect_stroke(rect, 4.0, stroke, StrokeKind::Inside);

        // Draw player regions
        let inner_rect = rect.shrink(3.0); // Padding from border

        for (region_idx, region) in preset.regions.iter().enumerate() {
            let [x, y, w, h] = *region;

            // Get which instance is in this region (custom order or default)
            let instance_idx = instance_order
                .and_then(|order| order.get(region_idx).copied())
                .unwrap_or(region_idx);

            // Calculate region rect within the preview
            let region_rect = egui::Rect::from_min_size(
                inner_rect.min + egui::vec2(x * inner_rect.width(), y * inner_rect.height()),
                egui::vec2(w * inner_rect.width(), h * inner_rect.height()),
            )
            .shrink(1.5); // Gap between regions

            // Color based on which instance is assigned (not region index)
            let color = PLAYER_COLORS[instance_idx % PLAYER_COLORS.len()];

            // Fill with semi-transparent color
            painter.rect_filled(region_rect, 2.0, color.gamma_multiply(0.25));
            // Stroke with full color
            painter.rect_stroke(region_rect, 2.0, Stroke::new(1.5, color), StrokeKind::Inside);

            // Draw player number label (shows which instance, not region)
            let text = format!("{}", instance_idx + 1);
            painter.text(
                region_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(11.0),
                theme::colors::TEXT_PRIMARY,
            );
        }
    }

    response
}

/// Render the layout carousel widget
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `player_count` - Number of active players (determines available presets)
/// * `current_index` - Current preset index within the player count's presets
/// * `is_focused` - Whether this widget has gamepad focus
/// * `custom_order` - Optional custom instance order to display
///
/// # Returns
/// A `LayoutCarouselResponse` indicating if navigation occurred
pub fn render_layout_carousel(
    ui: &mut egui::Ui,
    player_count: usize,
    current_index: usize,
    is_focused: bool,
    custom_order: Option<&[usize]>,
) -> LayoutCarouselResponse {
    let presets = get_presets_for_count(player_count);

    if presets.is_empty() {
        ui.label("No layouts available");
        return LayoutCarouselResponse::none();
    }

    let current_index = current_index.min(presets.len() - 1);
    let current_preset = presets[current_index];
    let has_custom_order = custom_order.is_some();

    let mut response = LayoutCarouselResponse::none();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;

        // Left arrow button
        let left_btn = ui.add_enabled(
            presets.len() > 1,
            egui::Button::new(RichText::new(icons::CARET_LEFT).size(16.0))
                .min_size(egui::vec2(28.0, 28.0)),
        );
        if left_btn.clicked() {
            response.changed = true;
            response.direction = -1;
        }

        // Layout preview - shows custom order if set
        let preview_size = egui::vec2(80.0, 50.0);
        let preview_response =
            draw_layout_preview(ui, current_preset, preview_size, is_focused, custom_order);

        // Left-click cycles forward, right-click enters custom mode
        if preview_response.clicked() && presets.len() > 1 {
            response.changed = true;
            response.direction = 1;
        }
        if preview_response.secondary_clicked() {
            response.enter_custom_mode = true;
        }

        // Tooltip with controls
        preview_response.on_hover_text("Click: cycle preset | Right-click: customize positions");

        // Right arrow button
        let right_btn = ui.add_enabled(
            presets.len() > 1,
            egui::Button::new(RichText::new(icons::CARET_RIGHT).size(16.0))
                .min_size(egui::vec2(28.0, 28.0)),
        );
        if right_btn.clicked() {
            response.changed = true;
            response.direction = 1;
        }

        // Preset name and page indicator
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(current_preset.name)
                        .strong()
                        .color(if is_focused {
                            theme::colors::ACCENT
                        } else {
                            theme::colors::TEXT_PRIMARY
                        }),
                );
                // Show "customized" indicator if custom order exists
                if has_custom_order {
                    ui.label(
                        RichText::new("(customized)")
                            .small()
                            .color(theme::colors::ACCENT),
                    );
                }
            });
            ui.label(
                RichText::new(format!("{} / {}", current_index + 1, presets.len()))
                    .small()
                    .color(theme::colors::TEXT_MUTED),
            );
        });
    });

    response
}

/// Render the custom assignment mode UI
///
/// Shows a larger preview with selectable regions. Users can focus regions
/// and cycle which instance is assigned to each.
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `preset` - The current layout preset
/// * `instance_order` - Current instance order (index=region, value=instance)
/// * `focused_region` - Which region is currently focused
/// * `player_count` - Number of players/instances
///
/// # Returns
/// A `CustomAssignResponse` with interaction results
pub fn render_custom_assignment(
    ui: &mut egui::Ui,
    preset: &LayoutPreset,
    instance_order: &[usize],
    focused_region: usize,
    player_count: usize,
) -> CustomAssignResponse {
    let mut response = CustomAssignResponse {
        exit_custom_mode: false,
        new_focused_region: None,
        cycle_instance: false,
    };

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Customize Layout").strong());
            ui.add_space(8.0);
            ui.label(RichText::new(preset.name).color(theme::colors::TEXT_MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("Done")).clicked() {
                    response.exit_custom_mode = true;
                }
            });
        });

        ui.add_space(8.0);

        // Larger preview for custom mode
        let preview_size = egui::vec2(160.0, 100.0);
        let (rect, _) = ui.allocate_exact_size(preview_size, egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);

            // Draw monitor outline/background
            painter.rect_filled(rect, 4.0, theme::colors::BG_DARK);
            painter.rect_stroke(
                rect,
                4.0,
                Stroke::new(1.5, theme::colors::TEXT_MUTED),
                StrokeKind::Inside,
            );

            let inner_rect = rect.shrink(4.0);

            // Draw each region as a clickable area
            for (region_idx, region) in preset.regions.iter().enumerate() {
                let [x, y, w, h] = *region;

                let region_rect = egui::Rect::from_min_size(
                    inner_rect.min + egui::vec2(x * inner_rect.width(), y * inner_rect.height()),
                    egui::vec2(w * inner_rect.width(), h * inner_rect.height()),
                )
                .shrink(2.0);

                // Get the instance assigned to this region
                let instance_idx = instance_order.get(region_idx).copied().unwrap_or(region_idx);
                let color = PLAYER_COLORS[instance_idx % PLAYER_COLORS.len()];
                let is_focused = region_idx == focused_region;

                // Draw region
                painter.rect_filled(region_rect, 3.0, color.gamma_multiply(0.35));

                let stroke = if is_focused {
                    Stroke::new(3.0, theme::colors::ACCENT_GLOW)
                } else {
                    Stroke::new(1.5, color)
                };
                painter.rect_stroke(region_rect, 3.0, stroke, StrokeKind::Inside);

                // Draw instance label (P1, P2, etc.)
                let label = format!("P{}", instance_idx + 1);
                painter.text(
                    region_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(14.0),
                    if is_focused {
                        theme::colors::ACCENT_GLOW
                    } else {
                        theme::colors::TEXT_PRIMARY
                    },
                );

                // Check for clicks on this region
                let region_response = ui.interact(
                    region_rect,
                    egui::Id::new(("custom_region", region_idx)),
                    egui::Sense::click(),
                );
                if region_response.clicked() {
                    if region_idx == focused_region {
                        // Already focused, cycle the instance
                        response.cycle_instance = true;
                    } else {
                        // Focus this region
                        response.new_focused_region = Some(region_idx);
                    }
                }
            }
        }

        ui.add_space(8.0);
        ui.label(
            RichText::new("Click region to select, click again to cycle player")
                .small()
                .color(theme::colors::TEXT_MUTED),
        );

        // Show current mapping
        ui.horizontal(|ui| {
            for (region_idx, &instance_idx) in instance_order.iter().enumerate().take(player_count) {
                let color = PLAYER_COLORS[instance_idx % PLAYER_COLORS.len()];
                let text = format!("R{}: P{}", region_idx + 1, instance_idx + 1);
                ui.label(RichText::new(text).color(color).small());
                if region_idx < player_count - 1 {
                    ui.label(RichText::new("│").color(theme::colors::TEXT_MUTED).small());
                }
            }
        });
    });

    response
}

/// Calculate the next preset index after navigating in a direction
pub fn navigate_preset(current_index: usize, direction: i32, preset_count: usize) -> usize {
    if preset_count == 0 {
        return 0;
    }

    if direction > 0 {
        (current_index + 1) % preset_count
    } else if direction < 0 {
        if current_index == 0 {
            preset_count - 1
        } else {
            current_index - 1
        }
    } else {
        current_index
    }
}
