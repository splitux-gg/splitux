//! Game execution pipeline

use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// Ctrl+C / SIGTERM on the `splitux launch` terminal must tear the WHOLE session
// down (kill games + gamescope, unmount overlays, restore the bar) instead of
// orphaning it — the CLI launch otherwise has no signal handler, so Ctrl+C killed
// splitux and left everything running (and waybar dead). The handler only flips a
// flag (async-signal-safe); the supervise loop polls it and falls through to the
// normal teardown path below. A second Ctrl+C force-exits in case teardown wedges.
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static SIGINT_COUNT: AtomicUsize = AtomicUsize::new(0);

extern "C" fn on_interrupt(_sig: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
    if SIGINT_COUNT.fetch_add(1, Ordering::SeqCst) >= 1 {
        // second signal while tearing down — bail hard rather than hang
        unsafe { libc::_exit(130) };
    }
}

fn install_interrupt_handler() {
    let h = on_interrupt as extern "C" fn(libc::c_int);
    unsafe {
        libc::signal(libc::SIGINT, h as libc::sighandler_t);
        libc::signal(libc::SIGTERM, h as libc::sighandler_t);
    }
}

use crate::app::{SplituxConfig, WindowManagerType};
use crate::audio::{
    resolve_audio_system, setup_audio_session, teardown_audio_session, AudioContext, AudioSystem,
    VirtualSink,
};
use crate::bwrap;
use crate::gptokeyb;
use crate::handler::Handler;
use crate::input::DeviceInfo;
use crate::instance::Instance;
use crate::monitor::Monitor;
use crate::wm::presets::{get_preset_by_id, get_presets_for_count};
use crate::wm::{LayoutContext, WindowManager, WindowManagerBackend};

use super::build_cmds::launch_cmds;
use super::super::operations::scope;
use super::super::pure::command::{format_launch_cmd, rebuild_command_with_blocking};

/// RAII safety net for the launch's host-stability-critical resources. The normal
/// teardown only runs if the supervise loop is reached; any EARLY error (`?` from
/// launch_cmds / wm.setup / netns / spawn / window-positioning) would otherwise
/// return and ORPHAN them — leaving seat-streamers (and their raw-gadget xpad360
/// helpers) alive holding dummy_hcd UDCs, fuse-overlayfs game mounts mounted, and
/// the launch slice up — which bloats the env and degrades/wedges the next launch.
/// On any early return this Drop stops the launch slice (cascades SIGTERM to the
/// scoped seat-streamers → they release their UDCs gracefully), unmounts the
/// overlays, and tears down netns. Disarmed right before the normal teardown so it
/// never double-runs on the success path. (stop_slice/unmount are idempotent.)
struct LaunchGuard {
    scoping: bool,
    launch_id: String,
    bridged: bool,
    n_instances: usize,
    armed: bool,
}

impl Drop for LaunchGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        println!(
            "[splitux] launch aborted before supervise — safety teardown (slice/overlays/netns)"
        );
        if self.scoping {
            scope::stop_slice(&self.launch_id);
            scope::clear_active_slice();
        }
        if self.bridged {
            crate::netns::teardown(self.n_instances);
        }
        if let Err(e) = crate::util::fuse_overlayfs_unmount_gamedirs() {
            println!("[splitux] Warning: fuse-overlayfs unmount failed during safety teardown: {e}");
        }
    }
}

/// Launch the game with all instances
#[allow(clippy::too_many_arguments)]
pub fn launch_game(
    handlers: &[Handler],
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    monitors: &[Monitor],
    cfg: &SplituxConfig,
    ready: &std::sync::atomic::AtomicBool,
) -> Result<(), Box<dyn std::error::Error>> {
    // All handler reads are now per-unit: session-wide steps that genuinely need
    // a representative handler index `handlers[…]` explicitly; the spawn loop and
    // batch ops use `handlers[instance.game]`. No launch-wide `h` shim remains.
    // Establish the per-launch namespace FIRST. Everything per-launch keys off it
    // — scratch dirs (tmp/<ns>), scope units, AND the audio capture sink names —
    // so concurrent splitux processes never collide. Must precede audio routing,
    // which embeds this ns in each sink name.
    let scoping = scope::enabled();
    let launch_id = scope::new_launch_id();
    // Namespace ALL per-launch scratch (overlay mounts, goldberg overlays, work
    // dirs) under tmp/<launch_id> so concurrent splitux processes don't collide
    // on tmp/game-0 etc. Must be set before any scratch dir is created below.
    crate::paths::set_launch_ns(&launch_id);

    // Set up audio routing (per-session capture sinks for together instances).
    let (audio_system, virtual_sinks, audio_sink_envs) = setup_audio_routing(instances, cfg);

    // Set up gptokeyb daemons if enabled (spawns before command building so we can pass virtual device paths)
    let (mut gptokeyb_handles, gptokeyb_virtual_devices) =
        setup_gptokeyb_daemons(handlers, input_devices, instances);

    // Process containment must be established BEFORE spawning together
    // seat-streamers, so each streamer launches inside the per-launch slice and
    // therefore shares the launch lifecycle (instead of orphaning as a bare
    // child on a hard kill and keeping its virtual input devices alive). Killing
    // splitux by any signal cascades teardown to the whole launch slice — game
    // cgroup AND seat-streamers.
    // Reap scratch (tmp/<ns>) left by DEAD splitux processes — skips LIVE
    // concurrent sessions (their pid is still alive), so this is safe to run
    // while other splitux processes have games up.
    crate::util::reap_orphan_launch_scratch();
    let main_scope = scope::current_main_scope();
    if scoping {
        // Reap leftover units from any previous crashed/killed run first.
        scope::sweep_orphan_units();
        scope::set_active_slice(&launch_id);
        if main_scope.is_none() {
            println!(
                "[splitux] scope - Warning: not running inside a splitux-main scope; \
                 launch slice won't auto-die with splitux (self-scope re-exec may have failed)"
            );
        }
        println!(
            "[splitux] scope - Launch slice {} (main scope: {:?})",
            scope::slice_name(&launch_id),
            main_scope
        );
        // If the TUI launched us with a session id, record a runtime marker so it
        // can show this session ● active and target End/Kill at our exact units.
        // ALWAYS write it when a session id is present — the launch SLICE is the
        // reliable live-handle the TUI keys off, and it exists regardless of the
        // main scope. (A detached TUI-spawned launch often has NO splitux-main
        // scope — main_scope is then empty — which used to skip the marker
        // entirely, leaving the TUI's per-session End/Kill thinking nothing was
        // running so only Kill-All worked.)
        if let Ok(sid) = std::env::var(crate::session_store::SESSION_ID_ENV) {
            let ms = main_scope.as_deref().unwrap_or("");
            crate::session_store::write_marker(&sid, &scope::slice_name(&launch_id), ms);
        }
    }

    // Arm the early-abort safety net NOW — before seat-streamers spawn — so any
    // `?` between here and the supervise loop still tears the slice/overlays/netns
    // down instead of orphaning seats that hold dummy_hcd UDCs. Disarmed right
    // before the normal teardown below.
    let mut launch_guard = LaunchGuard {
        scoping,
        launch_id: launch_id.clone(),
        bridged: false,
        n_instances: instances.len(),
        armed: true,
    };

    // Set up splitux-together remote seats (no-op unless a player is marked
    // remote). Spawns one seat-streamer per remote player BEFORE command
    // building so its virtual devices exist for gamescope's --libinput-hold-dev,
    // exactly like gptokeyb above. Each streamer is scoped into the launch slice
    // so it lives and dies with the launch. Returns the per-instance virtual
    // device paths to wire into the launch command + the invite URLs to pop up.
    let game_labels: Vec<String> = handlers.iter().map(|gh| gh.name.clone()).collect();
    let (mut together_handles, together_devices, together_invites) =
        crate::together::setup_together_seats(
            instances,
            cfg,
            &game_labels,
            &launch_id,
            main_scope.as_deref(),
            scoping,
            &audio_sink_envs,
        );

    let new_cmds = launch_cmds(
        handlers,
        input_devices,
        instances,
        monitors,
        cfg,
        &audio_sink_envs,
        &gptokeyb_virtual_devices,
        &together_devices,
    )?;

    // Create WM backend based on config
    let mut wm = match &cfg.window_manager {
        WindowManagerType::Auto => WindowManagerBackend::detect(),
        WindowManagerType::KWin => WindowManagerBackend::KWin(crate::wm::KWinManager::new()),
        WindowManagerType::Hyprland => {
            WindowManagerBackend::Hyprland(crate::wm::HyprlandManager::new())
        }
        WindowManagerType::GamescopeOnly => {
            WindowManagerBackend::GamescopeOnly(crate::wm::GamescopeOnlyManager::new())
        }
    };

    // Setup WM with layout context
    let player_count = instances.len();
    let preset_id = cfg.layout_presets.get_for_count(player_count);

    println!("[splitux] Layout: preset_id from config = '{}'", preset_id);

    let preset_found = get_preset_by_id(preset_id);
    let preset = preset_found
        .or_else(|| {
            println!("[splitux] Layout: preset '{}' not found, using fallback", preset_id);
            get_presets_for_count(player_count).first().copied()
        })
        .expect("No layout preset available");

    println!("[splitux] Layout: using preset '{}' ({})", preset.id, preset.name);

    // Get custom instance order (or default sequential)
    // instance_order[region] = instance_idx (which instance goes in which region)
    let instance_order = cfg.layout_presets.get_order(preset_id, player_count);
    println!("[splitux] Layout: instance_order = {:?}", instance_order);

    // Compute inverse mapping: instance_to_region[instance] = region
    // This tells us which region each spawned window should go to
    let mut instance_to_region = vec![0; player_count];
    for (region, &instance_idx) in instance_order.iter().enumerate() {
        if instance_idx < player_count {
            instance_to_region[instance_idx] = region;
        }
    }
    println!("[splitux] Layout: instance_to_region = {:?}", instance_to_region);

    // Gamescope-bypass: mirror build_cmds' per-instance decision EXACTLY so the WM
    // agrees with the commands that were actually built. build_cmds gates on the
    // instance's resolved seat list being empty (`seats.is_empty()`), NOT on
    // `inst.together` — a together instance whose seat-streamer failed to spawn
    // has an empty seat list, so build_cmds bypasses gamescope; if we keyed off
    // `inst.together` here instead, the WM would wait for a gamescope window that
    // was never created and the LaunchGuard would kill the running game.
    //
    // Computed per instance (same monitor-sharing rule as build_cmds' bypass_gamescope):
    // a mixed launch — e.g. one local instance alone on its own monitor plus a
    // together instance on another — bypasses gamescope for the local instance
    // only. `no_gamescope` (used by the WM to relax window matching and to pick
    // the best-effort — vs hard-fail — wait/position path) is true whenever ANY
    // instance bypasses: the WM's per-window ownership resolution (see
    // `wm::niri::resolve_window_instance`) then sorts out which window belongs to
    // which instance, so relaxing matching for the whole launch is safe. This does
    // mean a mixed launch never takes the strict hard-fail path even for its
    // gamescope-backed instances — an intentional trade toward "never kill a
    // running game over a WM positioning miss" (see the soft-path comment below).
    let mon_instance_counts: Vec<usize> = {
        let max_monitor = instances.iter().map(|inst| inst.monitor).max().unwrap_or(0);
        let mut counts = vec![0usize; max_monitor + 1];
        for inst in instances.iter() {
            counts[inst.monitor] += 1;
        }
        counts
    };
    let no_gamescope = instances.iter().enumerate().any(|(i, inst)| {
        let h = &handlers[inst.game];
        h.effective_disable_gamescope(cfg)
            && mon_instance_counts[inst.monitor] == 1
            && together_devices.get(i).map(|s| s.is_empty()).unwrap_or(true)
            && !h.disable_bwrap
    });

    let ctx = LayoutContext {
        instances: instances.to_vec(),
        monitors: monitors.to_vec(),
        preset,
        instance_to_region,
        no_gamescope,
    };

    // (Process containment / launch slice was established above, before the
    // together seat-streamers were spawned, so `scoping`, `launch_id` and
    // `main_scope` are already in scope here for wrapping each game instance.)
    // Each instance launches into its own scope under the launch slice, bound to
    // splitux's main scope, so killing splitux cascades teardown to the whole
    // game cgroup — wine, the EOS overlay, fuse daemons and all.

    println!("[splitux] Setting up {} window manager", wm.name());
    wm.setup(&ctx)?;

    // Delay after each spawn for Vulkan/GPU initialization
    let vulkan_init_delay = 6.0;

    // Delay before each spawn for input/SDL initialization
    let input_init_delay = cfg.input_init_delay.unwrap_or(1.0);

    // goldberg.bridged_lan: put each instance in its own network namespace +
    // veth into a shared Linux bridge so co-located instances are distinct LAN
    // hosts (own IP, own loopback, own game port). Each built command is wrapped
    // to launch inside its namespace (see the spawn loop below); the bridge and
    // namespaces are created here, before any instance spawns, and torn down
    // after the session ends (both the clean-exit and the Ctrl+C paths fall
    // through to the teardown below).
    //
    // This is PER UNIT (per game): only instances whose game opts into
    // `bridged_lan` get a namespace, so a mixed launch can run one bridged game
    // alongside an un-bridged (or EOS-localhost) one.
    let bridged_per_inst: Vec<bool> = instances
        .iter()
        .map(|inst| {
            handlers[inst.game]
                .goldberg_ref()
                .map(|g| g.bridged_lan)
                .unwrap_or(false)
        })
        .collect();
    let any_bridged = bridged_per_inst.iter().any(|&b| b);
    if any_bridged {
        // Warn ONCE per bridged game that also runs EOS (localhost-incompatible).
        let mut warned: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for inst in instances.iter() {
            if !warned.insert(inst.game) {
                continue;
            }
            let gh = &handlers[inst.game];
            if gh.goldberg_ref().map(|g| g.bridged_lan).unwrap_or(false) && gh.has_eos() {
                println!(
                    "[splitux] netns - WARNING: goldberg.bridged_lan is set together with an EOS \
                     backend (game '{}'). The EOS emu's localhost mode expects a shared 127.0.0.1 \
                     and is incompatible with split network namespaces — discovery/join may break.",
                    gh.display()
                );
            }
        }
        // Hard-fail (don't silently launch un-isolated) if the host can't do it.
        crate::netns::preflight()?;
        crate::netns::setup_bridge()?;
        for (i, &b) in bridged_per_inst.iter().enumerate() {
            if b {
                crate::netns::add_instance(i)?;
            }
        }
    }
    // netns is now (fully) set up; let the guard tear it down on an early abort.
    launch_guard.bridged = any_bridged;

    let mut handles = Vec::new();

    for (i, (cmd, bwrap_arg_count)) in new_cmds.into_iter().enumerate() {
        // This instance's unit handler — isolation / bridged / stdout are all
        // per-game (single-game: handlers[0]).
        let hh = &handlers[instances[i].game];

        // For native Linux games with Facepunch/BepInEx, redirect stdout to
        // prevent a CStreamWriter crash: BepInEx's LinuxConsoleDriver checks
        // isatty(1) and crashes if stdout is a TTY; redirecting to null makes
        // isatty(1) return false.
        let redirect_stdout = !hh.win() && hh.has_facepunch();
        // Input initialization delay before spawn (except first instance)
        if i > 0 && input_init_delay > 0.0 {
            println!(
                "[splitux] Input init delay: {}ms",
                (input_init_delay * 1000.0) as u32
            );
            std::thread::sleep(std::time::Duration::from_secs_f64(input_init_delay));
        }

        // Build fresh device isolation args right before spawn (spawn-time permission check).
        // These must be inserted as bwrap args, before the child command (proton/game).
        use crate::handler::InputIsolation;
        let blocking_args = if hh.disable_bwrap {
            Vec::new()
        } else {
            match hh.effective_input_isolation() {
                InputIsolation::None => Vec::new(),
                // Legacy SDL-only path: /dev/null-bind unassigned devices. Breaks
                // raw-evdev engines (Godot) — kept only for explicit opt-in.
                InputIsolation::Sdl => {
                    let initial_js_devices = bwrap::glob_js_devices();
                    let mut args = bwrap::get_js_blocking_args(&initial_js_devices, i);
                    args.extend(bwrap::get_evdev_hidraw_blocking_args(
                        input_devices,
                        &instances[i].devices,
                        i,
                    ));
                    args
                }
                // Universal allowlist: expose ONLY this instance's assigned input
                // devices (local pads + remote seat pads); everything else becomes
                // ENOENT, which SDL2 and raw-evdev engines (Godot) both skip cleanly.
                InputIsolation::Evdev => {
                    let mut allowed =
                        bwrap::get_assigned_gamepad_paths(input_devices, &instances[i].devices);
                    if let Some(seats) = together_devices.get(i) {
                        for seat in seats {
                            if let Some(pad) = &seat.pad {
                                allowed.push(pad.to_string_lossy().to_string());
                            }
                        }
                    }
                    allowed.retain(|p| std::path::Path::new(p).exists());
                    println!(
                        "[splitux] Instance {}: evdev allowlist — exposing {} device(s): {:?}",
                        i,
                        allowed.len(),
                        allowed
                    );
                    let refs: Vec<&str> = allowed.iter().map(|s| s.as_str()).collect();
                    bwrap::build_allowlist_args(&refs)
                }
            }
        };

        // Reconstruct command with blocking args inserted at the bwrap/child boundary
        let mut cmd = rebuild_command_with_blocking(cmd, bwrap_arg_count, &blocking_args);

        // goldberg.bridged_lan: wrap into instance i's network namespace. This
        // MUST happen AFTER rebuild_command_with_blocking — that insertion uses
        // bwrap_arg_count, an index into the ORIGINAL (gamescope-rooted) command,
        // and the wrap re-roots the command at `sudo` (prefixing tokens). Doing
        // the wrap last means the device-block index is computed and applied
        // before any prefix exists, so the offset stays correct and device
        // isolation is unaffected. (Same reasoning the scope wrap below relies
        // on — both are outer wrappers applied post-blocking.)
        if bridged_per_inst[i] {
            cmd = crate::netns::wrap_command_in_netns(cmd, i);
        }

        // Print the final command (with blocking args)
        print!("{}", format_launch_cmd(&cmd, i));
        println!();

        if redirect_stdout {
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());
        }

        // Wrap in a systemd scope so the whole subtree is contained and dies
        // with splitux. systemd-run waits on the game, so wait() below is intact.
        if scoping {
            cmd = scope::wrap_command(cmd, &launch_id, instances[i].game, i, main_scope.as_deref());
        }

        let handle = cmd.spawn()?;
        handles.push(handle);

        // Vulkan/GPU initialization delay after spawn (except last instance)
        if i < instances.len() - 1 {
            println!(
                "[splitux] Vulkan init delay: {}ms",
                (vulkan_init_delay * 1000.0) as u32
            );
            std::thread::sleep(std::time::Duration::from_secs_f64(vulkan_init_delay));
        }
    }

    // Notify WM that all instances have been launched (for positioning)
    if !wm.is_reactive() {
        println!("[splitux] Non-reactive WM, positioning windows explicitly");
        wm.on_instances_launched(&ctx)?;
    }

    // Windows are up and positioned: tell the UI it can drop the "Launching…"
    // overlay and let the launcher be used while we supervise the session below.
    ready.store(true, std::sync::atomic::Ordering::Release);

    // Remote seats are live now — pop the invite URLs so the host can hand each
    // friend their single-URL link.
    crate::together::popup_invites(&together_invites);

    // Supervise the session: wait for all games to exit OR a Ctrl+C/SIGTERM.
    // Either way we fall through to the teardown below (stop slice, unmount
    // overlays, restore the bar) so the CLI launch never orphans the session.
    install_interrupt_handler();
    loop {
        if INTERRUPTED.load(Ordering::SeqCst) {
            println!("[splitux] interrupt received — tearing down session…");
            for handle in handles.iter_mut() {
                let _ = handle.kill();
            }
            for handle in handles.iter_mut() {
                let _ = handle.wait();
            }
            break;
        }
        let mut all_exited = true;
        for handle in handles.iter_mut() {
            match handle.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) => all_exited = false,
                Err(_) => {}
            }
        }
        if all_exited {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // Reached the supervise loop and fell through to the normal teardown — disarm
    // the early-abort guard so it doesn't double-run the slice/overlay/netns
    // teardown we're about to do in full (incl. wm/gptokeyb/seats/audio).
    launch_guard.armed = false;

    // Stop the launch slice (kills any lingering instance scope + its cgroup),
    // then drop the fuse-overlayfs mounts. Idempotent — the games normally
    // exited already, but this reaps stragglers and clears the active marker.
    if scoping {
        scope::stop_slice(&launch_id);
        scope::clear_active_slice();
    }

    // goldberg.bridged_lan teardown. Reached on BOTH the clean-exit and the
    // interrupt paths: the SIGINT/SIGTERM handler only flips INTERRUPTED, the
    // supervise loop breaks, and execution falls through here. (A SECOND Ctrl+C
    // hard-exits via libc::_exit and skips this — but teardown is idempotent and
    // add_instance() re-cleans stale namespaces on the next bridged launch.)
    if any_bridged {
        crate::netns::teardown(instances.len());
    }

    if let Err(e) = crate::util::fuse_overlayfs_unmount_gamedirs() {
        println!("[splitux] Warning: fuse-overlayfs unmount failed: {e}");
    }

    // Teardown WM
    println!("[splitux] Tearing down {} window manager", wm.name());
    wm.teardown()?;

    // Teardown gptokeyb daemons
    gptokeyb::terminate_all(&mut gptokeyb_handles);

    // Teardown together seat-streamers (and any local orchestrator we started)
    crate::together::terminate_all(&mut together_handles);

    // Teardown audio routing
    if !virtual_sinks.is_empty()
        && let Err(e) = teardown_audio_session(audio_system, &virtual_sinks) {
            println!("[splitux] Warning: Audio teardown failed: {}", e);
        }

    Ok(())
}

// rebuild_command_with_blocking moved to launch/pure/command.rs

/// Set up audio routing for all instances
///
/// Returns (audio_system, virtual_sinks, sink_env_vars_per_instance)
fn setup_audio_routing(
    instances: &[Instance],
    cfg: &SplituxConfig,
) -> (AudioSystem, Vec<VirtualSink>, Vec<String>) {
    // Per-session audio isolation: every `together` instance needs its OWN sink so
    // its seat-streamer can capture only that instance's audio. Without this the
    // streamers all tap the shared default-sink monitor and every game's sound
    // bleeds into every stream. So we run audio setup whenever EITHER explicit
    // routing is enabled OR any remote seat is present, even if cfg.audio.enabled
    // is off (that flag governs only the user-facing per-device routing feature).
    let any_together = instances.iter().any(|inst| inst.together);
    if !cfg.audio.enabled && !any_together {
        return (AudioSystem::None, vec![], vec![String::new(); instances.len()]);
    }

    let audio_system = resolve_audio_system(cfg.audio.system);
    if audio_system == AudioSystem::None {
        println!("[splitux] audio - No audio system available, skipping audio routing");
        return (AudioSystem::None, vec![], vec![String::new(); instances.len()]);
    }

    // Build per-instance sink assignments. An explicit config assignment (only
    // honored when the routing feature is enabled) wins; otherwise a together
    // instance gets a dedicated CAPTURE sink (its monitor is fed to that seat's
    // stream); local instances keep the default sink (None).
    let assignments: Vec<Option<String>> = instances
        .iter()
        .enumerate()
        .map(|(i, inst)| {
            if cfg.audio.enabled
                && let Some(target) = cfg.audio.default_assignments.get(&i) {
                    return Some(target.clone());
                }
            if inst.together {
                return Some(crate::audio::AUDIO_CAPTURE_SENTINEL.to_string());
            }
            None
        })
        .collect();

    let ctx = AudioContext {
        system: audio_system,
        assignments,
        // Tie sink names to this launch's namespace (already set in launch_game
        // before this runs) so concurrent splitux processes never collide.
        ns: crate::paths::launch_ns(),
    };

    match setup_audio_session(&ctx) {
        Ok((virtual_sinks, sink_envs)) => {
            println!(
                "[splitux] audio - Audio routing set up: {} virtual sinks",
                virtual_sinks.len()
            );
            (audio_system, virtual_sinks, sink_envs)
        }
        Err(e) => {
            println!("[splitux] audio - Warning: Audio setup failed: {}", e);
            (audio_system, vec![], vec![String::new(); instances.len()])
        }
    }
}

/// Set up gptokeyb daemons for all instances
///
/// Returns (child_handles, virtual_device_paths).
/// - child_handles: Some for instances with gptokeyb, None otherwise
/// - virtual_device_paths: path to gptokeyb's virtual keyboard/mouse device for each instance
fn setup_gptokeyb_daemons(
    handlers: &[Handler],
    input_devices: &[DeviceInfo],
    instances: &[Instance],
) -> (Vec<Option<Child>>, Vec<Option<std::path::PathBuf>>) {
    let num_instances = instances.len();
    let mut handles: Vec<Option<Child>> = (0..num_instances).map(|_| None).collect();
    let mut devices: Vec<Option<std::path::PathBuf>> = (0..num_instances).map(|_| None).collect();

    // Any game want gptokeyb at all?
    let any_gptokeyb = instances
        .iter()
        .any(|inst| handlers[inst.game].has_gptokeyb());
    if !any_gptokeyb {
        return (handles, devices);
    }

    if !gptokeyb::is_available() {
        println!("[splitux] gptokeyb - Binary not found, skipping controller→keyboard translation");
        return (handles, devices);
    }

    // Per game (unit): each game has its own gptokeyb profile + handler dir, and
    // only its own instances get a daemon. Group by game, spawn with the unit's
    // settings, scatter back to GLOBAL instance slots (which also name the
    // daemons' virtual devices, keeping them unique across games).
    let mut games_seen: Vec<usize> = Vec::new();
    let mut idxs_by_game: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for (gi, inst) in instances.iter().enumerate() {
        idxs_by_game
            .entry(inst.game)
            .or_insert_with(|| {
                games_seen.push(inst.game);
                Vec::new()
            })
            .push(gi);
    }

    for game in games_seen {
        let h = &handlers[game];
        if !h.has_gptokeyb() {
            continue;
        }
        println!(
            "[splitux] gptokeyb - Game {}: controller→keyboard translation (profile: {})",
            game, h.gptokeyb.profile
        );
        let global_idxs = &idxs_by_game[&game];
        let instance_device_indices: Vec<Vec<usize>> = global_idxs
            .iter()
            .map(|&gi| instances[gi].devices.clone())
            .collect();

        let (g_handles, g_devices) = gptokeyb::spawn_all_daemons(
            &h.gptokeyb,
            &h.path_handler,
            input_devices,
            &instance_device_indices,
            global_idxs,
        );

        for (local_k, (handle, dev)) in g_handles.into_iter().zip(g_devices).enumerate() {
            let gi = global_idxs[local_k];
            handles[gi] = handle;
            devices[gi] = dev;
        }
    }

    (handles, devices)
}
