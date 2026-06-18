//! splitux-together integration: stream launched instances to remote browsers.
//!
//! When `cfg.together.enabled`, every launched instance gets a `seat-streamer`
//! sidecar. The streamer owns a virtual gamepad/keyboard/mouse (which splitux
//! feeds to that instance exactly like a local controller — gamescope holds the
//! kbd/mouse, the game's SDL reads the pad) and captures the instance's
//! gamescope output over PipeWire, H.264-encoding it to a remote browser over
//! WebRTC. splitux pops up one single-URL invite (`{base}/j/{token}`) per seat;
//! the host hands each link to a friend, who opens it and auto-joins that seat.
//!
//! The lifecycle mirrors `gptokeyb`: seats are set up BEFORE command building
//! (so their virtual device nodes exist for gamescope's `--libinput-hold-dev`),
//! their device paths are threaded into `launch_cmds`, and they're torn down
//! with the session.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::app::SplituxConfig;
use crate::instance::Instance;
use crate::paths::{BIN_ORCHESTRATOR, BIN_SEAT_STREAMER, PATH_PARTY};

/// The three virtual input device nodes a seat-streamer creates for one seat.
/// `kbd`/`mouse` are handed to gamescope (`--libinput-hold-dev`); `pad` is fed
/// to the game's SDL via `SDL_JOYSTICK_DEVICE`.
#[derive(Clone, Debug, Default)]
pub struct TogetherSeatDevices {
    pub pad: Option<PathBuf>,
    pub kbd: Option<PathBuf>,
    pub mouse: Option<PathBuf>,
}

/// One shareable invite: the seat it joins and the single URL that carries it.
#[derive(Clone, Debug)]
pub struct InviteLink {
    /// Seat id the link joins (e.g. "seat-1"); kept for logging/diagnostics.
    #[allow(dead_code)]
    pub seat: String,
    pub name: String,
    pub url: String,
}

/// Stable seat id for instance index `i` (0-based) → "seat-1", "seat-2", …
fn seat_id(i: usize) -> String {
    format!("seat-{}", i + 1)
}

/// PipeWire node name for instance `i`'s gamescope capture. Each together
/// instance must advertise a UNIQUE node name, otherwise a seat-streamer
/// matching by name binds every seat to the first gamescope node (all seats
/// capture one instance). gamescope reads this from `GAMESCOPE_PIPEWIRE_NODE`
/// (set on its launch in build_cmds) and the matching seat-streamer targets it
/// via `--pw-name`. Keep the two in lockstep through this one helper.
pub fn node_name_for_instance(i: usize) -> String {
    format!("gamescope-{}", seat_id(i))
}

/// A ~22-char URL-safe random invite token.
fn gen_token() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    (0..22).map(|_| ALPHABET[fastrand::usize(..ALPHABET.len())] as char).collect()
}

/// Build the single-URL invite: `{public_base_url}/j/{token}` (no trailing
/// slash duplication).
fn build_invite_url(cfg: &SplituxConfig, token: &str) -> String {
    let base = cfg.together.public_base_url.trim_end_matches('/');
    format!("{base}/j/{token}")
}

/// Scan `/proc/bus/input/devices` for this seat's three virtual nodes by name.
/// seat-streamer names them "splitux-together <seat>" (pad), "<seat> kbd",
/// "<seat> mouse" — the exact, proven lookup the bench uses. Retries until all
/// three appear or `timeout` elapses (the devices are created at streamer
/// startup, a beat after spawn).
fn wait_for_seat_devices(seat: &str, timeout: Duration) -> TogetherSeatDevices {
    let pad_name = format!("splitux-together {seat}");
    let kbd_name = format!("splitux-together {seat} kbd");
    let mouse_name = format!("splitux-together {seat} mouse");

    let deadline = Instant::now() + timeout;
    loop {
        let mut devs = TogetherSeatDevices::default();
        if let Ok(text) = std::fs::read_to_string("/proc/bus/input/devices") {
            // Blocks are separated by blank lines; within a block `N: Name="…"`
            // names the device and `H: Handlers=… eventN …` carries its node.
            for block in text.split("\n\n") {
                let mut name = None;
                let mut event = None;
                for line in block.lines() {
                    if let Some(rest) = line.strip_prefix("N: Name=") {
                        name = Some(rest.trim().trim_matches('"').to_string());
                    } else if let Some(rest) = line.strip_prefix("H: Handlers=") {
                        event = rest.split_whitespace().find(|t| t.starts_with("event")).map(String::from);
                    }
                }
                let (Some(name), Some(event)) = (name, event) else { continue };
                let path = PathBuf::from("/dev/input").join(&event);
                // Order matters: the "kbd"/"mouse" suffixes must be checked
                // before the bare pad name (which is a prefix of both).
                if name == kbd_name {
                    devs.kbd = Some(path);
                } else if name == mouse_name {
                    devs.mouse = Some(path);
                } else if name == pad_name {
                    devs.pad = Some(path);
                }
            }
        }
        if devs.pad.is_some() && devs.kbd.is_some() && devs.mouse.is_some() {
            return devs;
        }
        if Instant::now() >= deadline {
            // Return whatever we found; the caller warns. A partial set still
            // lets some input through and makes the misconfig visible.
            return devs;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Spawn one seat-streamer for `seat`, announcing `token` so `/j/<token>`
/// auto-joins it. Output goes to `/tmp/splitux-together-<seat>.log`.
fn spawn_seat_streamer(
    cfg: &SplituxConfig,
    seat: &str,
    name: &str,
    token: &str,
    instance: &Instance,
    pw_node: &str,
    launch_id: &str,
    seat_idx: usize,
    main_scope: Option<&str>,
    scoping: bool,
) -> std::io::Result<Child> {
    let log_path = format!("/tmp/splitux-together-{seat}.log");
    let log = std::fs::File::create(&log_path)?;
    let log_err = log.try_clone()?;

    let mut cmd = Command::new(BIN_SEAT_STREAMER.as_path());
    cmd.args(["--seat", seat])
        .args(["--name", name])
        .args(["--invite-token", token])
        .args(["--signalling", &cfg.together.signalling_uri])
        .args(["--source", "pipewire"])
        // Target this seat's HOST INSTANCE gamescope node. Must match the
        // GAMESCOPE_PIPEWIRE_NODE build_cmds sets on that gamescope launch. In
        // online/LAN each seat has its own instance (seat-N → instance-N node);
        // in local-split every seat shares instance-0's node (multi-consumer).
        .args(["--pw-name", pw_node])
        .args(["--encoder", &cfg.together.encoder])
        .args(["--bitrate", &cfg.together.bitrate.to_string()])
        .args(["--stun", &cfg.together.stun]);
    cmd.args(["--fps", &cfg.together.resolved_fps().to_string()]);
    if instance.width > 0 && instance.height > 0 {
        cmd.args(["--width", &instance.width.to_string()]);
        cmd.args(["--height", &instance.height.to_string()]);
    }
    if let Some(turn) = &cfg.together.turn {
        cmd.args(["--turn", turn]);
    }
    // FORCE radeonsi: this box's session can carry a stale LIBVA_DRIVER_NAME=nvidia
    // that silently kills the gst `va` plugin (AMD card). The bench forces it too.
    cmd.env("LIBVA_DRIVER_NAME", "radeonsi");
    cmd.env("RUST_LOG", "info");

    // The Vulkan zero-copy encoder needs the custom `dmabufvulkanupload` plugin
    // (libgstdmabufvulkan.so) — it imports gamescope's NV12 dmabuf straight into
    // the encoder (no per-frame copy), which is what lets the HW encoder sustain
    // the full fps tier instead of bottlenecking ~128fps on the upload. It lives
    // in splitux's data dir, NOT a default GStreamer scan path, so without this
    // the seat-streamer fails with `no element "dmabufvulkanupload"` and the
    // session zero-videos. Append to any inherited path so a bench override wins.
    let plugin_dir = PATH_PARTY.join("gst-plugins");
    let gst_plugin_path = match std::env::var("GST_PLUGIN_PATH") {
        Ok(existing) if !existing.is_empty() => format!("{}:{}", plugin_dir.display(), existing),
        _ => plugin_dir.display().to_string(),
    };
    cmd.env("GST_PLUGIN_PATH", gst_plugin_path);

    // Pass through the CQP quality knobs for the vulkan-zerocopy encoder so the
    // QP point can be swept live (RADV's vbr/cbr ignore the bitrate target; CQP
    // is the only working lever). The systemd-run scope wrapper does not inherit
    // arbitrary env, so forward them explicitly when present.
    for k in ["GSE_CQP_QP_I", "GSE_CQP_QP_P"] {
        if let Ok(v) = std::env::var(k) {
            cmd.env(k, v);
        }
    }

    // Scope the streamer into the per-launch slice so it shares the launch
    // lifecycle — it dies with the launch (slice teardown + BindsTo cascade +
    // startup sweep all reap it), instead of orphaning as a bare child and
    // keeping its virtual input devices alive to poison the next launch's
    // gamescope --libinput-hold-dev grab. stdio is applied AFTER the wrap because
    // wrap_seat_command copies program/args/env/cwd but not stdio.
    let mut cmd = if scoping {
        crate::launch::scope::wrap_seat_command(cmd, launch_id, seat_idx, main_scope)
    } else {
        cmd
    };
    cmd.stdout(Stdio::from(log));
    cmd.stderr(Stdio::from(log_err));

    println!("[splitux] together - seat {seat}: spawning seat-streamer (log: {log_path})");
    cmd.spawn()
}

/// Best-effort local orchestrator. When `spawn_local_orchestrator` is set and
/// nothing is already serving the signalling host, start the bundled
/// orchestrator. Non-fatal: if the binary or web dir is missing we warn and
/// assume `signalling_uri` points at a running service.
fn ensure_orchestrator(cfg: &SplituxConfig) -> Option<Child> {
    if !cfg.together.spawn_local_orchestrator {
        return None;
    }
    let bin = BIN_ORCHESTRATOR.as_path();
    if !bin.exists() {
        println!(
            "[splitux] together - spawn_local_orchestrator set but no orchestrator binary at {} — \
             assuming {} is a running service",
            bin.display(),
            cfg.together.signalling_uri
        );
        return None;
    }
    let web = PATH_PARTY.join("together/web");
    let mut cmd = Command::new(bin);
    cmd.args(["--bind", "0.0.0.0:8080"]);
    if web.is_dir() {
        cmd.arg("--web").arg(&web);
    }
    if let Ok(log) = std::fs::File::create("/tmp/splitux-together-orchestrator.log") {
        if let Ok(err) = log.try_clone() {
            cmd.stdout(Stdio::from(log)).stderr(Stdio::from(err));
        }
    }
    match cmd.spawn() {
        Ok(child) => {
            println!("[splitux] together - started local orchestrator (web: {})", web.display());
            std::thread::sleep(Duration::from_millis(800)); // let it bind before seats dial in
            Some(child)
        }
        Err(e) => {
            println!("[splitux] together - failed to start local orchestrator: {e}");
            None
        }
    }
}

/// Set up remote seats for the session, mirroring `setup_gptokeyb_daemons`.
///
/// Returns, in instance order:
///   * `seat_handles`   — the seat-streamer child per instance (+ a trailing
///                        local-orchestrator handle, if we started one)
///   * `seat_devices`   — the virtual pad/kbd/mouse paths to thread into
///                        `launch_cmds` (None for instances without a seat)
///   * `invite_links`   — the URLs to pop up once windows are up
///
/// No-op (all-None) when `together.enabled` is false, so normal local
/// splitscreen is completely unaffected.
pub fn setup_together_seats(
    instances: &[Instance],
    cfg: &SplituxConfig,
    game_label: &str,
    launch_id: &str,
    main_scope: Option<&str>,
    scoping: bool,
) -> (Vec<Child>, Vec<Vec<TogetherSeatDevices>>, Vec<InviteLink>) {
    let n = instances.len();
    // Total remote seats across all instances. Normally one per `together`
    // instance (online/LAN); a local-split game folds N seats onto one instance.
    let remote_count: usize = instances.iter().map(|inst| inst.together_seats as usize).sum();
    if remote_count == 0 {
        return (Vec::new(), vec![Vec::new(); n], Vec::new());
    }
    if !BIN_SEAT_STREAMER.exists() {
        println!(
            "[splitux] together - a player is marked remote but the seat-streamer binary is not at {} — \
             skipping remote seats",
            BIN_SEAT_STREAMER.display()
        );
        return (Vec::new(), vec![Vec::new(); n], Vec::new());
    }
    if !cfg.input_holding {
        println!(
            "[splitux] together - WARNING: input holding is OFF, so gamescope-splitux (the capture \
             source) won't be used and remote seats can't capture or receive kbd/mouse. Enable \
             input holding for together."
        );
    }

    println!("[splitux] together - setting up {remote_count} remote seat(s) → {}", cfg.together.signalling_uri);

    let mut handles: Vec<Child> = Vec::new();
    let mut devices: Vec<Vec<TogetherSeatDevices>> = vec![Vec::new(); n];
    let mut links: Vec<InviteLink> = Vec::new();

    if let Some(orch) = ensure_orchestrator(cfg) {
        handles.push(orch);
    }

    // Seat ids are allocated from one global counter so every seat is unique even
    // when several share an instance (local-split). The PipeWire node, in
    // contrast, is keyed by the HOST INSTANCE: all of an instance's seats capture
    // its one gamescope (multi-consumer), so they target the same node name that
    // build_cmds stamps onto that gamescope via GAMESCOPE_PIPEWIRE_NODE.
    let mut seat_index = 0usize;
    for (i, instance) in instances.iter().enumerate() {
        if instance.together_seats == 0 {
            continue; // local player — untouched
        }
        let node = node_name_for_instance(i);
        for _ in 0..instance.together_seats {
            let seat = seat_id(seat_index);
            let name = if remote_count == 1 {
                game_label.to_string()
            } else {
                format!("{game_label} — Player {}", seat_index + 1)
            };
            let token = gen_token();

            match spawn_seat_streamer(
                cfg, &seat, &name, &token, instance, &node,
                launch_id, seat_index, main_scope, scoping,
            ) {
                Ok(child) => handles.push(child),
                Err(e) => {
                    println!("[splitux] together - seat {seat}: spawn failed: {e}");
                    seat_index += 1;
                    continue;
                }
            }

            // The virtual devices appear a beat after the streamer starts;
            // gamescope must hold them at launch, so block until they exist.
            let devs = wait_for_seat_devices(&seat, Duration::from_secs(10));
            if devs.kbd.is_none() || devs.mouse.is_none() || devs.pad.is_none() {
                println!(
                    "[splitux] together - seat {seat}: virtual devices incomplete (pad={:?} kbd={:?} mouse={:?}) — \
                     check /tmp/splitux-together-{seat}.log",
                    devs.pad, devs.kbd, devs.mouse
                );
            } else {
                println!(
                    "[splitux] together - seat {seat}: pad={} kbd={} mouse={}",
                    devs.pad.as_ref().unwrap().display(),
                    devs.kbd.as_ref().unwrap().display(),
                    devs.mouse.as_ref().unwrap().display()
                );
            }
            devices[i].push(devs);

            links.push(InviteLink { seat, name, url: build_invite_url(cfg, &token) });
            seat_index += 1;
        }
    }

    (handles, devices, links)
}

/// Fold a local-split handler's together players into a single game instance
/// that owns all their remote seats (one game process, N browsers). Online/LAN
/// handlers (the default) are returned unchanged — every player keeps its own
/// instance. Local (non-together) pads are merged onto the shared instance so a
/// host can sit at the same couch game.
pub fn collapse_for_local_split(
    instances: Vec<Instance>,
    handler: &crate::handler::Handler,
) -> Vec<Instance> {
    if !handler.is_local_split() || instances.len() <= 1 {
        return instances;
    }
    let seats: u32 = instances
        .iter()
        .filter(|inst| inst.together)
        .map(|inst| inst.together_seats.max(1) as u32)
        .sum();
    // Base the shared instance on the first player (its profile owns the single
    // overlay/prefix), but gather every player's local input devices onto it.
    let mut base = instances[0].clone();
    base.devices = instances.iter().flat_map(|inst| inst.devices.iter().copied()).collect();
    base.together = seats > 0;
    base.together_seats = seats.min(u8::MAX as u32) as u8;
    if base.together {
        // Local-split is gamepad-only (games lock kb/m to P1); every seat's pad
        // is wired into the one game's SDL device list.
        base.together_input = crate::instance::TogetherInput::Gamepad;
    }
    vec![base]
}

/// Show the invite URLs to the host so they can hand them to friends. Also
/// prints them and writes them to a file (the dialog text isn't easily
/// copyable). Runs the dialog on a detached thread so it never blocks session
/// supervision.
pub fn popup_invites(links: &[InviteLink]) {
    if links.is_empty() {
        return;
    }

    let mut body = String::from("splitux together — share one link per remote player:\n\n");
    for l in links {
        body.push_str(&format!("• {}\n  {}\n", l.name, l.url));
    }
    println!("[splitux] together - invites:\n{body}");

    // Persist so the host can copy/paste even after dismissing the dialog.
    let file = PATH_PARTY.join("together-invites.txt");
    if let Err(e) = std::fs::write(&file, &body) {
        println!("[splitux] together - couldn't write {}: {e}", file.display());
    } else {
        println!("[splitux] together - invites also written to {}", file.display());
    }

    let title = "splitux together — invite links";
    std::thread::spawn(move || {
        rfd::MessageDialog::new()
            .set_title(title)
            .set_description(&body)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    });
}

/// Terminate all seat-streamer (and local-orchestrator) children at teardown.
pub fn terminate_all(handles: &mut Vec<Child>) {
    for mut child in handles.drain(..) {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{CoopMode, Handler};
    use crate::instance::{Instance, TogetherInput};

    fn together_instance(devices: Vec<usize>) -> Instance {
        Instance {
            devices,
            profname: String::new(),
            profselection: 0,
            monitor: 0,
            width: 0,
            height: 0,
            together: true,
            together_input: TogetherInput::Gamepad,
            together_seats: 1,
        }
    }

    fn handler_with(coop: CoopMode) -> Handler {
        Handler { coop_mode: coop, ..Handler::default() }
    }

    #[test]
    fn separate_handler_is_left_untouched() {
        let instances = vec![together_instance(vec![]), together_instance(vec![])];
        let out = collapse_for_local_split(instances.clone(), &handler_with(CoopMode::Separate));
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|i| i.together_seats == 1));
    }

    #[test]
    fn local_split_folds_players_into_one_instance() {
        let instances = vec![together_instance(vec![]), together_instance(vec![])];
        let out = collapse_for_local_split(instances, &handler_with(CoopMode::LocalSplit));
        assert_eq!(out.len(), 1, "two players collapse to one game instance");
        assert_eq!(out[0].together_seats, 2, "the one instance owns both seats");
        assert!(out[0].together);
        assert_eq!(out[0].together_input, TogetherInput::Gamepad);
    }

    #[test]
    fn local_split_merges_local_pads_onto_the_instance() {
        // A host local pad (instance with devices [3], not together) plus two
        // remote seats should yield one instance carrying the local pad and 2
        // remote seats.
        let mut local = together_instance(vec![3]);
        local.together = false;
        local.together_seats = 0;
        let instances = vec![local, together_instance(vec![]), together_instance(vec![])];
        let out = collapse_for_local_split(instances, &handler_with(CoopMode::LocalSplit));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].together_seats, 2, "only together players become seats");
        assert!(out[0].devices.contains(&3), "host's local pad is preserved");
    }

    #[test]
    fn single_player_is_not_collapsed() {
        let instances = vec![together_instance(vec![])];
        let out = collapse_for_local_split(instances, &handler_with(CoopMode::LocalSplit));
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn node_name_is_keyed_per_instance() {
        // All seats on one instance must resolve to that instance's node so they
        // share its single gamescope capture (multi-consumer fan-out).
        assert_eq!(node_name_for_instance(0), "gamescope-seat-1");
        assert_eq!(node_name_for_instance(1), "gamescope-seat-2");
    }
}
