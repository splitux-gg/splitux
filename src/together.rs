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
use std::sync::LazyLock;
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
    /// Absolute pointer (QEMU-tablet-style ABS_X/Y) for touch clients driving
    /// mouse-only games. Held by gamescope alongside kbd/mouse.
    pub ptr: Option<PathBuf>,
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

/// Per-launch namespace so CONCURRENT splitux sessions don't collide. Seat ids
/// AND pipewire node names both derive from `seat_id`, so without this a second
/// session reusing `seat-1`/`seat-2` would (a) overwrite the first session's
/// orchestrator registration and (b) clash on the `gamescope-seat-1` pipewire
/// node. One value per launch PROCESS — both the gamescope node (build_cmds) and
/// the seat-streamer args (--seat/--pw-name) are computed in this same process, so
/// they stay in lockstep; the spawned seat-streamer receives the resolved strings
/// as args and never recomputes them.
static SESSION_NS: LazyLock<String> = LazyLock::new(|| {
    const A: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    (0..5).map(|_| A[fastrand::usize(..A.len())] as char).collect()
});

/// Seat id for instance index `i` (0-based) → "<ns>-seat-1", "<ns>-seat-2", …
/// The `<ns>` prefix is unique per launch (see [`SESSION_NS`]).
fn seat_id(i: usize) -> String {
    format!("{}-seat-{}", &*SESSION_NS, i + 1)
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
/// Parse the seat-streamer's `--device-report` file (`label=/dev/input/eventN`
/// lines). Returns the seat's devices only once ALL four are present. This is the
/// authoritative source AND the only way to learn the raw-gadget pad node (named
/// "Microsoft X-Box 360 pad", which /proc name-matching for "splitux-together
/// <seat>" can never find — leaving the pad unwired and un-barriered).
fn read_device_report(seat: &str) -> Option<TogetherSeatDevices> {
    let path = format!("/tmp/splitux-together-{seat}.devices");
    let text = std::fs::read_to_string(&path).ok()?;
    let mut devs = TogetherSeatDevices::default();
    for line in text.lines() {
        let Some((label, p)) = line.split_once('=') else { continue };
        let p = PathBuf::from(p.trim());
        match label.trim() {
            "pad" => devs.pad = Some(p),
            "kbd" => devs.kbd = Some(p),
            "mouse" => devs.mouse = Some(p),
            "ptr" => devs.ptr = Some(p),
            _ => {}
        }
    }
    if devs.pad.is_some() && devs.kbd.is_some() && devs.mouse.is_some() && devs.ptr.is_some() {
        Some(devs)
    } else {
        None
    }
}

fn wait_for_seat_devices(seat: &str, timeout: Duration) -> TogetherSeatDevices {
    let pad_name = format!("splitux-together {seat}");
    let kbd_name = format!("splitux-together {seat} kbd");
    let mouse_name = format!("splitux-together {seat} mouse");
    let ptr_name = format!("splitux-together {seat} ptr");

    let deadline = Instant::now() + timeout;
    loop {
        // Preferred: the seat-streamer's own device-report (authoritative; the only
        // way to find the raw-gadget pad). Fall through to /proc name-matching if
        // it's absent/incomplete (uinput pads, or an older seat-streamer).
        if let Some(devs) = read_device_report(seat) {
            return devs;
        }
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
                // Order matters: the "kbd"/"mouse"/"ptr" suffixes must be checked
                // before the bare pad name (which is a prefix of all of them).
                if name == kbd_name {
                    devs.kbd = Some(path);
                } else if name == mouse_name {
                    devs.mouse = Some(path);
                } else if name == ptr_name {
                    devs.ptr = Some(path);
                } else if name == pad_name {
                    devs.pad = Some(path);
                }
            }
        }
        if devs.pad.is_some() && devs.kbd.is_some() && devs.mouse.is_some() && devs.ptr.is_some() {
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
/// Resolve the default sink's monitor source (e.g.
/// "alsa_output.pci-….analog-stereo.monitor") for use as `pulsesrc device=`.
/// Works on PulseAudio and pipewire-pulse. None if pactl is unavailable.
fn default_sink_monitor() -> Option<String> {
    let out = Command::new("pactl").arg("get-default-sink").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let sink = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if sink.is_empty() {
        return None;
    }
    Some(format!("{sink}.monitor"))
}

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
    audio_monitor: Option<&str>,
) -> std::io::Result<Child> {
    let log_path = format!("/tmp/splitux-together-{seat}.log");
    let log = std::fs::File::create(&log_path)?;
    let log_err = log.try_clone()?;

    // seat-streamer writes its resolved device nodes here once created; we read it
    // in wait_for_seat_devices to learn the REAL pad node (the raw-gadget pad isn't
    // named "splitux-together <seat>", so /proc name-matching can't find it). Clear
    // any stale file from a prior run so we never read a dead seat's paths.
    let device_report = format!("/tmp/splitux-together-{seat}.devices");
    let _ = std::fs::remove_file(&device_report);

    let mut cmd = Command::new(BIN_SEAT_STREAMER.as_path());
    cmd.args(["--seat", seat])
        .args(["--device-report", &device_report])
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
    // Audio passthrough: stream the game's sound as an Opus track. Each together
    // instance has its OWN capture sink (a per-launch null sink the game is routed
    // to via PULSE_SINK), so we capture THAT sink's monitor — isolating this
    // instance's audio to this seat's stream instead of every seat tapping the one
    // shared default-sink monitor (which mixed all games into all streams). Falls
    // back to the default sink's monitor if no per-instance sink was created.
    let audio_device = audio_monitor
        .map(str::to_string)
        .or_else(default_sink_monitor);
    if let Some(mon) = audio_device {
        cmd.args(["--audio-device", &mon]);
    }
    if instance.width > 0 && instance.height > 0 {
        cmd.args(["--width", &instance.width.to_string()]);
        cmd.args(["--height", &instance.height.to_string()]);
    }
    if let Some(turn) = &cfg.together.turn {
        cmd.args(["--turn", turn]);
    }
    // Align the seat-streamer's HW video encoder with the configured GPU vendor
    // (gpu_vendor=auto detects from the DRM render node). Replaces a hardcoded
    // LIBVA_DRIVER_NAME=radeonsi: a stale/foreign LIBVA driver (e.g. a session
    // carrying LIBVA_DRIVER_NAME=nvidia on an AMD card) silently kills the gst
    // `va` plugin, and the vulkan encoder path wants the matching driver too.
    for (k, v) in cfg.gpu_vendor.driver_env() {
        cmd.env(k, v);
    }
    // Honor an inherited RUST_LOG so a debug trace of the seat-streamer (pad
    // lifecycle, frame streaming) can be turned on from the parent env, e.g.
    // `RUST_LOG=debug splitux launch ...`; default to "info" when unset.
    cmd.env(
        "RUST_LOG",
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
    );

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

    // Pass through the vulkan-zerocopy encoder tuning knobs so they can be swept
    // live (RADV's vbr/cbr ignore the bitrate target; CQP is the only rate lever,
    // and quality/b-frames are the EFFICIENCY levers that cut bitrate at the same
    // quality+fps). The scope wrapper forwards these via --setenv.
    for k in [
        "GSE_CQP_QP_I",
        "GSE_CQP_QP_P",
        "GSE_ENC_QUALITY",
        "GSE_BFRAMES",
        "GSE_H264_PROFILE",
        "GSE_CODEC",
        "GSE_FORCE_FPS",
        "GSE_PAD_DEBUG",
    ] {
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
        crate::launch::scope::wrap_seat_command(cmd, launch_id, instance.game, seat_idx, main_scope)
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
    if let Ok(log) = std::fs::File::create("/tmp/splitux-together-orchestrator.log")
        && let Ok(err) = log.try_clone() {
            cmd.stdout(Stdio::from(log)).stderr(Stdio::from(err));
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
///     local-orchestrator handle, if we started one)
///   * `seat_devices`   — the virtual pad/kbd/mouse paths to thread into
///     `launch_cmds` (None for instances without a seat)
///   * `invite_links`   — the URLs to pop up once windows are up
///
/// No-op (all-None) when `together.enabled` is false, so normal local
/// splitscreen is completely unaffected.
pub fn setup_together_seats(
    instances: &[Instance],
    cfg: &SplituxConfig,
    // One label per game (indexed by `Instance.game`), so a multi-game launch
    // gives each seat its own game's name on the invite. Single-game = length-1.
    game_labels: &[String],
    launch_id: &str,
    main_scope: Option<&str>,
    scoping: bool,
    audio_sink_envs: &[String],
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
        // This instance's audio capture source. setup_audio_routing gives every
        // together instance its own null sink (PULSE_SINK = that sink), so capture
        // "<sink>.monitor"; all of this instance's seats share it (one game = one
        // audio stream). Empty env → no per-instance sink, so spawn_seat_streamer
        // falls back to the default-sink monitor.
        let audio_monitor = audio_sink_envs
            .get(i)
            .filter(|s| !s.is_empty())
            .map(|s| format!("{s}.monitor"));
        // This seat's invite carries its OWN game's label (multi-game safe).
        let game_label = game_labels
            .get(instance.game)
            .map(String::as_str)
            .unwrap_or("");
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
                audio_monitor.as_deref(),
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
            if devs.kbd.is_none() || devs.mouse.is_none() || devs.pad.is_none() || devs.ptr.is_none() {
                println!(
                    "[splitux] together - seat {seat}: virtual devices incomplete (pad={:?} kbd={:?} mouse={:?} ptr={:?}) — \
                     check /tmp/splitux-together-{seat}.log",
                    devs.pad, devs.kbd, devs.mouse, devs.ptr
                );
            } else {
                println!(
                    "[splitux] together - seat {seat}: pad={} kbd={} mouse={} ptr={}",
                    devs.pad.as_ref().unwrap().display(),
                    devs.kbd.as_ref().unwrap().display(),
                    devs.mouse.as_ref().unwrap().display(),
                    devs.ptr.as_ref().unwrap().display()
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
    // A local (non-together) player among the folded set means the host plays
    // this shared instance directly (kb/m or pad via window focus). Capture it
    // BEFORE `base.together` is overwritten below — once the seats are folded in,
    // gamescope holds their virtual devices and would otherwise block all parent
    // compositor input, locking the host out. The flag re-opens that path.
    let has_local_player = instances.iter().any(|inst| !inst.together);
    // Base the shared instance on the first player (its profile owns the single
    // overlay/prefix), but gather every player's local input devices onto it.
    let mut base = instances[0].clone();
    base.devices = instances.iter().flat_map(|inst| inst.devices.iter().copied()).collect();
    base.together = seats > 0;
    base.together_seats = seats.min(u8::MAX as u32) as u8;
    base.local_input = has_local_player;
    if base.together {
        // Local-split is gamepad-only (games lock kb/m to P1); every seat's pad
        // is wired into the one game's SDL device list.
        base.together_input = crate::instance::TogetherInput::Gamepad;
    }
    vec![base]
}

/// Collapse each game's instances independently for local-split (couch co-op).
///
/// Multi-game safe wrapper over [`collapse_for_local_split`]: groups instances by
/// `Instance.game` (first-seen order), folds each game with ITS own handler, and
/// preserves the game tag on the folded instance. A single-game launch is one
/// group → the legacy single `collapse_for_local_split` call, byte-identical.
///
/// This is the FIRST step of the shared launch-core (see
/// [`crate::launch::run_launch`]) so every presentation layer collapses the same
/// way — previously only the CLI did, leaving the GUI's local-split path
/// divergent.
pub fn collapse_instances_per_game(
    instances: Vec<Instance>,
    handlers: &[crate::handler::Handler],
) -> Vec<Instance> {
    use std::collections::HashMap;
    let mut order: Vec<usize> = Vec::new();
    let mut by_game: HashMap<usize, Vec<Instance>> = HashMap::new();
    for inst in instances {
        by_game
            .entry(inst.game)
            .or_insert_with(|| {
                order.push(inst.game);
                Vec::new()
            })
            .push(inst);
    }
    let mut out: Vec<Instance> = Vec::new();
    for game in order {
        let group = by_game.remove(&game).unwrap_or_default();
        // Guard against an out-of-range game tag (shouldn't happen post-parse).
        if let Some(handler) = handlers.get(game) {
            out.extend(collapse_for_local_split(group, handler));
        } else {
            out.extend(group);
        }
    }
    out
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
///
/// SIGTERM, NOT SIGKILL: seat-streamer installs a SIGTERM handler that runs its
/// graceful raw-gadget shutdown (SIGTERM xpad360 -> EP_DISABLE -> join the ep-io
/// threads -> close the raw-gadget fd). That clean release is what avoids the
/// dummy_hcd use-after-free oops (dummy_timer completing a freed in-flight
/// request -> HW-watchdog hard reset). A blunt `child.kill()` (SIGKILL) skips the
/// handler entirely — the exact abrupt death of a raw_gadget holder that wedges
/// the kernel gadget layer. We SIGTERM all of them first (parallel graceful
/// teardown), wait against a shared deadline, then SIGKILL only as a backstop.
pub fn terminate_all(handles: &mut Vec<Child>) {
    use std::time::{Duration, Instant};
    for child in handles.iter_mut() {
        unsafe {
            libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
        }
    }
    // Generous deadline: each seat's xpad360 now joins its kernel ep-io threads on
    // the way out (up to ~2s per pad), and seats tear down in parallel.
    let deadline = Instant::now() + Duration::from_millis(4000);
    for mut child in handles.drain(..) {
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill(); // SIGKILL backstop for a stuck streamer
                    let _ = child.wait();
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => break,
            }
        }
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
            game: 0,
            profname: String::new(),
            profselection: 0,
            monitor: 0,
            width: 0,
            height: 0,
            together: true,
            together_input: TogetherInput::Gamepad,
            together_seats: 1,
            local_input: false,
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
        // Per-instance node names must differ (so seats bind their own gamescope),
        // share the one per-launch namespace prefix, and end with the stable
        // `-seat-N` suffix that pairs gamescope (build_cmds) with the seat-streamer.
        let n0 = node_name_for_instance(0);
        let n1 = node_name_for_instance(1);
        assert_ne!(n0, n1);
        assert!(n0.starts_with("gamescope-") && n0.ends_with("-seat-1"), "{n0}");
        assert!(n1.ends_with("-seat-2"), "{n1}");
        // Same namespace for both instances of this launch.
        let ns0 = n0.trim_start_matches("gamescope-").trim_end_matches("-seat-1");
        let ns1 = n1.trim_start_matches("gamescope-").trim_end_matches("-seat-2");
        assert_eq!(ns0, ns1);
        assert!(!ns0.is_empty());
    }
}
