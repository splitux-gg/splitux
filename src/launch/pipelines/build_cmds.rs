//! Command building pipeline

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app::{PadFilterType, SplituxConfig};
use crate::backend;
use crate::backend::facepunch::get_linux_bepinex_env;
use crate::backend::photon::generate_all_configs as photon_generate_configs;
use crate::bwrap;
use crate::gamescope;
use crate::handler::{Handler, SDL2Override};
use crate::input::DeviceInfo;
use crate::instance::Instance;
use crate::monitor::Monitor;
use crate::paths::{PATH_PARTY, PATH_STEAM};
use crate::proton;
use crate::util::*;

use super::super::operations::fuse_overlayfs_mount_gamedirs;
use super::super::pure::validate_runtime;
use super::super::types::SDL_GAMECONTROLLER_IGNORE_DEVICES;

/// Build launch commands for all instances
///
/// The `audio_sink_envs` parameter is a list of PULSE_SINK values per instance.
/// Empty string means no audio routing for that instance.
///
/// The `gptokeyb_virtual_devices` parameter contains the path to each instance's
/// virtual keyboard/mouse device created by gptokeyb (None if gptokeyb not used).
/// Returns Vec of (Command, bwrap_arg_count) where bwrap_arg_count is the
/// number of args before the child command. Device blocking args are inserted
/// at this position at spawn time for fresh permission checks.
pub fn launch_cmds(
    h: &Handler,
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    monitors: &[Monitor],
    cfg: &SplituxConfig,
    audio_sink_envs: &[String],
    gptokeyb_virtual_devices: &[Option<PathBuf>],
) -> Result<Vec<(std::process::Command, usize)>, Box<dyn std::error::Error>> {
    let win = h.win();
    let exec = Path::new(&h.exec);
    let runtime = h.runtime.as_str();

    // Validate Steam Runtime if needed
    validate_runtime(runtime)?;

    // Create backend overlays if needed (before mounting game dirs)
    let backend_overlays = if h.is_saved_handler() {
        backend::create_backend_overlays(h, instances, win)?
    } else {
        vec![]
    };

    // Generate Photon configs at launch time (needs instance count)
    if h.has_photon() && h.is_saved_handler() {
        photon_generate_configs(h, instances)?;
    }

    // Mount game directories with overlays
    if h.is_saved_handler() && !cfg.disable_mount_gamedirs {
        fuse_overlayfs_mount_gamedirs(h, instances, &backend_overlays)?;
    }

    let mut cmds: Vec<(Command, usize)> = Vec::new();

    for (i, instance) in instances.iter().enumerate() {
        let gamedir = if h.is_saved_handler() && !cfg.disable_mount_gamedirs {
            PATH_PARTY.join("tmp").join(format!("game-{}", i))
        } else {
            PathBuf::from(h.get_game_rootpath()?)
        };

        if !gamedir.join(exec).exists() {
            return Err(format!("Executable not found: {}", gamedir.join(exec).display()).into());
        }

        let path_exec = gamedir.join(exec);
        let cwd = path_exec.parent().ok_or_else(|| "couldn't get parent")?;
        let path_prof = PATH_PARTY.join("profiles").join(&instance.profname);

        // 1. Create gamescope command
        let mut cmd = gamescope::create_command(cfg);
        cmd.current_dir(cwd);

        // 2. Set up gamescope environment
        gamescope::setup_env(&mut cmd);

        // Proton debug logging
        cmd.env("PROTON_LOG", "1");
        cmd.env("WINEDEBUG", "trace+dinput,trace+xinput");
        cmd.env("PROTON_USE_XALIA", "0");

        // SDL2 override if configured
        if h.sdl2_override != SDL2Override::No {
            let path_sdl = match h.sdl2_override {
                SDL2Override::Srt => {
                    PATH_STEAM.join("ubuntu12_32/steam-runtime/usr/lib/i386-linux-gnu/libSDL2-2.0.so.0")
                }
                SDL2Override::Sys => PathBuf::from("/usr/lib/libSDL2.so"),
                _ => PathBuf::new(),
            };
            cmd.env("SDL_DYNAMIC_API", path_sdl);
        }

        // Proton environment (for Windows games)
        if win {
            proton::setup_env(&mut cmd, h, cfg, i);

            // BepInEx doorstop requires native winhttp.dll override
            // Without this, Wine uses its builtin and BepInEx never loads
            if h.has_photon() || h.has_facepunch() || h.has_goldberg_plugin() || h.has_standalone() {
                cmd.env("WINEDLLOVERRIDES", "winhttp=n,b");
            }
        }

        // Steam Input configuration
        if cfg.pad_filter_type != PadFilterType::NoSteamInput {
            cmd.env("SDL_GAMECONTROLLER_ALLOW_STEAM_VIRTUAL_GAMEPAD", "1");
        }
        if cfg.pad_filter_type == PadFilterType::OnlySteamInput {
            cmd.env("SDL_GAMECONTROLLER_IGNORE_DEVICES", SDL_GAMECONTROLLER_IGNORE_DEVICES);
        }

        // Handler custom environment variables
        if !h.env.is_empty() {
            for env_var in h.env.split_whitespace() {
                if let Some((key, value)) = env_var.split_once('=') {
                    cmd.env(key, value);
                }
            }
        }

        // 3. Add gamescope arguments
        gamescope::add_args(&mut cmd, instance, monitors, cfg);
        let virtual_device = gptokeyb_virtual_devices.get(i).and_then(|v| v.as_ref());
        gamescope::add_input_holding_args(&mut cmd, virtual_device.map(|p| p.as_path()), cfg);
        gamescope::add_separator(&mut cmd);

        // 4. Add bwrap container (unless disabled)
        if !h.disable_bwrap {
            bwrap::add_base_args(&mut cmd);

            // Get gamepad paths for this instance
            let gamepad_paths = bwrap::get_assigned_gamepad_paths(input_devices, &instance.devices);
            if !gamepad_paths.is_empty() {
                println!("[splitux] Instance {}: SDL_JOYSTICK_DEVICE={}", i, gamepad_paths.join(","));
            }

            // Set up SDL environment inside container
            // Skip SDL config entirely if input isolation disabled (like test-repo.sh)
            if !h.disable_input_isolation {
                bwrap::setup_sdl_env(&mut cmd, &gamepad_paths);
            }

            // Set up audio routing inside container
            if let Some(sink_name) = audio_sink_envs.get(i) {
                if !sink_name.is_empty() {
                    bwrap::setup_audio_env(&mut cmd, sink_name);
                    println!("[splitux] Instance {}: PULSE_SINK={}", i, sink_name);
                }
            }

            // Set up BepInEx environment for Linux native games with Facepunch
            if !win && h.has_facepunch() {
                let bepinex_env = get_linux_bepinex_env(&gamedir);
                if !bepinex_env.is_empty() {
                    bwrap::setup_bepinex_env(&mut cmd, &bepinex_env);
                }
            }

            // Set Steam App ID for native Linux games (required for Steam API init)
            if !win {
                if let Some(appid) = h.steam_appid {
                    cmd.args(["--setenv", "SteamAppId", &appid.to_string()]);
                    cmd.args(["--setenv", "SteamGameId", &appid.to_string()]);
                }
            }

            // Log assigned devices and block unassigned devices
            if !h.disable_input_isolation {
                bwrap::log_assigned_devices(&mut cmd, input_devices, &instance.devices, i);
            }

            // 5. Profile bindings
            if win {
                let path_pfx_user = proton::get_prefix_user_path(cfg, i);
                cmd.arg("--bind")
                    .args([&path_prof.join("windata"), &path_pfx_user]);
            } else {
                let path_prof_home = path_prof.join("home");
                // Set HOME inside bwrap container (not on parent process)
                cmd.args(["--setenv", "HOME", &path_prof_home.to_string_lossy()]);
            }

            // 6. Game null paths (disable specific game features)
            for subpath in &h.game_null_paths {
                let game_subpath = gamedir.join(subpath);
                if game_subpath.is_file() {
                    cmd.args(["--bind", "/dev/null", &game_subpath.to_string_lossy()]);
                } else if game_subpath.is_dir() {
                    cmd.args([
                        "--bind",
                        &PATH_PARTY.join("tmp/null").to_string_lossy(),
                        &game_subpath.to_string_lossy(),
                    ]);
                }
            }

        } else {
            println!("[splitux] Instance {}: bwrap disabled, skipping container", i);
        }

        // Record arg count at end of bwrap section (before runtime/game args).
        // Device blocking args will be inserted at this position at spawn time.
        let bwrap_arg_count = cmd.get_args().count();

        // 7. Runtime (Proton/Wine or Steam Runtime)
        if win {
            let proton_bin = proton::get_binary(h)?;
            cmd.arg(&proton_bin);

            // Add waitforexitandrun only for direct Proton (not umu-run)
            if proton::uses_direct_proton(h) {
                cmd.arg("waitforexitandrun");
            }
        } else {
            match runtime {
                "scout" => {
                    cmd.arg(PATH_STEAM.join("ubuntu12_32/steam-runtime/run.sh"));
                }
                "soldier" => {
                    cmd.arg(
                        PATH_STEAM.join(
                            "steamapps/common/SteamLinuxRuntime_soldier/_v2-entry-point",
                        ),
                    );
                    cmd.arg("--");
                }
                _ => {}
            };
        }

        // 8. Game executable
        cmd.arg(&path_exec);

        // 9. Handler arguments with variable substitution
        for arg in h.args.split_whitespace() {
            let processed_arg = match arg {
                "$PROFILE" => &instance.profname,
                "$WIDTH" => &instance.width.to_string(),
                "$HEIGHT" => &instance.height.to_string(),
                "$RESOLUTION" => &format!("{}x{}", instance.width, instance.height),
                "$INSTANCECOUNT" => &instances.len().to_string(),
                "$INSTANCENUM" => &i.to_string(),
                "$GAMEDIR" => &gamedir.os_fmt(win),
                "$HANDLERDIR" => &h.path_handler.os_fmt(win),
                _ => &String::from(arg).sanitize_path(),
            };
            cmd.arg(processed_arg);
        }

        cmds.push((cmd, bwrap_arg_count));
    }

    Ok(cmds)
}

