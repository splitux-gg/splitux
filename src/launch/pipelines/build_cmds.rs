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
use crate::paths::{PATH_ASSETS, PATH_PARTY, PATH_STEAM};
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
    together_devices: &[Vec<crate::together::TogetherSeatDevices>],
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

        // Align the game's graphics driver with the configured GPU vendor
        // (gpu_vendor=auto detects from the DRM render node). Centralizes the
        // LIBVA/GLX/GBM driver env so native GL/Vulkan games and Proton resolve
        // the right driver instead of inheriting a stale/foreign one.
        for (k, v) in cfg.gpu_vendor.driver_env() {
            cmd.env(k, v);
        }

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

        // EOS emulator (splitux eos_sdk_emu) identity. The emu is configured
        // entirely through its native EOSLAN_* env (no Nemirtingas JSON). Each
        // instance MUST get a distinct, stable username: the emu derives a
        // deterministic per-instance EpicAccountId + ProductUserId from it, and
        // UE's account registry enforces one puid <-> one epic, so a shared
        // username collides and breaks the EOS join. profname is unique per
        // instance and stable across the game's (multi-)process launch.
        if h.has_eos() {
            cmd.env("EOSLAN_USERNAME", &instance.profname);
            // Diagnostic: capture the emu's own debug log per instance to a
            // persistent path. It MUST live outside the sandbox's private
            // `--tmpfs /tmp` (which is discarded at teardown) — PATH_PARTY is
            // bind-visible under `--dev-bind / /` and survives. Without this the
            // emu log (EOSLAN_DEBUG=1) goes nowhere under splitux, unlike the
            // bench which sets EOSLAN_LOG_PATH. Wine maps the unix path via Z:.
            let eos_log = PATH_PARTY.join(format!("eos-emu-{}.log", instance.profname));
            cmd.env("EOSLAN_LOG_PATH", &eos_log);
        }

        // Goldberg force-logging. A release steam_api64.dll only writes its
        // debug log when GSE_FORCE_LOG is set (our gbe_fork upgrade). Per-instance
        // log on a persistent path so the bridge (and discovery/connect) is
        // observable, mirroring the bench. Unlike the EOS emu, goldberg wants a
        // Windows-style path, so prefix the unix path with wine's Z: drive (maps
        // to /). The log MUST live outside the sandbox's `--tmpfs /tmp` — PATH_PARTY
        // is bind-visible under `--dev-bind / /` and survives teardown.
        if h.has_goldberg() {
            let gse_log = PATH_PARTY.join(format!("gse-{}.log", instance.profname));
            cmd.env("GSE_FORCE_LOG", "1");
            cmd.env("GSE_LOG_PATH", format!("Z:{}", gse_log.display()));

            // Goldberg save/userdata base. Pin GseSavePath to a stable absolute
            // per-profile dir so goldberg's save path (and GetUserDataFolder) is
            // deterministic and never falls back to its in-sandbox default
            // resolution — which can degrade to a relative module-name base
            // ("libsteam_api.so/userdata/...") and abort games that build their
            // data dir from it (e.g. Chronicon). A handler may override the base
            // via goldberg.save_path. goldberg reads GseSavePath verbatim, so the
            // Windows (Proton/wine) path needs the Z: drive prefix; native .so
            // builds take the plain unix path.
            let save_base = h
                .goldberg_ref()
                .and_then(|g| g.save_path.clone())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    PATH_PARTY
                        .join("profiles")
                        .join(&instance.profname)
                        .join("goldberg-saves")
                });
            let _ = std::fs::create_dir_all(&save_base);
            if h.win() {
                cmd.env("GseSavePath", format!("Z:{}", save_base.display()));
            } else {
                cmd.env("GseSavePath", &save_base);
            }

            // Goldberg program/app path. goldberg's get_full_program_path() falls
            // back to the dirname of the loaded steam_api lib path — but in the
            // sandbox that lib can resolve to the bare relative module name
            // ("libsteam_api.so"), which has no dirname, yielding a bogus
            // "libsteam_api.so/" base. Games that build their data dir from it then
            // try to mkdir under a *file* and abort (Chronicon:
            // "libsteam_api.so/userdata/<id>/<appid>"). Pin GseAppPath to the
            // absolute sandbox game dir so it's always a real directory. Windows
            // (Proton/wine) needs the Z: drive prefix; native takes the unix path.
            if h.win() {
                cmd.env("GseAppPath", format!("Z:{}", cwd.display()));
            } else {
                cmd.env("GseAppPath", cwd);
            }
        }

        // Goldberg raw-UDP <-> legacy-Steam-P2P bridge (goldberg.p2p_bridge,
        // opt-in). Mirrors the bench's GSE_IP_P2P_BRIDGE: for IP-LAN games whose
        // host listens via legacy ISteamNetworking P2P while joiners connect
        // raw. The goldberg DLL only activates the bridge when this env is set,
        // so it stays inert for games that don't need it.
        if h.goldberg_ref().map(|g| g.p2p_bridge).unwrap_or(false) {
            cmd.env("GSE_IP_P2P_BRIDGE", "1");
            println!("[splitux] Instance {}: GSE_IP_P2P_BRIDGE=1 (goldberg P2P bridge)", i);
        }

        // splitux-together: this instance's remote seats (if any). One in the
        // online/LAN case; N for a local-split (couch-co-op) game where several
        // browsers drive the one instance. Each seat's virtual kbd/mouse are
        // ALWAYS held by gamescope (so remote keystrokes reach the game, not the
        // host desktop); their pads are wired into the game's SDL below only when
        // the player is set to Gamepad input.
        let seats: &[crate::together::TogetherSeatDevices] =
            together_devices.get(i).map(Vec::as_slice).unwrap_or(&[]);

        // 3. Add gamescope arguments
        gamescope::add_args(&mut cmd, instance, monitors, cfg);
        let virtual_device = gptokeyb_virtual_devices.get(i).and_then(|v| v.as_ref());
        gamescope::add_input_holding_args(&mut cmd, virtual_device.map(|p| p.as_path()), cfg);
        if !seats.is_empty() {
            // Drive the compositor at the fps tier once for the instance, then
            // hold EVERY seat's kbd/mouse (gamescope takes repeated
            // --libinput-hold-dev). Give the instance's PipeWire capture a
            // unique, targetable node name so its seat-streamer(s) bind to THIS
            // gamescope; all of this instance's seats share that one node
            // (multi-consumer) — which is exactly how local-split fans out.
            gamescope::add_together_refresh_rate(&mut cmd, cfg);
            for seat in seats {
                gamescope::add_seat_hold_args(&mut cmd, seat, cfg);
            }
            cmd.env(
                "GAMESCOPE_PIPEWIRE_NODE",
                crate::together::node_name_for_instance(i),
            );
        }
        gamescope::add_separator(&mut cmd);

        // 4. Add bwrap container (unless disabled)
        if !h.disable_bwrap {
            bwrap::add_base_args(&mut cmd);

            // goldberg.steamclient: shadow Proton's steamclient COPY SOURCE with
            // goldberg's experimental steamclient. These games resolve Steam via
            // the steamclient path (lsteamclient -> C:\…\Steam\steamclient64.dll)
            // and never load goldberg's steam_api. Proton copies that DLL from
            // {STEAM_COMPAT_CLIENT_INSTALL_PATH}/legacycompat/ into the prefix on
            // EVERY launch (try_copy clobbers unconditionally), so pre-placing
            // goldberg's in the prefix is futile — and ro-binding over the prefix
            // copy *target* would crash Proton (its os.remove() of a bind mount
            // hits EBUSY, which try_copy doesn't forgive). Instead we ro-bind
            // goldberg's steamclient over the copy *source* inside the sandbox:
            // Proton reads it and copies GOLDBERG's steamclient into the prefix, so
            // the game's lsteamclient loads the offline emulator instead of real
            // Steam (which would otherwise fall through to steam://run and exit).
            // Per-instance identity flows via GseAppPath/steam_settings (root
            // steam_settings is created in the goldberg overlay for this mode).
            if win && h.goldberg_ref().map(|g| g.steamclient).unwrap_or(false) {
                let gb_win = PATH_ASSETS.join("goldberg/win");

                // (a) Deploy goldberg's steamclient directly into THIS instance's
                // prefix Steam dir. Proton only re-copies steamclient when the
                // prefix config changes (cold / version-bumped prefix); on a WARM
                // prefix it SKIPS the copy, so the source-shadow below never fires
                // and the prefix keeps whatever steamclient it had — real Steam's,
                // which falls through to steam://run and exits. Writing goldberg's
                // here makes the warm path correct; the shadow (b) covers the
                // cold/copy-runs path (Proton then copies goldberg, not real Steam).
                let pfx_steam = proton::get_prefix_path(cfg, i)
                    .join("drive_c/Program Files (x86)/Steam");
                match std::fs::create_dir_all(&pfx_steam) {
                    Ok(()) => {
                        for dll in [
                            "steamclient64.dll",
                            "steamclient.dll",
                            "GameOverlayRenderer64.dll",
                            "GameOverlayRenderer.dll",
                        ] {
                            let src = gb_win.join(dll);
                            if !src.exists() {
                                continue;
                            }
                            let dst = pfx_steam.join(dll);
                            // Skip the (multi-MB) copy when the prefix already holds
                            // this exact build (size match) — common on warm prefixes.
                            let same = std::fs::metadata(&dst).map(|m| m.len()).ok()
                                == std::fs::metadata(&src).map(|m| m.len()).ok();
                            if same {
                                continue;
                            }
                            match std::fs::copy(&src, &dst) {
                                Ok(_) => println!(
                                    "[splitux] Instance {}: goldberg.steamclient deploy -> prefix: {}",
                                    i, dll
                                ),
                                Err(e) => println!(
                                    "[splitux] Instance {}: goldberg.steamclient deploy {} failed: {}",
                                    i, dll, e
                                ),
                            }
                        }
                    }
                    Err(e) => println!(
                        "[splitux] Instance {}: goldberg.steamclient: couldn't create prefix Steam dir: {}",
                        i, e
                    ),
                }

                // (b) Source-shadow: ro-bind goldberg's steamclient over Proton's
                // legacycompat copy source so the cold/version-bumped copy installs
                // goldberg's DLL into the prefix instead of real Steam's.
                let legacy = PATH_STEAM.join("legacycompat");
                // 64-bit pair covers modern titles; the 32-bit steamclient is bound
                // too when present so 32-bit games are handled. Each bind is applied
                // only if both the goldberg source and the legacycompat target exist.
                for dll in ["steamclient64.dll", "GameOverlayRenderer64.dll", "steamclient.dll"] {
                    let src = gb_win.join(dll);
                    let dst = legacy.join(dll);
                    if src.exists() && dst.exists() {
                        // legacycompat/{steamclient64,GameOverlayRenderer64}.dll are
                        // SYMLINKS into the Steam root; bwrap can't create a file
                        // mountpoint over a symlink ("Can't mkdir parents"). Resolve
                        // to the real target and shadow THAT — Proton's try_copy
                        // follows the symlink when reading the source, so shadowing
                        // the resolved file makes the copy pick up goldberg's DLL.
                        let dst = dst.canonicalize().unwrap_or(dst);
                        cmd.args([
                            "--ro-bind",
                            &src.to_string_lossy(),
                            &dst.to_string_lossy(),
                        ]);
                        println!(
                            "[splitux] Instance {}: goldberg steamclient shadow {} -> {}",
                            i,
                            src.display(),
                            dst.display()
                        );
                    } else if !src.exists() {
                        println!(
                            "[splitux] Instance {}: WARNING goldberg.steamclient set but asset missing: {}",
                            i,
                            src.display()
                        );
                    }
                }
            }

            // Get gamepad paths for this instance
            let mut gamepad_paths = bwrap::get_assigned_gamepad_paths(input_devices, &instance.devices);
            // Remote seats set to Gamepad input contribute their virtual pad, so
            // the game's SDL reads the friend's controller. Kb+Mouse seats add no
            // pad (no phantom controller for a pad-based game). A local-split
            // instance carries several gamepad seats → several pads on one game.
            //
            // EXCEPTION — EOS games: a session JOIN only completes once the game
            // has a device-backed local player. UE CommonUser binds a UserIdx on
            // a "controller connection changed" event; a Kb+Mouse seat is injected
            // by gamescope and exposes NO input device, so the joiner never binds a
            // local player and the EOS join silently aborts to the menu (E007) —
            // the game receives JoinSession=EOS_Success but then makes no further
            // EOS calls and never travels. The seat-streamer always creates a
            // virtual pad (wait_for_seat_devices requires pad+kbd+mouse), so for
            // EOS games we wire it in for Kb+Mouse seats too: it gives the joiner
            // the controller-connection it needs to bind a local player while
            // keyboard/mouse still drive gameplay via gamescope injection.
            let wire_seat_pads = instance.together_input
                == crate::instance::TogetherInput::Gamepad
                || h.has_eos();
            if wire_seat_pads {
                for seat in seats {
                    if let Some(pad) = &seat.pad {
                        gamepad_paths.push(pad.to_string_lossy().to_string());
                    }
                }
            }
            if !gamepad_paths.is_empty() {
                println!("[splitux] Instance {}: SDL_JOYSTICK_DEVICE={}", i, gamepad_paths.join(","));
            }

            // Set up SDL environment inside container. Keep it for both isolation
            // modes: it forces SDL onto evdev (HIDAPI off) and pins the device,
            // which keeps SDL games well-behaved alongside the evdev allowlist.
            // Skip only when isolation is fully off.
            if h.effective_input_isolation() != crate::handler::InputIsolation::None {
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

        // Diagnostic (opt-in): pass UE `-LogCmds` to EOS games when the
        // SPLITUX_GAME_LOGCMDS env var is set, e.g.
        //   SPLITUX_GAME_LOGCMDS="LogOnlineServices Verbose, LogNet Verbose"
        // The whole value carries spaces, so it's emitted as ONE argv element;
        // wine wraps the space-containing arg in quotes when building the Windows
        // command line, so UE sees a single `-LogCmds=<value>` token (no inner
        // quoting needed). Used to surface why an EOS join aborts post-Success.
        if h.has_eos() {
            if let Ok(logcmds) = std::env::var("SPLITUX_GAME_LOGCMDS") {
                if !logcmds.trim().is_empty() {
                    cmd.arg(format!("-LogCmds={logcmds}"));
                }
            }
        }

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

