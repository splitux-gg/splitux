//! Command building pipeline

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app::{PadFilterType, PartyConfig};
use crate::backend;
use crate::bwrap;
use crate::facepunch;
use crate::gamescope;
use crate::handler::{Handler, SDL2Override};
use crate::input::DeviceInfo;
use crate::instance::Instance;
use crate::paths::{PATH_PARTY, PATH_STEAM};
use crate::photon;
use crate::proton;
use crate::util::*;

use super::super::operations::fuse_overlayfs_mount_gamedirs;
use super::super::pure::validate_runtime;
use super::super::types::SDL_GAMECONTROLLER_IGNORE_DEVICES;

/// Build launch commands for all instances
pub fn launch_cmds(
    h: &Handler,
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    cfg: &PartyConfig,
) -> Result<Vec<std::process::Command>, Box<dyn std::error::Error>> {
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
        photon::generate_all_configs(h, instances)?;
    }

    // Mount game directories with overlays
    if h.is_saved_handler() && !cfg.disable_mount_gamedirs {
        fuse_overlayfs_mount_gamedirs(h, instances, &backend_overlays)?;
    }

    let mut cmds: Vec<Command> = Vec::new();

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
        gamescope::add_args(&mut cmd, instance, cfg);
        gamescope::add_input_holding_args(&mut cmd, instance, input_devices, cfg);
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
            bwrap::setup_sdl_env(&mut cmd, &gamepad_paths);

            // Set up BepInEx environment for Linux native games with Facepunch
            if !win && facepunch::uses_facepunch(h) {
                let bepinex_env = facepunch::get_linux_bepinex_env(&gamedir);
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

            // Block unassigned input devices
            bwrap::block_unassigned_devices(&mut cmd, input_devices, &instance.devices, i);

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

        cmds.push(cmd);
    }

    Ok(cmds)
}

/// Print launch commands for debugging
pub fn print_launch_cmds(cmds: &Vec<Command>) {
    for (i, cmd) in cmds.iter().enumerate() {
        println!("[splitux] INSTANCE {}:", i + 1);

        let cwd = cmd.get_current_dir().unwrap_or_else(|| Path::new(""));
        println!("[splitux] CWD={}", cwd.display());

        for var in cmd.get_envs() {
            let value = var.1.ok_or_else(|| "").unwrap_or_default();
            println!(
                "[splitux] {}={}",
                var.0.to_string_lossy(),
                value.display()
            );
        }

        println!("[splitux] \"{}\"", cmd.get_program().display());

        print!("[splitux] ");
        for arg in cmd.get_args() {
            let fmtarg = arg.to_string_lossy();
            if fmtarg == "--bind"
                || fmtarg == "bwrap"
                || (fmtarg.starts_with("/") && fmtarg.len() > 1)
            {
                print!("\n[splitux] ");
            } else {
                print!(" ");
            }
            print!("\"{}\"", fmtarg);
        }

        println!("\n[splitux] ---------------------");
    }
}
