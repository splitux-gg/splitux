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

/// Launch the game with all instances
pub fn launch_game(
    h: &Handler,
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    monitors: &[Monitor],
    cfg: &SplituxConfig,
    ready: &std::sync::atomic::AtomicBool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Set up audio routing if enabled
    let (audio_system, virtual_sinks, audio_sink_envs) = setup_audio_routing(instances, cfg);

    // Set up gptokeyb daemons if enabled (spawns before command building so we can pass virtual device paths)
    let (mut gptokeyb_handles, gptokeyb_virtual_devices) =
        setup_gptokeyb_daemons(h, input_devices, instances);

    // Set up splitux-together remote seats (no-op unless a player is marked
    // remote). Spawns one seat-streamer per remote player BEFORE command
    // building so its virtual devices exist for gamescope's --libinput-hold-dev,
    // exactly like gptokeyb above. Returns the per-instance virtual device paths
    // to wire into the launch command + the invite URLs to pop up.
    let (mut together_handles, together_devices, together_invites) =
        crate::together::setup_together_seats(instances, cfg, &h.name);

    let new_cmds = launch_cmds(
        h,
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

    let ctx = LayoutContext {
        instances: instances.to_vec(),
        monitors: monitors.to_vec(),
        preset,
        instance_to_region,
    };

    // Process containment: each instance launches into its own systemd scope
    // under a per-launch slice, bound to splitux's main scope. Killing splitux
    // (by any signal) cascades teardown to the whole game cgroup — wine, the EOS
    // overlay, fuse daemons and all — instead of letting them reparent and leak.
    let scoping = scope::enabled();
    let launch_id = scope::new_launch_id();
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
    }

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
    let bridged = h.goldberg_ref().map(|g| g.bridged_lan).unwrap_or(false);
    if bridged {
        if h.has_eos() {
            println!(
                "[splitux] netns - WARNING: goldberg.bridged_lan is set together with an EOS \
                 backend. The EOS emu's localhost mode expects a shared 127.0.0.1 and is \
                 incompatible with split network namespaces — discovery/join may break."
            );
        }
        // Hard-fail (don't silently launch un-isolated) if the host can't do it.
        crate::netns::preflight()?;
        crate::netns::setup_bridge()?;
        for i in 0..instances.len() {
            crate::netns::add_instance(i)?;
        }
    }

    let mut handles = Vec::new();

    // For native Linux games with Facepunch/BepInEx, redirect stdout to prevent
    // CStreamWriter crash. BepInEx's LinuxConsoleDriver checks isatty(1) and crashes
    // if stdout is a TTY. Redirecting to null makes isatty(1) return false.
    let redirect_stdout = !h.win() && h.has_facepunch();

    for (i, (cmd, bwrap_arg_count)) in new_cmds.into_iter().enumerate() {
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
        let blocking_args = if h.disable_bwrap {
            Vec::new()
        } else {
            match h.effective_input_isolation() {
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
        if bridged {
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
            cmd = scope::wrap_command(cmd, &launch_id, i, main_scope.as_deref());
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
    if bridged {
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
    if !virtual_sinks.is_empty() {
        if let Err(e) = teardown_audio_session(audio_system, &virtual_sinks) {
            println!("[splitux] Warning: Audio teardown failed: {}", e);
        }
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
    if !cfg.audio.enabled {
        // Audio routing disabled, return empty vectors
        return (AudioSystem::None, vec![], vec![String::new(); instances.len()]);
    }

    let audio_system = resolve_audio_system(cfg.audio.system);
    if audio_system == AudioSystem::None {
        println!("[splitux] audio - No audio system available, skipping audio routing");
        return (AudioSystem::None, vec![], vec![String::new(); instances.len()]);
    }

    // Build assignments from config
    let assignments: Vec<Option<String>> = (0..instances.len())
        .map(|i| cfg.audio.default_assignments.get(&i).cloned())
        .collect();

    let ctx = AudioContext {
        system: audio_system,
        assignments,
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
    h: &Handler,
    input_devices: &[DeviceInfo],
    instances: &[Instance],
) -> (Vec<Option<Child>>, Vec<Option<std::path::PathBuf>>) {
    let num_instances = instances.len();

    if !h.has_gptokeyb() {
        return (
            (0..num_instances).map(|_| None).collect(),
            (0..num_instances).map(|_| None).collect(),
        );
    }

    if !gptokeyb::is_available() {
        println!("[splitux] gptokeyb - Binary not found, skipping controller→keyboard translation");
        return (
            (0..num_instances).map(|_| None).collect(),
            (0..num_instances).map(|_| None).collect(),
        );
    }

    println!(
        "[splitux] gptokeyb - Setting up controller→keyboard translation (profile: {})",
        h.gptokeyb.profile
    );

    // Collect device indices per instance
    let instance_device_indices: Vec<Vec<usize>> = instances
        .iter()
        .map(|inst| inst.devices.clone())
        .collect();

    gptokeyb::spawn_all_daemons(
        &h.gptokeyb,
        &h.path_handler,
        input_devices,
        &instance_device_indices,
    )
}
