//! Input handling for gamepad and keyboard navigation
//!
//! This module handles all input processing for the application:
//! - `gamepad` - Main entry point, button dispatch
//! - `keyboard` - Keyboard navigation (arrow keys, Enter, Escape)
//! - `navigation` - D-pad navigation for each page (legacy, being migrated)
//! - `nav_apply` - Bridge to new focus pipeline system
//! - `buttons` - Button action handlers (A, B, X, Y)
//! - `instances` - Instance page specific handlers
//! - `profile_builder` - Profile Builder (KB/Mouse Mapper) navigation

mod buttons;
mod gamepad;
mod instances;
mod keyboard;
mod nav_apply;
mod navigation;
mod profile_builder;
