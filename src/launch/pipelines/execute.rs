//! Game execution pipeline

use crate::app::{PartyConfig, WindowManagerType};
use crate::audio::{
    resolve_audio_system, setup_audio_session, teardown_audio_session, AudioContext, AudioSystem,
    VirtualSink,
};
use crate::handler::Handler;
use crate::input::DeviceInfo;
use crate::instance::Instance;
use crate::monitor::Monitor;
use crate::wm::{LayoutContext, LayoutOrientation, WindowManager, WindowManagerBackend};

use super::build_cmds::{launch_cmds, print_launch_cmds};

/// Launch the game with all instances
pub fn launch_game(
    h: &Handler,
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    monitors: &[Monitor],
    cfg: &PartyConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Set up audio routing if enabled
    let (audio_system, virtual_sinks, audio_sink_envs) = setup_audio_routing(instances, cfg);

    let new_cmds = launch_cmds(h, input_devices, instances, cfg, &audio_sink_envs)?;
    print_launch_cmds(&new_cmds);

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
    let orientation = if cfg.vertical_two_player {
        LayoutOrientation::Vertical
    } else {
        LayoutOrientation::Horizontal
    };

    let ctx = LayoutContext {
        instances: instances.clone(),
        monitors: monitors.to_vec(),
        orientation,
    };

    println!("[splitux] Setting up {} window manager", wm.name());
    wm.setup(&ctx)?;

    let sleep_time = match h.pause_between_starts {
        Some(f) => f,
        None => 0.5,
    };

    let mut handles = Vec::new();

    // For native Linux games with Facepunch/BepInEx, redirect stdout to prevent
    // CStreamWriter crash. BepInEx's LinuxConsoleDriver checks isatty(1) and crashes
    // if stdout is a TTY. Redirecting to null makes isatty(1) return false.
    let redirect_stdout = !h.win() && h.has_facepunch();

    let mut i = 0;
    for mut cmd in new_cmds {
        if redirect_stdout {
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());
        }
        let handle = cmd.spawn()?;
        handles.push(handle);

        if i < instances.len() - 1 {
            std::thread::sleep(std::time::Duration::from_secs_f64(sleep_time));
        }
        i += 1;
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

    // Teardown audio routing
    if !virtual_sinks.is_empty() {
        if let Err(e) = teardown_audio_session(audio_system, &virtual_sinks) {
            println!("[splitux] Warning: Audio teardown failed: {}", e);
        }
    }

    Ok(())
}

/// Set up audio routing for all instances
///
/// Returns (audio_system, virtual_sinks, sink_env_vars_per_instance)
fn setup_audio_routing(
    instances: &[Instance],
    cfg: &PartyConfig,
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
