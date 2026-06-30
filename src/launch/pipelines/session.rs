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
use crate::util::{cleanup_orphaned_processes, fuse_overlayfs_unmount_gamedirs};

use super::launch_game;
use super::super::setup_profiles;

/// Shared launch-core facade: the single entry every presentation layer
/// (CLI, TUI-via-CLI, GUI) funnels through.
///
/// Each surface only COLLECTS a launch — resolved `handlers`, game-tagged
/// `instances`, `monitors`, `profiles`, input `dev_infos`, `cfg`, and the
/// `master_profile` — and calls this. The common pre-launch transforms live HERE
/// so the surfaces cannot diverge:
///   1. per-game local-split (couch co-op) collapse,
///   2. per-monitor (or single-monitor) resolution sizing,
///   3. profile-name resolution,
/// then the shared [`run_session`]. Previously the CLI did all three inline while
/// the GUI did its own subset (no local-split collapse, single-game only) — this
/// facade is what makes "internal logic identical across CLI/TUI/GUI" true.
///
/// `use_multimonitor` selects per-monitor sizing (multi-game and any
/// `--display`-pinned launch) vs splitting one monitor.
#[allow(clippy::too_many_arguments)]
pub fn run_launch(
    handlers: &[Handler],
    instances: Vec<Instance>,
    monitors: Vec<Monitor>,
    profiles: &[String],
    dev_infos: &[DeviceInfo],
    cfg: &SplituxConfig,
    use_multimonitor: bool,
    master_profile: Option<&str>,
    ready: &AtomicBool,
    notify: &dyn Fn(&str, &str),
) {
    // 1. Per-game local-split collapse (fold a game's players into one instance
    //    owning N seats). Single-game = the legacy single collapse.
    let mut instances = crate::together::collapse_instances_per_game(instances, handlers);

    // 2. Resolution sizing — per-monitor or single-monitor.
    if use_multimonitor {
        crate::instance::set_instance_resolutions_multimonitor(&mut instances, &monitors, cfg);
    } else {
        crate::instance::set_instance_resolutions(&mut instances, &monitors[0], cfg);
    }

    // 3. Resolve profile display names (guests get random names).
    crate::instance::set_instance_names(&mut instances, profiles);

    // 4. Run the shared session (blocks until teardown).
    run_session(
        handlers,
        &instances,
        &monitors,
        dev_infos,
        cfg,
        master_profile,
        ready,
        notify,
        use_multimonitor,
    );
}

/// Run a full game session: profile + save setup, launch (which blocks until the
/// games exit and tears the WM/seats down), then save-back and cleanup. Sets
/// `ready` once the windows are up so a GUI can drop its overlay; the CLI can
/// ignore it. `notify` surfaces user-facing failures (title, body).
#[allow(clippy::too_many_arguments)]
pub fn run_session(
    handlers: &[Handler],
    instances: &Vec<Instance>,
    monitors: &[Monitor],
    dev_infos: &[DeviceInfo],
    cfg: &SplituxConfig,
    master_profile: Option<&str>,
    ready: &AtomicBool,
    notify: &dyn Fn(&str, &str),
    displays_assigned: bool,
) {
    // Single-game shim: until profile/save setup is grouped per game (step 4),
    // these session-level steps operate on the first (only) game's handler. A
    // length-1 slice keeps the legacy single-game behavior byte-for-byte.
    let handler = &handlers[0];

    // Clean up any orphaned processes from previous sessions.
    cleanup_orphaned_processes();

    sleep(Duration::from_secs_f32(1.5));

    if let Err(err) = setup_profiles(handler, instances) {
        println!("[splitux] Error setting up profiles: {}", err);
        notify("Failed setting up profiles", &format!("{err}"));
        return;
    }

    // Initialize profile saves with master-based inheritance.
    if handler.save_steam_cloud || !handler.original_save_path.is_empty() {
        if let Err(err) = save_sync::initialize_profile_saves(handler, instances, master_profile) {
            println!("[splitux] Warning: Failed to initialize saves: {}", err);
            // Non-fatal — continue.
        }
    }

    // Start shared backend sidecar services (e.g. Keen auth server) before the
    // games launch; they're killed at teardown below. No-op for backends that
    // don't need a sidecar (Goldberg/EOS/etc.).
    let mut backend_services = crate::backend::start_backend_services(handlers);

    // Note: fuse_overlayfs_mount_gamedirs runs inside launch_cmds with proper
    // Goldberg overlay support.
    if let Err(err) = launch_game(handlers, dev_infos, instances, monitors, cfg, ready, displays_assigned) {
        println!("[splitux] Error launching instances: {}", err);
        notify("Launch Error", &format!("{err}"));
    }

    // Games have exited (launch_game blocks until teardown): stop sidecars.
    for mut child in backend_services.drain(..) {
        let _ = child.kill();
        let _ = child.wait();
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
    // Launch-scoped: drop only THIS launch's scratch (tmp/<ns>), never the whole
    // tmp/ — a concurrent session from another splitux process keeps its own.
    if let Err(err) = fuse_overlayfs_unmount_gamedirs() {
        println!("[splitux] Error removing launch scratch: {}", err);
        notify("Failed removing launch scratch", &format!("{err}"));
    }

    // Clean teardown reached (incl. any save sync-back above): drop the runtime
    // marker so the TUI sees this session go inactive. A force-kill SIGKILLs the
    // main scope before here — the TUI removes the marker itself in that path.
    if let Ok(sid) = std::env::var(crate::session_store::SESSION_ID_ENV) {
        crate::session_store::remove_marker(&sid);
    }
}
