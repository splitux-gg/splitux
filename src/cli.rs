//! Headless command-line interface for splitux.
//!
//! Lets a full session be discovered, inspected, and launched without the egui
//! GUI, so a remote-play "together" session or a couch split can be stood up
//! from a script or over SSH. The CLI reuses the exact scanning + launch
//! pipeline the GUI drives — it only replaces the interactive *assembly* of the
//! session with flags, and surfaces the same layout / monitor / input config as
//! overrides (`--layout`, `--display`) plus `list` inspectors so you can see the
//! machine's games, profiles, input devices, monitors, and layout presets before
//! committing to a launch.
//!
//! `run_if_cli()` is called from `main()` before the GUI is created: if the
//! first argument names a subcommand we handle it and return an exit code;
//! otherwise we return `None` and `main()` falls through to the GUI.

use std::sync::atomic::AtomicBool;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

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
use crate::session_store::{self, SaveAnchor, SavedPlayer, StoredInput};
use crate::wm::presets::get_presets_for_count;

const LAUNCH_AFTER_HELP: &str = "\
EXAMPLES:
    # Two players, side-by-side split on the primary display
    splitux launch --game Satisfactory \\
        --player profile=Gabe,input=local:kbm \\
        --player profile=Ruth,input=local:gamepad \\
        --layout vertical

    # One player per monitor, each rendered fullscreen
    splitux launch --game Palworld \\
        --player profile=Alice,input=local:kbm \\
        --player profile=Bob,input=local:gamepad \\
        --layout fullscreen --display DP-1 --display HDMI-A-1

    # Inspect the machine before launching
    splitux list monitors   # connectors + resolutions
    splitux list layouts    # valid layout presets per player count
    splitux list inputs     # gamepads / keyboards / mice
    splitux list profiles   # player profiles
    splitux list games      # installed game handlers
";

#[derive(Parser)]
#[command(
    name = "splitux",
    about = "Split-screen / remote-play game launcher",
    long_about = "Splitux runs several instances of a game in a split-screen or multi-monitor \
                  layout, each driven by its own input device or streamed to a remote player. \
                  Run with no subcommand to open the GUI; use the subcommands below to inspect \
                  the system and launch sessions headlessly (scriptable / over SSH)."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Inspect the machine: games, profiles, input devices, monitors, layouts.
    List {
        /// What to enumerate (games | profiles | inputs | monitors | layouts).
        #[arg(value_enum)]
        what: ListWhat,
    },
    /// Launch a full session headlessly (no GUI).
    #[command(after_help = LAUNCH_AFTER_HELP)]
    Launch {
        /// Game name (case-insensitive), e.g. "Satisfactory". See `list games`.
        #[arg(long)]
        game: String,
        /// One player per flag, repeatable. Comma-separated `key=val`:
        ///   profile=<name>   profile to run as (default: Guest; see `list profiles`)
        ///   input=<spec>     local:kbm | local:gamepad | together:kbm | together:gamepad
        /// e.g. --player profile=Gabe,input=local:kbm
        #[arg(long = "player", value_name = "SPEC", required = true)]
        players: Vec<String>,
        /// Window layout, overriding settings.json for this launch only:
        ///   vertical    side-by-side columns
        ///   horizontal  stacked rows
        ///   grid        2x2 (4 players)
        ///   fullscreen  each instance at full monitor resolution
        /// Which presets are valid depends on player count — see `list layouts`.
        #[arg(long, value_enum)]
        layout: Option<Layout>,
        /// Display(s) to render on, by connector name (see `list monitors`).
        /// Repeatable. A single value splits all instances on that one monitor;
        /// multiple values place instances across those monitors (1:1 when the
        /// counts match, otherwise round-robin).
        /// e.g. --display DP-3   or   --display DP-1 --display HDMI-A-1
        #[arg(long = "display", value_name = "CONNECTOR")]
        displays: Vec<String>,

        // --- Save anchoring (carry real progress in, sync it back) ---
        /// Profile that owns the canonical ("anchored") save — the master. The
        /// master is seeded from the original save at start and (with
        /// --save-sync-back) written back at the end. Overrides settings.json.
        #[arg(long)]
        master: Option<String>,
        /// Absolute path to the original save to anchor for this launch (overrides
        /// the handler's original_save_path). Copied into the master at start.
        #[arg(long = "save-anchor", value_name = "PATH")]
        save_anchor: Option<String>,
        /// Sync the master profile's saves back to the anchored location after the
        /// session ends. The original is always backed up first (hard-gated).
        #[arg(long = "save-sync-back")]
        save_sync_back: bool,
        /// Remap Steam IDs embedded in save filenames (DRG-style) when copying
        /// to/from profiles.
        #[arg(long = "save-steam-id-remap")]
        save_steam_id_remap: bool,
    },
    /// Save a session as a reusable, pinned template (no launch). Records the
    /// same Session entry the GUI/TUI reads from `sessions.json`, so a template
    /// created here shows up in the GUI session list immediately. Templates are
    /// device-agnostic: a player's input records only kbm/gamepad + local/together
    /// (any `local:<io>:<idx>` device index in a spec is dropped).
    SaveSession {
        /// Game name (case-insensitive), e.g. "Satisfactory". See `list games`.
        #[arg(long)]
        game: String,
        /// One player per flag, repeatable. Same spec syntax as `launch`:
        ///   profile=<name>,input=<spec>  where input is
        ///   local:kbm | local:gamepad | together:kbm | together:gamepad
        ///   (a trailing device index like local:gamepad:3 is accepted but
        ///   dropped — templates store no device).
        #[arg(long = "player", value_name = "SPEC", required = true)]
        players: Vec<String>,
        /// Profile that owns the canonical ("anchored") save — the master. Sets
        /// the session's save anchor (master_profile).
        #[arg(long)]
        master: Option<String>,
        /// Enable sync-back on the anchor (only meaningful with --master).
        #[arg(long = "save-sync-back")]
        save_sync_back: bool,
        /// Override the auto-generated template name.
        #[arg(long)]
        name: Option<String>,
    },
    /// Interactive terminal UI: pick a game, assign profiles/inputs, launch, and
    /// watch / kill / restart running sessions (a keyboard-driven GUI replacement).
    Tui,
    /// Print a shell completion script (bash, zsh, fish, elvish, powershell).
    Completions {
        /// Target shell.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(ValueEnum, Clone, Copy)]
enum ListWhat {
    /// Installed game handlers.
    Games,
    /// Player profiles.
    Profiles,
    /// Connected input devices (gamepads / keyboards / mice).
    Inputs,
    /// Active monitors (connector name + resolution).
    Monitors,
    /// Layout presets available for each player count.
    Layouts,
}

#[derive(ValueEnum, Clone, Copy)]
enum Layout {
    /// Side-by-side columns.
    Vertical,
    /// Stacked rows.
    Horizontal,
    /// 2x2 grid (4 players).
    Grid,
    /// Each instance at full monitor resolution.
    Fullscreen,
}

impl Layout {
    /// Map a friendly layout choice to the preset id for `count` players, e.g.
    /// (Vertical, 2) -> "2p_vertical". Validity per count is checked by caller.
    fn preset_id(self, count: usize) -> String {
        let mode = match self {
            Layout::Vertical => "vertical",
            Layout::Horizontal => "horizontal",
            Layout::Grid => "grid",
            Layout::Fullscreen => "fullscreen",
        };
        format!("{count}p_{mode}")
    }
}

/// Run the CLI if a subcommand was given. Returns `Some(exit_code)` when the CLI
/// handled the invocation, or `None` when there's no recognized subcommand and
/// `main()` should launch the GUI instead.
pub fn run_if_cli() -> Option<i32> {
    // Engage clap only when argv[1] is one of our subcommands, so a plain
    // `splitux` (or one launched with stray toolkit args) still opens the GUI.
    let first = std::env::args().nth(1);
    if !matches!(
        first.as_deref(),
        Some("list") | Some("launch") | Some("save-session") | Some("completions") | Some("tui")
    ) {
        return None;
    }

    let cli = Cli::parse();
    let code = match cli.command {
        Command::List { what } => {
            list(what);
            0
        }
        Command::Launch {
            game,
            players,
            layout,
            displays,
            master,
            save_anchor,
            save_sync_back,
            save_steam_id_remap,
        } => launch(
            &game,
            &players,
            layout,
            &displays,
            master,
            save_anchor,
            save_sync_back,
            save_steam_id_remap,
        ),
        Command::SaveSession {
            game,
            players,
            master,
            save_sync_back,
            name,
        } => save_session(&game, &players, master, save_sync_back, name),
        Command::Tui => crate::tui::run(),
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "splitux", &mut std::io::stdout());
            0
        }
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
        ListWhat::Monitors => {
            let monitors = get_monitors_sdl();
            if monitors.is_empty() {
                println!("(no monitors detected)");
            }
            for (i, m) in monitors.iter().enumerate() {
                println!(
                    "{i:>2}  {:<12}  {}x{}",
                    m.connector_name(),
                    m.width(),
                    m.height()
                );
            }
        }
        ListWhat::Layouts => {
            for count in 2..=4usize {
                println!("{count} players:");
                for p in get_presets_for_count(count) {
                    println!("    {:<14}  {}", p.id, p.name);
                }
            }
        }
    }
}

/// Headless launch. Resolves the game + per-player specs into the same
/// structures the GUI builds, applies any `--layout`/`--display` overrides, then
/// runs the shared `run_session`. Returns a process exit code.
#[allow(clippy::too_many_arguments)]
fn launch(
    game: &str,
    players: &[String],
    layout: Option<Layout>,
    displays: &[String],
    master_override: Option<String>,
    save_anchor: Option<String>,
    save_sync_back: bool,
    save_steam_id_remap: bool,
) -> i32 {
    let mut cfg = load_cfg();

    let Some(mut handler) = scan_handlers()
        .into_iter()
        .find(|h| h.display().eq_ignore_ascii_case(game))
    else {
        eprintln!("[splitux] game '{game}' not found — run `splitux list games`.");
        return 2;
    };

    // Per-launch save-anchor overrides (set by the TUI when a Session anchors a
    // real save). These flip the handler's save-sync fields for this run only.
    if let Some(path) = save_anchor {
        handler.original_save_path = path;
    }
    if save_sync_back {
        handler.save_sync_back = true;
    }
    if save_steam_id_remap {
        handler.save_steam_id_remap = true;
    }

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

    // --display: pin instances onto chosen monitor(s) by connector name. A single
    // display splits all instances on that screen; multiple displays distribute
    // them (1:1 when counts match, else round-robin).
    let mut use_multimonitor = cfg.gamescope_sdl_backend;
    if !displays.is_empty() {
        let mut idxs = Vec::with_capacity(displays.len());
        for d in displays {
            match monitors.iter().position(|m| {
                m.connector_name().eq_ignore_ascii_case(d) || m.name().eq_ignore_ascii_case(d)
            }) {
                Some(i) => idxs.push(i),
                None => {
                    eprintln!("[splitux] display '{d}' not found — run `splitux list monitors`.");
                    return 2;
                }
            }
        }
        for (i, inst) in instances.iter_mut().enumerate() {
            inst.monitor = idxs[i % idxs.len()];
        }
        use_multimonitor = true;
    }

    // --layout: override the layout preset for this player count.
    if let Some(layout) = layout {
        let count = instances.len();
        let preset_id = layout.preset_id(count);
        if get_presets_for_count(count)
            .iter()
            .any(|p| p.id == preset_id.as_str())
        {
            cfg.layout_presets.set_for_count(count, preset_id);
        } else {
            eprintln!(
                "[splitux] layout '{preset_id}' is not valid for {count} player(s) — run `splitux list layouts`."
            );
            return 2;
        }
    }

    // Same pre-launch sizing/naming the GUI does.
    if use_multimonitor {
        set_instance_resolutions_multimonitor(&mut instances, &monitors, &cfg);
    } else {
        set_instance_resolutions(&mut instances, &monitors[0], &cfg);
    }
    set_instance_names(&mut instances, &profiles);

    let dev_infos: Vec<DeviceInfo> = input_devices.iter().map(|d| d.info()).collect();
    // --master overrides settings.json's master_profile for this launch.
    let master = master_override.or_else(|| cfg.master_profile.clone());
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

/// Save a session as a pinned, reusable template without launching it. Resolves
/// the game + per-player specs the same way `launch` does (same handler scan +
/// `parse_player`), maps each Instance to a `SavedPlayer`, then upserts the entry
/// into `sessions.json` so the GUI/TUI pick it up immediately. Returns an exit
/// code.
fn save_session(
    game: &str,
    players: &[String],
    master: Option<String>,
    save_sync_back: bool,
    name: Option<String>,
) -> i32 {
    // Validate the game against installed handlers (same lookup as `launch`).
    if !scan_handlers()
        .into_iter()
        .any(|h| h.display().eq_ignore_ascii_case(game))
    {
        eprintln!("[splitux] game '{game}' not found — run `splitux list games`.");
        return 2;
    }

    let profiles = scan_profiles(true);

    // Parse each player spec into an Instance (errors on unknown profile / bad
    // input spec), then project it down to the device-agnostic SavedPlayer the
    // session schema stores.
    let mut saved_players: Vec<SavedPlayer> = Vec::new();
    for spec in players {
        let inst = match parse_player(spec, &profiles) {
            Ok(inst) => inst,
            Err(e) => {
                eprintln!("[splitux] --player '{spec}': {e}");
                return 2;
            }
        };
        let input = match inst.together_input {
            TogetherInput::Gamepad => StoredInput::Gamepad,
            TogetherInput::KbMouse => StoredInput::KbMouse,
        };
        saved_players.push(SavedPlayer {
            profile: profiles[inst.profselection].clone(),
            input,
            together: inst.together,
        });
    }

    // --master must be one of the players (it anchors a player's save).
    if let Some(m) = &master {
        if !saved_players.iter().any(|p| p.profile.eq_ignore_ascii_case(m)) {
            eprintln!("[splitux] master '{m}' is not one of the players.");
            return 2;
        }
    }

    // Upsert by (game, profile-set) — same identity the GUI/TUI dedup on — then
    // pin it and apply name/anchor overrides on the resolved entry.
    let mut sessions = session_store::load();
    let id = session_store::upsert(&mut sessions, game, saved_players);
    if let Some(entry) = sessions.iter_mut().find(|s| s.id == id) {
        entry.pinned = true;
        if let Some(n) = &name {
            entry.name = n.clone();
        }
        if let Some(m) = master {
            entry.anchor = Some(SaveAnchor {
                enabled: save_sync_back,
                master_profile: m,
                save_path: String::new(),
                steam_id_remap: false,
            });
        }
        let label = entry.name.clone();
        session_store::save(&sessions);
        println!("[splitux] saved template '{label}' (id: {id}).");
    } else {
        // Should be unreachable: upsert guarantees the id exists.
        eprintln!("[splitux] internal error: saved session '{id}' vanished.");
        return 1;
    }
    0
}

/// Parse one `--player key=val,key=val` spec into an Instance. P2 supports
/// `profile=<name>` and `input=together:gamepad|together:kbm`; local-device
/// inputs (pad/kbm) land in P3.
fn parse_player(spec: &str, profiles: &[String]) -> Result<Instance, String> {
    let mut profselection = 0; // Guest
    let mut together = false;
    let mut together_input = TogetherInput::Gamepad;
    // Physical input devices pinned to this local player (indices into
    // `splitux list inputs`). Empty unless a `local:<io>:<idx>` spec is given.
    let mut devices: Vec<usize> = vec![];

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
                // Local (non-together) players: rendered as a visible nested
                // gamescope window on the host's own display, driven by the host's
                // physical kb/m+pad (focus-based) — no seat-streamer, no virtual
                // input. The CLI is general splitux; together is just one routing.
                // Used to isolate the together streaming/virtual-input layer from
                // the splitux base (bwrap+gamescope+overlay) when debugging.
                // Local specs may carry an optional device index from
                // `splitux list inputs`: `local:kbm:2` / `local:gamepad:2`.
                other => {
                    let mut parts = other.split(':');
                    match parts.next() {
                        Some("local") => {
                            together = false;
                            // io: defaults to kbm when bare `local`.
                            together_input = match parts.next() {
                                None | Some("kbm") => TogetherInput::KbMouse,
                                Some("gamepad") => TogetherInput::Gamepad,
                                Some(io) => {
                                    return Err(format!(
                                        "unsupported local input io '{io}' (want kbm or gamepad)"
                                    ))
                                }
                            };
                            // Optional trailing device index.
                            if let Some(idx_str) = parts.next() {
                                let idx: usize = idx_str.parse().map_err(|_| {
                                    format!(
                                        "invalid device index '{idx_str}' in '{other}' (want a non-negative integer from `splitux list inputs`)"
                                    )
                                })?;
                                devices = vec![idx];
                            }
                            if parts.next().is_some() {
                                return Err(format!(
                                    "malformed input '{other}' (want local, local:kbm, local:gamepad, local:kbm:<idx>, local:gamepad:<idx>)"
                                ));
                            }
                        }
                        _ => {
                            return Err(format!(
                                "unsupported input '{other}' (supported: together:gamepad, together:kbm, local:kbm, local:gamepad, local:kbm:<idx>, local:gamepad:<idx>)"
                            ))
                        }
                    }
                }
            },
            other => return Err(format!("unknown key '{other}' (expected profile or input)")),
        }
    }

    Ok(Instance {
        devices,
        profname: String::new(),
        profselection,
        monitor: 0,
        width: 0,
        height: 0,
        together,
        together_input,
        together_seats: if together { 1 } else { 0 },
        // A standalone local player is its own instance with no held seats, so
        // gamescope never blocks parent input — only a collapsed local-split
        // instance needs this set, which collapse_for_local_split handles.
        local_input: false,
    })
}
