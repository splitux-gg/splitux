pub mod colors;

// Re-export all colors and functions
pub use colors::{
    apply_theme, card_frame, elevated_frame, focus_stroke, nav_frame, panel_frame,
    separator_color, ACCENT, ACCENT_DIM, ACCENT_GLOW, BG_DARK, BG_HOVER, BG_LIGHT, BG_MID,
    BUTTON_ACTIVE, BUTTON_BG, BUTTON_HOVER, ERROR, SELECTION_BG, SELECTION_STROKE, SUCCESS,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, WARNING,
};
