//! Headless command-line interface for splitux.
//!
//! Lets a full session be discovered and (later) launched without the egui GUI,
//! so a remote-play "together" session can be stood up from a script or over
//! SSH. The CLI reuses the exact scanning + launch pipeline the GUI drives —
//! it only replaces the interactive *assembly* of the session with flags.
//!
//! `run_if_cli()` is called from `main()` before the GUI is created: if the
//! first argument names a subcommand we handle it and return an exit code;
//! otherwise we return `None` and `main()` falls through to the GUI.

use std::sync::atomic::AtomicBool;

use clap::{Parser, Subcommand, ValueEnum};

use crate::config::load_cfg;
use crate::handler::scan_handlers;
use crate::input::{scan_input_devices, DeviceInfo, DeviceType};
use crate::instance::{
    set_instance_names, set_instance_resolutions, set_instance_resolutions_multimonitor, Instance,
    TogetherInput,
};
use crate::launch::run_session;
use crate::monitor::get_monitors_sdl;
use crate::profiles::scan_profiles;

#[derive(Parser)]
#[command(name = "splitux", about = "Split-screen / remote-play game launcher")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List installed games, profiles, or connected input devices.
    List {
        #[arg(value_enum)]
        what: ListWhat,
    },
    /// Launch a full session headlessly (no GUI).
    Launch {
        /// Game name (case-insensitive), e.g. "Palworld". See `list games`.
        #[arg(long)]
        game: String,
        /// One per player, repeatable. Comma-separated `key=val`:
        ///   profile=<name>   (default: Guest)
        ///   input=<together:gamepad|together:kbm>
        /// e.g. --player profile=Gabe,input=together:gamepad
        #[arg(long = "player", value_name = "SPEC", required = true)]
        players: Vec<String>,
    },
}

#[derive(ValueEnum, Clone, Copy)]
enum ListWhat {
    Games,
    Profiles,
    Inputs,
}

/// Run the CLI if a subcommand was given. Returns `Some(exit_code)` when the CLI
/// handled the invocation, or `None` when there's no recognized subcommand and
/// `main()` should launch the GUI instead.
pub fn run_if_cli() -> Option<i32> {
    // Engage clap only when argv[1] is one of our subcommands, so a plain
    // `splitux` (or one launched with stray toolkit args) still opens the GUI.
    let first = std::env::args().nth(1);
    if !matches!(first.as_deref(), Some("list") | Some("launch")) {
        return None;
    }

    let cli = Cli::parse();
    let code = match cli.command {
        Command::List { what } => {
            list(what);
            0
        }
        Command::Launch { game, players } => launch(&game, &players),
    };
    Some(code)
}

fn list(what: ListWhat) {
    match what {
        ListWhat::Games => {
            for h in scan_handlers() {
                println!("{}", h.display());
            }
        }
        ListWhat::Profiles => {
            for p in scan_profiles(true) {
                println!("{p}");
            }
        }
        ListWhat::Inputs => {
            let cfg = load_cfg();
            for (i, d) in scan_input_devices(&cfg.pad_filter_type).iter().enumerate() {
                let kind = match d.device_type() {
                    DeviceType::Gamepad => "gamepad",
                    DeviceType::Keyboard => "keyboard",
                    DeviceType::Mouse => "mouse",
                    DeviceType::Other => "other",
                };
                println!("{i:>2}  {kind:<8}  {}", d.fancyname());
            }
        }
    }
}

/// Headless launch. Resolves the game + per-player specs into the same
/// structures the GUI builds, then runs the shared `run_session`. Returns a
/// process exit code.
fn launch(game: &str, players: &[String]) -> i32 {
    let cfg = load_cfg();

    let Some(handler) = scan_handlers()
        .into_iter()
        .find(|h| h.display().eq_ignore_ascii_case(game))
    else {
        eprintln!("[splitux] game '{game}' not found — run `splitux list games`.");
        return 2;
    };

    let profiles = scan_profiles(true);
    let input_devices = scan_input_devices(&cfg.pad_filter_type);
    let monitors = get_monitors_sdl();
    if monitors.is_empty() {
        eprintln!("[splitux] no monitors detected — a display/output must be active to render the game.");
        return 2;
    }

    let mut instances: Vec<Instance> = Vec::new();
    for spec in players {
        match parse_player(spec, &profiles) {
            Ok(inst) => instances.push(inst),
            Err(e) => {
                eprintln!("[splitux] --player '{spec}': {e}");
                return 2;
            }
        }
    }

    // Local-split (couch co-op) games fold their remote players into ONE game
    // instance owning N seats — do this before sizing so it lays out as a single
    // fullscreen window, not an N-way split. Online/LAN handlers are unchanged.
    let mut instances = crate::together::collapse_for_local_split(instances, &handler);

    // Same pre-launch sizing/naming the GUI does.
    if cfg.gamescope_sdl_backend {
        set_instance_resolutions_multimonitor(&mut instances, &monitors, &cfg);
    } else {
        set_instance_resolutions(&mut instances, &monitors[0], &cfg);
    }
    set_instance_names(&mut instances, &profiles);

    let dev_infos: Vec<DeviceInfo> = input_devices.iter().map(|d| d.info()).collect();
    let master = cfg.master_profile.clone();
    let ready = AtomicBool::new(false);

    let remote: usize = instances.iter().map(|i| i.together_seats as usize).sum();
    println!(
        "[splitux] launching '{}' headlessly — {} player(s), {} remote seat(s). \
         Together invite URLs print below once the game is up.",
        handler.display(),
        instances.len(),
        remote
    );

    // run_session blocks until the game exits, then tears everything down.
    run_session(
        &handler,
        &instances,
        &monitors,
        &dev_infos,
        &cfg,
        master.as_deref(),
        &ready,
        &|title, body| eprintln!("[splitux] {title}: {body}"),
    );
    0
}

/// Parse one `--player key=val,key=val` spec into an Instance. P2 supports
/// `profile=<name>` and `input=together:gamepad|together:kbm`; local-device
/// inputs (pad/kbm) land in P3.
fn parse_player(spec: &str, profiles: &[String]) -> Result<Instance, String> {
    let mut profselection = 0; // Guest
    let mut together = false;
    let mut together_input = TogetherInput::Gamepad;

    for tok in spec.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let (key, val) = tok
            .split_once('=')
            .ok_or_else(|| format!("'{tok}' is not key=val"))?;
        match key.trim() {
            "profile" => {
                profselection = profiles
                    .iter()
                    .position(|p| p.eq_ignore_ascii_case(val.trim()))
                    .ok_or_else(|| format!("profile '{val}' not found (try `splitux list profiles`)"))?;
            }
            "input" => match val.trim() {
                "together:gamepad" => {
                    together = true;
                    together_input = TogetherInput::Gamepad;
                }
                "together:kbm" => {
                    together = true;
                    together_input = TogetherInput::KbMouse;
                }
                other => {
                    return Err(format!(
                        "unsupported input '{other}' (P2 supports together:gamepad or together:kbm)"
                    ))
                }
            },
            other => return Err(format!("unknown key '{other}' (expected profile or input)")),
        }
    }

    Ok(Instance {
        devices: vec![],
        profname: String::new(),
        profselection,
        monitor: 0,
        width: 0,
        height: 0,
        together,
        together_input,
        together_seats: if together { 1 } else { 0 },
    })
}
