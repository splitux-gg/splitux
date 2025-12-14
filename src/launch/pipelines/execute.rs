//! Game execution pipeline

use crate::app::{PartyConfig, WindowManagerType};
use crate::facepunch;
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
    let new_cmds = launch_cmds(h, input_devices, instances, cfg)?;
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
    let redirect_stdout = !h.win() && facepunch::uses_facepunch(h);

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

    Ok(())
}
