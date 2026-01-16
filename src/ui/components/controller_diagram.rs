//! Interactive controller diagram widget
//!
//! Displays the controller SVG with clickable button regions for mapping.

use crate::gptokeyb::ControllerButton;
use crate::ui::theme;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Ui, Vec2};

/// Buttons available in the diagram, in navigation order
pub const DIAGRAM_BUTTONS: &[ControllerButton] = &[
    // Top row: triggers and bumpers
    ControllerButton::L2,
    ControllerButton::L1,
    ControllerButton::R1,
    ControllerButton::R2,
    // Left side: D-pad
    ControllerButton::Up,
    ControllerButton::Left,
    ControllerButton::Right,
    ControllerButton::Down,
    // Center: special buttons
    ControllerButton::Back,
    ControllerButton::Guide,
    ControllerButton::Start,
    // Right side: face buttons
    ControllerButton::Y,
    ControllerButton::X,
    ControllerButton::B,
    ControllerButton::A,
    // Sticks
    ControllerButton::L3,
    ControllerButton::R3,
];

struct ButtonRegion {
    button: ControllerButton,
    center: (f32, f32),
    radius: f32,
}

fn button_regions() -> Vec<ButtonRegion> {
    vec![
        // Triggers (top)
        ButtonRegion { button: ControllerButton::L2, center: (0.145, 0.06), radius: 0.055 },
        ButtonRegion { button: ControllerButton::R2, center: (0.855, 0.06), radius: 0.055 },
        // Bumpers
        ButtonRegion { button: ControllerButton::L1, center: (0.145, 0.16), radius: 0.045 },
        ButtonRegion { button: ControllerButton::R1, center: (0.855, 0.16), radius: 0.045 },
        // Left stick
        ButtonRegion { button: ControllerButton::L3, center: (0.255, 0.52), radius: 0.085 },
        // Right stick
        ButtonRegion { button: ControllerButton::R3, center: (0.615, 0.72), radius: 0.085 },
        // D-pad
        ButtonRegion { button: ControllerButton::Up, center: (0.255, 0.30), radius: 0.032 },
        ButtonRegion { button: ControllerButton::Down, center: (0.255, 0.40), radius: 0.032 },
        ButtonRegion { button: ControllerButton::Left, center: (0.205, 0.35), radius: 0.032 },
        ButtonRegion { button: ControllerButton::Right, center: (0.305, 0.35), radius: 0.032 },
        // Face buttons
        ButtonRegion { button: ControllerButton::Y, center: (0.745, 0.30), radius: 0.038 },
        ButtonRegion { button: ControllerButton::B, center: (0.795, 0.38), radius: 0.038 },
        ButtonRegion { button: ControllerButton::A, center: (0.745, 0.46), radius: 0.038 },
        ButtonRegion { button: ControllerButton::X, center: (0.695, 0.38), radius: 0.038 },
        // Center buttons
        ButtonRegion { button: ControllerButton::Back, center: (0.385, 0.35), radius: 0.028 },
        ButtonRegion { button: ControllerButton::Guide, center: (0.50, 0.42), radius: 0.045 },
        ButtonRegion { button: ControllerButton::Start, center: (0.615, 0.35), radius: 0.028 },
    ]
}

pub struct ControllerDiagramResponse {
    pub clicked: Option<ControllerButton>,
    pub hovered: Option<ControllerButton>,
}

pub fn render_controller_diagram<F>(
    ui: &mut Ui,
    selected_button: Option<ControllerButton>,
    gamepad_focused: Option<ControllerButton>,
    mappings: F,
) -> ControllerDiagramResponse
where
    F: Fn(ControllerButton) -> Option<String>,
{
    let mut response = ControllerDiagramResponse {
        clicked: None,
        hovered: None,
    };

    let available = ui.available_size();
    let aspect = 673.42 / 417.68;
    let width = available.x.min(500.0);
    let height = width / aspect;
    let size = Vec2::new(width, height);

    let (rect, sense_response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Draw SVG background
        egui::Image::new(egui::include_image!("../../../res/controllermap.svg"))
            .fit_to_exact_size(size)
            .paint_at(ui, rect);

        // Button overlays
        for region in button_regions() {
            let center = Pos2::new(
                rect.left() + region.center.0 * rect.width(),
                rect.top() + region.center.1 * rect.height(),
            );
            let radius = region.radius * rect.width();
            let button_rect = Rect::from_center_size(center, Vec2::splat(radius * 2.0));

            let is_hovered = sense_response.hovered()
                && ui.input(|i| i.pointer.hover_pos())
                    .map(|pos| button_rect.contains(pos))
                    .unwrap_or(false);

            if is_hovered && sense_response.clicked() {
                response.clicked = Some(region.button);
            }
            if is_hovered {
                response.hovered = Some(region.button);
            }

            let is_selected = selected_button == Some(region.button);
            let is_gamepad_focused = gamepad_focused == Some(region.button);
            let has_mapping = mappings(region.button).is_some();

            let fill = if is_selected {
                Color32::from_rgba_unmultiplied(80, 180, 255, 160)
            } else if is_gamepad_focused {
                Color32::from_rgba_unmultiplied(255, 180, 60, 140) // Orange for gamepad focus
            } else if is_hovered {
                Color32::from_rgba_unmultiplied(255, 255, 255, 80)
            } else if has_mapping {
                Color32::from_rgba_unmultiplied(80, 200, 120, 140)
            } else {
                Color32::TRANSPARENT
            };

            let stroke = if is_selected || is_gamepad_focused {
                egui::Stroke::new(3.0, Color32::WHITE)
            } else if is_hovered || has_mapping {
                egui::Stroke::new(2.0, Color32::WHITE)
            } else {
                egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 100))
            };

            painter.circle(center, radius, fill, stroke);

            // Label
            let label = button_label(region.button);
            if !label.is_empty() {
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(11.0),
                    Color32::WHITE,
                );
            }

            // Mapping text
            if let Some(mapping) = mappings(region.button) {
                let text = if mapping.len() > 6 { &mapping[..6] } else { &mapping };
                painter.text(
                    Pos2::new(center.x, center.y + radius + 8.0),
                    egui::Align2::CENTER_TOP,
                    text,
                    egui::FontId::proportional(9.0),
                    theme::colors::ACCENT,
                );
            }
        }
    }

    response
}

fn button_label(button: ControllerButton) -> &'static str {
    match button {
        ControllerButton::A => "A",
        ControllerButton::B => "B",
        ControllerButton::X => "X",
        ControllerButton::Y => "Y",
        ControllerButton::Up => "▲",
        ControllerButton::Down => "▼",
        ControllerButton::Left => "◀",
        ControllerButton::Right => "▶",
        ControllerButton::L1 => "LB",
        ControllerButton::R1 => "RB",
        ControllerButton::L2 => "LT",
        ControllerButton::R2 => "RT",
        ControllerButton::L3 => "LS",
        ControllerButton::R3 => "RS",
        ControllerButton::Start => "☰",
        ControllerButton::Back => "⋮⋮",
        ControllerButton::Guide => "◉",
        _ => "",
    }
}

pub fn render_button_legend(ui: &mut Ui, show_gamepad_hint: bool) {
    ui.horizontal(|ui| {
        if show_gamepad_hint {
            ui.label("D-pad to navigate, A to select.");
        } else {
            ui.label("Click button to map.");
        }
        ui.add_space(8.0);
        ui.label(egui::RichText::new("○").color(Color32::from_rgb(80, 180, 255)));
        ui.label("selected");
        ui.add_space(4.0);
        ui.label(egui::RichText::new("○").color(Color32::from_rgb(255, 180, 60)));
        ui.label("focused");
        ui.add_space(4.0);
        ui.label(egui::RichText::new("○").color(Color32::from_rgb(80, 200, 120)));
        ui.label("mapped");
    });
}
