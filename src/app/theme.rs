// Custom console-style theme for Splitux
// Inspired by Steam Deck, PlayStation, and modern gaming UIs

use eframe::egui::{self, Color32, Stroke, Visuals};

// Color palette - deep blue/purple with cyan accents
pub mod colors {
    use super::Color32;

    // Base colors
    pub const BG_DARK: Color32 = Color32::from_rgb(15, 17, 26);        // Deep navy
    pub const BG_MID: Color32 = Color32::from_rgb(22, 25, 38);         // Panel background
    pub const BG_LIGHT: Color32 = Color32::from_rgb(32, 36, 52);       // Card/elevated surface
    pub const BG_HOVER: Color32 = Color32::from_rgb(42, 47, 68);       // Hover state

    // Text colors
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 235, 245);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(150, 160, 180);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(90, 100, 120);

    // Accent colors
    pub const ACCENT: Color32 = Color32::from_rgb(80, 180, 255);       // Bright cyan-blue
    pub const ACCENT_DIM: Color32 = Color32::from_rgb(50, 120, 180);   // Dimmer accent
    pub const ACCENT_GLOW: Color32 = Color32::from_rgb(100, 200, 255); // Focus glow

    // Semantic colors (available for future use in notifications, status indicators, etc.)
    #[allow(dead_code)]
    pub const SUCCESS: Color32 = Color32::from_rgb(80, 200, 120);
    pub const WARNING: Color32 = Color32::from_rgb(255, 180, 60);
    pub const ERROR: Color32 = Color32::from_rgb(255, 90, 90);

    // Interactive states
    pub const BUTTON_BG: Color32 = Color32::from_rgb(45, 50, 72);
    pub const BUTTON_HOVER: Color32 = Color32::from_rgb(55, 62, 88);
    pub const BUTTON_ACTIVE: Color32 = Color32::from_rgb(65, 75, 105);

    // Selection
    pub const SELECTION_BG: Color32 = Color32::from_rgb(40, 80, 130);
    pub const SELECTION_STROKE: Color32 = ACCENT;
}

/// Apply the custom Splitux theme to the egui context
pub fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Spacing and sizing
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(12);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.indent = 18.0;
    style.spacing.icon_width = 16.0;
    style.spacing.icon_spacing = 6.0;
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;

    // Build custom visuals
    let mut visuals = Visuals::dark();

    // Background colors
    visuals.panel_fill = colors::BG_MID;
    visuals.window_fill = colors::BG_MID;
    visuals.extreme_bg_color = colors::BG_DARK;
    visuals.faint_bg_color = colors::BG_LIGHT;
    visuals.code_bg_color = colors::BG_DARK;

    // Text colors
    visuals.override_text_color = Some(colors::TEXT_PRIMARY);
    visuals.warn_fg_color = colors::WARNING;
    visuals.error_fg_color = colors::ERROR;
    visuals.hyperlink_color = colors::ACCENT;

    // Selection
    visuals.selection.bg_fill = colors::SELECTION_BG;
    visuals.selection.stroke = Stroke::new(2.0, colors::SELECTION_STROKE);

    // Window styling
    visuals.window_corner_radius = 10.into();
    visuals.window_shadow = egui::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(100),
    };
    visuals.window_stroke = Stroke::new(1.0, colors::BG_LIGHT);

    visuals.menu_corner_radius = 6.into();
    visuals.popup_shadow = egui::Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(80),
    };

    // Widget styling - noninteractive (labels, etc.)
    visuals.widgets.noninteractive.bg_fill = colors::BG_LIGHT;
    visuals.widgets.noninteractive.weak_bg_fill = colors::BG_MID;
    visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
    visuals.widgets.noninteractive.corner_radius = 4.into();
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, colors::TEXT_SECONDARY);

    // Widget styling - inactive (buttons not hovered)
    visuals.widgets.inactive.bg_fill = colors::BUTTON_BG;
    visuals.widgets.inactive.weak_bg_fill = colors::BG_LIGHT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_white_alpha(10));
    visuals.widgets.inactive.corner_radius = 6.into();
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, colors::TEXT_PRIMARY);

    // Widget styling - hovered
    visuals.widgets.hovered.bg_fill = colors::BUTTON_HOVER;
    visuals.widgets.hovered.weak_bg_fill = colors::BG_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, colors::ACCENT_DIM);
    visuals.widgets.hovered.corner_radius = 6.into();
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, colors::TEXT_PRIMARY);

    // Widget styling - active (being clicked)
    visuals.widgets.active.bg_fill = colors::BUTTON_ACTIVE;
    visuals.widgets.active.weak_bg_fill = colors::BG_HOVER;
    visuals.widgets.active.bg_stroke = Stroke::new(2.0, colors::ACCENT);
    visuals.widgets.active.corner_radius = 6.into();
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, colors::TEXT_PRIMARY);

    // Widget styling - open (dropdown open, etc.)
    visuals.widgets.open.bg_fill = colors::BUTTON_ACTIVE;
    visuals.widgets.open.weak_bg_fill = colors::BG_HOVER;
    visuals.widgets.open.bg_stroke = Stroke::new(2.0, colors::ACCENT);
    visuals.widgets.open.corner_radius = 6.into();
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, colors::TEXT_PRIMARY);

    // Resize handle
    visuals.resize_corner_size = 10.0;

    // Apply visuals to style
    style.visuals = visuals;

    // Interaction - larger hit areas for controller use
    style.interaction.selectable_labels = true;
    style.interaction.tooltip_delay = 0.3;

    // Apply the style
    ctx.set_style(style);
}

/// Get the focus highlight stroke for gamepad navigation
pub fn focus_stroke() -> Stroke {
    Stroke::new(2.5, colors::ACCENT_GLOW)
}

/// Get a styled frame for cards/panels
pub fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(colors::BG_LIGHT)
        .corner_radius(8)
        .inner_margin(egui::Margin::same(12))
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(8)))
}

/// Get a styled frame for elevated surfaces (modals, dropdowns)
#[allow(dead_code)]
pub fn elevated_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(colors::BG_MID)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(16))
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: 20,
            spread: 0,
            color: Color32::from_black_alpha(120),
        })
        .stroke(Stroke::new(1.0, colors::BG_LIGHT))
}

/// Get a styled frame for the top navigation bar
#[allow(dead_code)]
pub fn nav_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(colors::BG_DARK)
        .inner_margin(egui::Margin::symmetric(12, 8))
}

/// Get a styled frame for side panels
#[allow(dead_code)]
pub fn panel_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(colors::BG_MID)
        .inner_margin(egui::Margin::same(8))
}

/// Styled separator with subtle color
#[allow(dead_code)]
pub fn separator_color() -> Color32 {
    Color32::from_white_alpha(20)
}
