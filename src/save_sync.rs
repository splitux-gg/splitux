// Save game synchronization module
// Handles copying original saves to profiles and syncing back after sessions
//
// User provides: original_save_path (full path to saves)
// We auto-detect:
//   - If inside game directory -> copy to gamesaves/{handler}/{relative}
//   - If under HOME -> copy to home/{relative}
//   - If Windows AppData style -> copy to windata/{path}
//
// Steam ID Remapping:
//   Some games (like DRG) tie save files to Steam IDs by embedding the ID in filenames.
//   When using Goldberg, each profile gets a unique Steam ID. We detect save files with
//   Steam ID prefixes and remap them to match the profile's Goldberg Steam ID.

pub mod operations;
pub mod pipelines;
pub mod pure;

// Re-export public API from pure

// Re-export public API from pipelines
#[allow(deprecated)]
pub use pipelines::{
    initialize_profile_saves,
    sync_master_saves_back,
};
