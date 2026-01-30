//! Type definitions for instance page dropdown actions

/// Audio override dropdown action
#[derive(Clone, PartialEq)]
pub(super) enum AudioOverrideAction {
    SetDevice(String),
    Mute,
    Reset,
}

/// Audio preference dropdown action
#[derive(Clone, PartialEq)]
pub(super) enum AudioPrefAction {
    SetDevice(String, String), // (name, description)
    Clear,
}

/// gptokeyb profile dropdown action
#[derive(Clone, PartialEq)]
pub(super) enum GptokeybAction {
    /// Use handler default (no override)
    Default,
    /// Disable gptokeyb for this instance
    Disabled,
    /// Use a specific profile
    Profile(String),
}
