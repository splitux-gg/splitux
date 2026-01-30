//! Game execution pipeline

use std::process::Child;

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
use super::super::pure::command::{format_launch_cmd, rebuild_command_with_blocking};

/// Launch the game with all instances
pub fn launch_game(
    h: &Handler,
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    monitors: &[Monitor],
    cfg: &SplituxConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Set up audio routing if enabled
    let (audio_system, virtual_sinks, audio_sink_envs) = setup_audio_routing(instances, cfg);

    // Set up gptokeyb daemons if enabled (spawns before command building so we can pass virtual device paths)
    let (mut gptokeyb_handles, gptokeyb_virtual_devices) =
        setup_gptokeyb_daemons(h, input_devices, instances);

    let new_cmds = launch_cmds(
        h,
        input_devices,
        instances,
        monitors,
        cfg,
        &audio_sink_envs,
        &gptokeyb_virtual_devices,
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

    println!("[splitux] Setting up {} window manager", wm.name());
    wm.setup(&ctx)?;

    // Delay after each spawn for Vulkan/GPU initialization
    let vulkan_init_delay = 6.0;

    // Delay before each spawn for input/SDL initialization
    let input_init_delay = cfg.input_init_delay.unwrap_or(1.0);

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

        // Build fresh device blocking args right before spawn (spawn-time permission check).
        // These must be inserted as bwrap args, before the child command (proton/game).
        let blocking_args = if !h.disable_bwrap && !h.disable_input_isolation {
            let initial_js_devices = bwrap::glob_js_devices();
            let mut args = bwrap::get_js_blocking_args(&initial_js_devices, i);
            args.extend(bwrap::get_evdev_hidraw_blocking_args(
                input_devices,
                &instances[i].devices,
                i,
            ));
            args
        } else {
            Vec::new()
        };

        // Reconstruct command with blocking args inserted at the bwrap/child boundary
        let mut cmd = rebuild_command_with_blocking(cmd, bwrap_arg_count, &blocking_args);

        // Print the final command (with blocking args)
        print!("{}", format_launch_cmd(&cmd, i));
        println!();

        if redirect_stdout {
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());
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

    for mut handle in handles {
        handle.wait()?;
    }

    // Teardown WM
    println!("[splitux] Tearing down {} window manager", wm.name());
    wm.teardown()?;

    // Teardown gptokeyb daemons
    gptokeyb::terminate_all(&mut gptokeyb_handles);

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
