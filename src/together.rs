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
        .args(["--pw-name", "gamescope"])
        .args(["--encoder", &cfg.together.encoder])
        .args(["--bitrate", &cfg.together.bitrate.to_string()])
        .args(["--stun", &cfg.together.stun]);
    if cfg.together.fps > 0 {
        cmd.args(["--fps", &cfg.together.fps.to_string()]);
    }
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
) -> (Vec<Child>, Vec<Option<TogetherSeatDevices>>, Vec<InviteLink>) {
    let n = instances.len();
    let remote_count = instances.iter().filter(|inst| inst.together).count();
    if remote_count == 0 {
        return (Vec::new(), vec![None; n], Vec::new());
    }
    if !BIN_SEAT_STREAMER.exists() {
        println!(
            "[splitux] together - a player is marked remote but the seat-streamer binary is not at {} — \
             skipping remote seats",
            BIN_SEAT_STREAMER.display()
        );
        return (Vec::new(), vec![None; n], Vec::new());
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
    let mut devices: Vec<Option<TogetherSeatDevices>> = vec![None; n];
    let mut links: Vec<InviteLink> = Vec::new();

    if let Some(orch) = ensure_orchestrator(cfg) {
        handles.push(orch);
    }

    for (i, instance) in instances.iter().enumerate() {
        if !instance.together {
            continue; // local player — untouched
        }
        let seat = seat_id(i);
        let name = if remote_count == 1 {
            game_label.to_string()
        } else {
            format!("{game_label} — Player {}", i + 1)
        };
        let token = gen_token();

        match spawn_seat_streamer(cfg, &seat, &name, &token, instance) {
            Ok(child) => handles.push(child),
            Err(e) => {
                println!("[splitux] together - seat {seat}: spawn failed: {e}");
                continue;
            }
        }

        // The virtual devices appear a beat after the streamer starts; gamescope
        // must hold them at launch, so block until they exist (bounded).
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
        devices[i] = Some(devs);

        links.push(InviteLink { seat, name, url: build_invite_url(cfg, &token) });
    }

    (handles, devices, links)
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
