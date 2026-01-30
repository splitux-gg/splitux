//! gptokeyb type definitions

use serde::{Deserialize, Serialize};

/// gptokeyb configuration for controller→keyboard/mouse translation
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GptokeybSettings {
    /// Profile name - either built-in (fps, mouse_only, etc.) or "custom"
    /// Built-in profiles are loaded from assets/gptokeyb/{profile}.gptk
    /// Custom profiles are loaded from handler_dir/gptokeyb.gptk
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub profile: String,

    /// Mouse sensitivity multiplier (default: 512)
    /// Higher = faster cursor movement
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouse_scale: Option<u32>,

    /// Mouse update delay in ms (default: 16 = ~60fps)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouse_delay: Option<u32>,

    /// Deadzone for analog sticks (default: 2000)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadzone: Option<u32>,
}

impl GptokeybSettings {
    /// Check if gptokeyb is enabled (has a profile set)
    pub fn is_enabled(&self) -> bool {
        !self.profile.is_empty()
    }

    /// Check if settings are default/empty (for skip_serializing_if)
    pub fn is_default(&self) -> bool {
        self.profile.is_empty()
            && self.mouse_scale.is_none()
            && self.mouse_delay.is_none()
            && self.deadzone.is_none()
    }
}

/// Built-in profile names
#[allow(dead_code)]
pub const PROFILE_FPS: &str = "fps";
#[allow(dead_code)]
pub const PROFILE_MOUSE_ONLY: &str = "mouse_only";
#[allow(dead_code)]
pub const PROFILE_RACING: &str = "racing";
#[allow(dead_code)]
pub const PROFILE_CUSTOM: &str = "custom";

/// Get list of built-in profile names (for UI selection)
#[allow(dead_code)]
pub fn builtin_profiles() -> &'static [&'static str] {
    &[PROFILE_FPS, PROFILE_MOUSE_ONLY, PROFILE_RACING]
}
