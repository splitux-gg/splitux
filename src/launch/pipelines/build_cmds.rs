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
    handlers: &[Handler],
    input_devices: &[DeviceInfo],
    instances: &Vec<Instance>,
    monitors: &[Monitor],
    cfg: &SplituxConfig,
    audio_sink_envs: &[String],
    gptokeyb_virtual_devices: &[Option<PathBuf>],
    together_devices: &[Vec<crate::together::TogetherSeatDevices>],
) -> Result<Vec<(std::process::Command, usize)>, Box<dyn std::error::Error>> {
    // The per-instance loop reads its own `handlers[instance.game]`; the batch
    // ops below are now grouped per game internally, so single-game (every
    // `instance.game == 0`) is byte-identical.

    // Per-game (unit) instance numbering. `$INSTANCENUM`/`$INSTANCECOUNT` and the
    // goldberg "first instance reports the real save's steam id" rule are
    // per-unit, not global. For a single-game launch these equal the global
    // index / total, so nothing changes.
    let games: Vec<usize> = instances.iter().map(|inst| inst.game).collect();
    let (game_inst_nums, game_inst_counts) =
        crate::launch::pure::per_game_instance_numbering(&games);

    // Backend overlays, GLOBAL-indexed, grouped per game (each unit built from
    // its own handler → mixed backends across games + per-game goldberg lobby
    // isolation). Non-saved-handler games contribute empty lists.
    let backend_overlays = backend::create_backend_overlays(handlers, instances)?;

    // Generate Photon configs at launch time, per game (each game's own instance
    // count drives its configs). Group by game in first-seen order; emit only for
    // photon + saved-handler games.
    {
        let mut games_seen: Vec<usize> = Vec::new();
        let mut insts_by_game: std::collections::HashMap<usize, Vec<Instance>> =
            std::collections::HashMap::new();
        for inst in instances {
            insts_by_game
                .entry(inst.game)
                .or_insert_with(|| {
                    games_seen.push(inst.game);
                    Vec::new()
                })
                .push(inst.clone());
        }
        for game in games_seen {
            let gh = &handlers[game];
            if gh.has_photon() && gh.is_saved_handler() {
                photon_generate_configs(gh, &insts_by_game[&game])?;
            }
        }
    }

    // Mount game directories with overlays. Per-instance handler inside; the
    // mount skips non-saved-handler games (they run from their real game root).
    if !cfg.disable_mount_gamedirs {
        fuse_overlayfs_mount_gamedirs(handlers, instances, &backend_overlays)?;
    }

    let mut cmds: Vec<(Command, usize)> = Vec::new();

    for (i, instance) in instances.iter().enumerate() {
        // This instance's unit handler. Single-game: always handlers[0].
        let h = &handlers[instance.game];
        let win = h.win();
        let exec = Path::new(&h.exec);
        let runtime = h.runtime.as_str();
        validate_runtime(runtime)?;

        let game_inst_num = game_inst_nums[i];
        let game_inst_count = game_inst_counts[i];
        let is_first_in_game = game_inst_num == 0;

        let gamedir = if h.is_saved_handler() && !cfg.disable_mount_gamedirs {
            crate::paths::launch_tmp_dir().join(format!("game-{}", i))
        } else {
            PathBuf::from(h.get_game_rootpath()?)
        };

        if !gamedir.join(exec).exists() {
            return Err(format!("Executable not found: {}", gamedir.join(exec).display()).into());
        }

        let path_exec = gamedir.join(exec);
        // Working dir for the game process (and goldberg's GseAppPath). Optional
        // handler `working_dir` (relative to game root) overrides the default for
        // launcher-shim games whose real binary lives in a subdir but must run
        // with cwd at a higher level (e.g. native Frozenbyte titles). Default =
        // the exec's parent dir.
        let cwd: PathBuf = if !h.working_dir.is_empty() {
            gamedir.join(&h.working_dir)
        } else {
            path_exec
                .parent()
                .ok_or_else(|| "couldn't get parent")?
                .to_path_buf()
        };
        let path_prof = PATH_PARTY.join("profiles").join(&instance.profname);

        // splitux-together: this instance's remote seats (if any). One in the
        // online/LAN case; N for a local-split (couch-co-op) game where several
        // browsers drive the one instance. Each seat's virtual kbd/mouse are
        // ALWAYS held by gamescope (so remote keystrokes reach the game, not the
        // host desktop); their pads are wired into the game's SDL below only when
        // the player is set to Gamepad input. Bound here (before the command root)
        // so the gamescope-bypass decision can see whether this instance streams.
        let seats: &[crate::together::TogetherSeatDevices] =
            together_devices.get(i).map(Vec::as_slice).unwrap_or(&[]);

        // Gamescope-bypass (cfg.disable_gamescope): run a single LOCAL seat
        // directly under the host compositor, with NO nested gamescope. ONLY for
        // a lone, non-together, bwrap'd instance — split-screen needs gamescope's
        // per-instance geometry and together needs its PipeWire capture, so those
        // always keep the nested compositor. Removes the double-compositor
        // scan-line artifact on high-refresh panels (see SplituxConfig docs). The
        // game inherits the session's X display (Xwayland-satellite), so its wine
        // display driver is unchanged — only the redundant compositor is dropped.
        let bypass_gamescope = h.effective_disable_gamescope(cfg)
            && instances.len() == 1
            && seats.is_empty()
            && !h.disable_bwrap;
        if bypass_gamescope {
            println!(
                "[splitux] Instance {}: disable_gamescope — running directly under the host \
                 compositor (no nested gamescope)",
                i
            );
        }

        // 1. Create the root command: gamescope normally, or bwrap directly when
        // bypassing gamescope for a single local seat.
        let mut cmd = if bypass_gamescope {
            Command::new("bwrap")
        } else {
            gamescope::create_command(cfg)
        };
        cmd.current_dir(&cwd);

        // 2. Set up gamescope environment (skipped when bypassing — these env
        // vars steer gamescope itself; the un-nested game inherits the session
        // environment, including DISPLAY for the host's Xwayland).
        if !bypass_gamescope {
            gamescope::setup_env(&mut cmd);
        }

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
            proton::setup_env(&mut cmd, h, cfg, &instance.profname, instance.game);

            // Gamescope-bypass: with no nested gamescope there is no embedded
            // Xwayland, and the host's rootless Xwayland (xwayland-satellite)
            // drops the warp-based relative-mouse that wine games use for
            // mouse-look (cursor is grabbed but the camera never turns). Run wine
            // as a NATIVE WAYLAND client (winewayland.drv) instead: relative
            // pointer + pointer-lock go through the Wayland protocols niri
            // implements, so mouse-look works AND the host compositor owns the
            // cursor grab (clean release on unfocus). It also makes niri see the
            // real game window (correct pid/app_id) so splitux can position it.
            // GE-Proton ships winewayland.drv. VALIDATED live on Enshrouded.
            if bypass_gamescope {
                cmd.env("PROTON_ENABLE_WAYLAND", "1");
            }

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
        // observable, mirroring the bench. The log MUST live outside the sandbox's
        // `--tmpfs /tmp` — PATH_PARTY is bind-visible under `--dev-bind / /` and
        // survives teardown. A WINDOWS (Proton/wine) game wants a Windows-style
        // path, so prefix the unix path with wine's Z: drive (maps to /); a NATIVE
        // .so build takes the plain unix path — the previous unconditional Z:
        // prefix produced a bogus "Z:/home/..." path that native gbe_fork couldn't
        // open, so it silently wrote NO log (which is why native goldberg games
        // like Brotato were undebuggable — no steam-id/init trace at all).
        if h.has_goldberg() {
            let gse_log = PATH_PARTY.join(format!("gse-{}.log", instance.profname));
            cmd.env("GSE_FORCE_LOG", "1");
            if h.win() {
                cmd.env("GSE_LOG_PATH", format!("Z:{}", gse_log.display()));
            } else {
                cmd.env("GSE_LOG_PATH", &gse_log);
            }

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

            // gbe_fork GLOBAL user settings. gbe_fork reads account_name /
            // account_steamid from a *global* settings dir (`<GseSavePath>/settings/
            // configs.user.ini`) that takes PRECEDENCE over the per-game
            // `steam_settings/configs.user.ini` we write per instance. If that global
            // file is absent, gbe_fork creates its own default ("gse orca" /
            // 76561198154692317) and reuses it for every game — so the steam id the
            // game actually reports has nothing to do with generate_steam_id(profname).
            // For save_steam_id_remap games (Brotato, DRG, …) the save folder is keyed
            // by the reported steam id, so the remap target (generate_steam_id) and the
            // folder the game reads diverge → the profile's save is never picked up.
            // Pin the global settings to the SAME id the per-game config and the save
            // remap use, so all three agree.
            let global_settings = save_base.join("settings");
            if std::fs::create_dir_all(&global_settings).is_ok() {
                // Must match the per-game GoldbergConfig.steam_id (goldberg.rs):
                // first instance reports the REAL save's id, others a generated id.
                let sid = if is_first_in_game {
                    crate::save_sync::pure::effective_steam_id(h, &instance.profname)
                } else {
                    crate::profiles::generate_steam_id(&instance.profname)
                };
                let user_ini = format!(
                    "[user::general]\naccount_name={}\naccount_steamid={}\nlanguage=english\nip_country=US\n",
                    instance.profname, sid
                );
                let _ = std::fs::write(global_settings.join("configs.user.ini"), user_ini);
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

        // 3. Add gamescope arguments (skipped entirely when bypassing — the game
        // runs directly under the host compositor with no nested gamescope).
        if !bypass_gamescope {
            gamescope::add_args(&mut cmd, instance, monitors, cfg);
            // Fullscreen single / online-co-op games (handler flag): fills the output
            // at native res instead of a ~720p floating window AND confines the cursor
            // to the output. Skipped for local split-screen (sub-region instances).
            if h.fullscreen {
                gamescope::add_fullscreen(&mut cmd);
            }
            let virtual_device = gptokeyb_virtual_devices.get(i).and_then(|v| v.as_ref());
            // NOTE: the instance's REAL keyboard/mouse are deliberately NOT held here.
            // Exclusively grabbing them (--libinput-hold-dev) to "confine the cursor"
            // removed the mouse from the host without gamescope presenting a usable
            // confined cursor — the device just vanished. For a kb/mouse seat the mouse
            // must stay usable, so gamescope gets input via normal compositor focus and
            // the fullscreen window naturally keeps the pointer. Only gptokeyb's virtual
            // device (controller→kb/m output) is held below. Real confinement, if ever
            // needed for multi-instance, must use pointer-constraints, not an exclusive
            // grab — see ~/.claude/plans/skittish-grabbing-cursor.md.
            gamescope::add_input_holding_args(&mut cmd, virtual_device.map(|p| p.as_path()), cfg);
            if !seats.is_empty() {
                // Cap the compositor at the stream fps tier — TOGETHER instances ONLY,
                // for PipeWire capture pacing (the headless backend else defaults to
                // 60Hz and the encode rate must match). LOCAL instances are left
                // UNCAPPED at the display's native refresh: capping a local seat to the
                // stream tier (e.g. 60 on a 200Hz panel) makes gamescope's frame limiter
                // strobe black frames on motion — a gamescope-only present artifact,
                // absent under native presentation (native Lutris clean; -r 60 not).
                gamescope::add_refresh_rate(&mut cmd, cfg);
                // Hold EVERY seat's kbd/mouse (gamescope takes repeated
                // --libinput-hold-dev). Give the instance's PipeWire capture a
                // unique, targetable node name so its seat-streamer(s) bind to THIS
                // gamescope; all of this instance's seats share that one node
                // (multi-consumer) — which is exactly how local-split fans out.
                for seat in seats {
                    gamescope::add_seat_hold_args(&mut cmd, seat, cfg);
                }
                // Mixed couch session: a local host shares this collapsed instance
                // with the remote seat(s). Holding the seat devices blocks parent
                // compositor input by default, so re-open it for the host's kb/m.
                if instance.local_input {
                    gamescope::add_libinput_allow_parent(&mut cmd, cfg);
                    println!(
                        "[splitux] Instance {}: --libinput-allow-parent (local host shares this together instance)",
                        i
                    );
                }
                cmd.env(
                    "GAMESCOPE_PIPEWIRE_NODE",
                    crate::together::node_name_for_instance(i),
                );
            }
            gamescope::add_separator(&mut cmd);
        }

        // 4. Add bwrap container (unless disabled). When bypassing gamescope the
        // command is rooted directly at bwrap, so add_base_args must not re-emit
        // the leading `bwrap` argument (as_program = bypass_gamescope).
        if !h.disable_bwrap {
            bwrap::add_base_args(&mut cmd, bypass_gamescope);

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
                let pfx_steam = proton::get_prefix_path(cfg, &instance.profname, instance.game)
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

            // Set up SDL environment inside container. This must run whenever this
            // instance has pads to wire — INDEPENDENT of device isolation. gamescope
            // exports SDL_JOYSTICK_DEVICE=/dev/null (so its own SDL ignores host
            // pads) and the game inherits it; udev joystick enumeration doesn't work
            // inside bwrap, so SDL falls back to that hint and sees NO controller
            // unless we override SDL_JOYSTICK_DEVICE with the real pad. Gating this
            // on isolation != None meant isolation:none handlers (Overcooked, Trine)
            // inherited /dev/null → the pad never appeared and "Press A" did nothing,
            // even though the seat's virtual pad was wired in. So: set it up for any
            // isolated handler AND for any handler that has gamepads to expose.
            if h.effective_input_isolation() != crate::handler::InputIsolation::None
                || !gamepad_paths.is_empty()
            {
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
                let path_pfx_user = proton::get_prefix_user_path(cfg, &instance.profname, instance.game);
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
                "$INSTANCECOUNT" => &game_inst_count.to_string(),
                "$INSTANCENUM" => &game_inst_num.to_string(),
                "$GAMEDIR" => &gamedir.os_fmt(win),
                "$HANDLERDIR" => &h.path_handler.os_fmt(win),
                _ => &String::from(arg).sanitize_path(),
            };
            cmd.arg(processed_arg);
        }

        // 9b. Backend-injected launch args (e.g. Keen's
        // --keenonline-server-data-file). `win` is this game's Proton/wine-ness.
        for arg in crate::backend::collect_backend_launch_args(h, win) {
            cmd.arg(arg);
        }

        cmds.push((cmd, bwrap_arg_count));
    }

    Ok(cmds)
}

