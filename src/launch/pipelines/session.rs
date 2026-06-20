//! Shared session-launch core.
//!
//! This is the body that used to live inline in the GUI's `prepare_game_launch`
//! launch thread. Extracted so the headless CLI (`splitux launch`) and the GUI
//! run the *exact same* sequence: clean up, set up profiles + saves, launch the
//! game, then sync saves back and tidy up. User-facing messages go through a
//! `notify(title, body)` callback — the GUI wires it to a modal dialog, the CLI
//! to stderr.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use crate::app::SplituxConfig;
use crate::input::DeviceInfo;
use crate::instance::Instance;
use crate::handler::Handler;
use crate::monitor::Monitor;
use crate::profiles::remove_guest_profiles;
use crate::save_sync;
use crate::util::{cleanup_orphaned_processes, clear_tmp};

use super::launch_game;
use super::super::setup_profiles;

/// Run a full game session: profile + save setup, launch (which blocks until the
/// games exit and tears the WM/seats down), then save-back and cleanup. Sets
/// `ready` once the windows are up so a GUI can drop its overlay; the CLI can
/// ignore it. `notify` surfaces user-facing failures (title, body).
pub fn run_session(
    handler: &Handler,
    instances: &Vec<Instance>,
    monitors: &[Monitor],
    dev_infos: &[DeviceInfo],
    cfg: &SplituxConfig,
    master_profile: Option<&str>,
    ready: &AtomicBool,
    notify: &dyn Fn(&str, &str),
) {
    // Clean up any orphaned processes from previous sessions.
    cleanup_orphaned_processes();

    sleep(Duration::from_secs_f32(1.5));

    if let Err(err) = setup_profiles(handler, instances) {
        println!("[splitux] Error setting up profiles: {}", err);
        notify("Failed setting up profiles", &format!("{err}"));
        return;
    }

    // Initialize profile saves with master-based inheritance.
    if !handler.original_save_path.is_empty() {
        if let Err(err) = save_sync::initialize_profile_saves(handler, instances, master_profile) {
            println!("[splitux] Warning: Failed to initialize saves: {}", err);
            // Non-fatal — continue.
        }
    }

    // Note: fuse_overlayfs_mount_gamedirs runs inside launch_cmds with proper
    // Goldberg overlay support.
    if let Err(err) = launch_game(handler, dev_infos, instances, monitors, cfg, ready) {
        println!("[splitux] Error launching instances: {}", err);
        notify("Launch Error", &format!("{err}"));
    }
    // Ensure any waiter is released even if launch errored before signaling.
    ready.store(true, Ordering::Release);

    // Sync master profile's saves back to the original location.
    if handler.save_sync_back {
        if let Err(err) = save_sync::sync_master_saves_back(handler, instances, master_profile) {
            println!("[splitux] Error syncing saves back: {}", err);
            notify("Save Sync Error", &format!("Failed to sync saves back: {err}"));
        }
    }

    if let Err(err) = remove_guest_profiles() {
        println!("[splitux] Error removing guest profiles: {}", err);
        notify("Failed removing guest profiles", &format!("{err}"));
    }
    if let Err(err) = clear_tmp() {
        println!("[splitux] Error removing tmp directory: {}", err);
        notify("Failed removing tmp directory", &format!("{err}"));
    }

    // Clean teardown reached (incl. any save sync-back above): drop the runtime
    // marker so the TUI sees this session go inactive. A force-kill SIGKILLs the
    // main scope before here — the TUI removes the marker itself in that path.
    if let Ok(sid) = std::env::var(crate::session_store::SESSION_ID_ENV) {
        crate::session_store::remove_marker(&sid);
    }
}
