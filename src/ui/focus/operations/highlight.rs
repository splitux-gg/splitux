// Focus ring rendering operations (egui-dependent)

use eframe::egui::{self, Color32, Rect, Stroke, StrokeKind, Ui};

/// Focus ring styling configuration
pub struct FocusRingStyle {
    pub stroke_width: f32,
    pub color: Color32,
    pub corner_radius: f32,
    pub padding: f32,
}

impl Default for FocusRingStyle {
    fn default() -> Self {
        Self {
            stroke_width: 2.5,
            color: Color32::from_rgb(100, 200, 255), // ACCENT_GLOW
            corner_radius: 6.0,
            padding: 2.0,
        }
    }
}

/// Draw a focus ring around a rectangle
pub fn draw_focus_ring(ui: &mut Ui, rect: Rect) {
    draw_focus_ring_styled(ui, rect, &FocusRingStyle::default());
}

/// Draw a focus ring with custom styling
pub fn draw_focus_ring_styled(ui: &mut Ui, rect: Rect, style: &FocusRingStyle) {
    let expanded = rect.expand(style.padding);
    let corner_radius = style.corner_radius as u8;
    ui.painter().rect_stroke(
        expanded,
        corner_radius,
        Stroke::new(style.stroke_width, style.color),
        StrokeKind::Outside,
    );
}

/// Draw a focus ring around a response's rect if it's focused
pub fn draw_focus_ring_if_focused(ui: &mut Ui, rect: Rect, is_focused: bool) {
    if is_focused {
        draw_focus_ring(ui, rect);
    }
}

/// Draw a subtle pulsing focus ring (for animated focus indicators)
pub fn draw_pulsing_focus_ring(ui: &mut Ui, rect: Rect, time: f32) {
    let pulse = (time * 2.0).sin() * 0.3 + 0.7; // Oscillate between 0.4 and 1.0
    let alpha = (pulse * 255.0) as u8;

    let style = FocusRingStyle {
        color: Color32::from_rgba_unmultiplied(100, 200, 255, alpha),
        ..Default::default()
    };

    draw_focus_ring_styled(ui, rect, &style);
}

/// Get a stroke for focus highlighting (for use with egui widgets)
pub fn focus_stroke() -> Stroke {
    Stroke::new(2.5, Color32::from_rgb(100, 200, 255))
}

/// Get the selection stroke for the active/selected item
pub fn selection_stroke() -> Stroke {
    Stroke::new(2.0, Color32::from_rgb(80, 180, 255)) // ACCENT
}
