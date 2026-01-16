pub mod components;
pub mod focus;
pub mod pages;
pub mod responsive;
pub mod theme;

// =============================================================================
// Legacy types (migrated from app/app.rs)
// =============================================================================

/// Application page/view
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum MenuPage {
    Games,     // Combined home + profiles view
    Registry,  // Browse and download handlers from online registry
    Settings,
    Instances, // Controller assignment screen (enters when "Play" pressed)
}

// =============================================================================
// Re-exports
// =============================================================================


// Legacy re-exports (for gradual migration)
pub use focus::{ActiveDropdown, FocusPane, InstanceFocus, ProfileBuilderFocus, RegistryFocus, SettingsCategory, SettingsFocus};
