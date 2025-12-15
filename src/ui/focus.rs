pub mod operations;
pub mod pipelines;
pub mod pure;
pub mod types;

// Re-exports
pub use types::FocusState;

// Legacy re-exports (for gradual migration)
pub use types::{FocusPane, InstanceFocus, RegistryFocus, SettingsFocus};
