//! Instance setup page display functions
//!
//! This module is split into submodules:
//! - `audio` - Audio conflict detection and effective audio resolution
//! - `cards` - Main instance card rendering and the `display_page_instances()` entry point
//! - `focus` - Focus helper functions for instance card elements
//! - `help_bar` - Controls help bar UI
//! - `launch_options` - Bottom bar with start button and launch options
//! - `profile_changes` - Profile selection change handling and auto-assignment
//! - `types` - Dropdown action enum types
//! - `warnings` - Instance warning display

mod audio;
mod cards;
mod focus;
mod help_bar;
mod launch_options;
mod profile_changes;
mod types;
mod warnings;
