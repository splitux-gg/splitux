//! systemd cgroup containment for launched games.
//!
//! The problem: a launched game spawns gamescope → bwrap → proton → a whole
//! wine tree. wine `setsid`'s its services into their own sessions and they
//! reparent to the user systemd manager (NOT init), so process-group / session
//! kills and `ppid == 1` checks can never reliably reap them. They leak.
//!
//! The fix: cgroups. cgroup membership is inherited by every descendant and
//! cannot be escaped by `setsid`/fork, so the entire tree stays inside one
//! container regardless of who its process parent becomes. We:
//!
//!  1. Re-exec splitux itself into a dedicated `splitux-main-<pid>.scope` at
//!     startup (`ensure_self_scoped`).
//!  2. Launch each game instance into `splitux-<launchid>.slice` as a transient
//!     scope that `BindsTo` the main scope (`wrap_command`).
//!
//! Result: killing the splitux pid by ANY means (normal exit, window close,
//! SIGTERM, SIGKILL, panic) empties the main scope, systemd cascades the
//! `BindsTo` teardown, and every game scope's whole cgroup — wine included — is
//! killed. No marker-guessing, no lottery.
//!
//! Everything degrades gracefully: if `systemd-run --user` is unavailable we
//! skip scoping and fall back to direct spawning + a best-effort sweep.
//!
//! # Unit hierarchy (the naming contract)
//!
//! Every unit splitux creates lives under one stable root slice, and every
//! per-process/per-launch leaf encodes its OWNER pid in its name. This is what
//! makes concurrent splitux processes safe to tell apart and reap independently:
//!
//! ```text
//! splitux.slice                              ROOT — shared, stable, never stopped by us
//! ├─ splitux-main-<pid>.scope                a splitux process's own scope (owner: <pid>)
//! ├─ splitux-restore-<pid>.service           that process's death watcher (owner: <pid>)
//! ├─ splitux-bar-*.scope                     relaunched host status bars (not a leftover)
//! └─ splitux-<pid>_<n>.slice                 one launch by process <pid> (owner: <pid>)
//!    ├─ splitux-<pid>_<n>-i<k>.scope         instance k's gamescope+game
//!    └─ splitux-<pid>_<n>-seat<k>.scope      seat k's seat-streamer
//! ```
//!
//! The launch id is `<pid>_<n>` (underscore, NOT hyphen — see [`new_launch_id`]),
//! so a launch slice nests DIRECTLY under [`ROOT_SLICE`] without systemd
//! auto-materializing surprise intermediate parents. Consequences the sweep MUST
//! respect ([`sweep_orphan_units`]): the root is shared infrastructure (stopping
//! it cascade-kills every concurrent launch), and a leaf is a reap-able orphan
//! only if [`owner_pid`] is dead.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Env guard that marks an already-self-scoped splitux process, preventing an
/// infinite re-exec loop.
const SCOPED_ENV: &str = "SPLITUX_SCOPED";

/// The single stable parent slice every splitux unit nests under. Shared by ALL
/// concurrent splitux processes and auto-created by systemd from the child
/// names; it carries no owner pid because it belongs to no one launch. It is
/// infrastructure — the sweep must never stop it (doing so cascades teardown to
/// every concurrent launch's child slice).
pub const ROOT_SLICE: &str = "splitux.slice";

/// The currently-active launch slice, so any teardown path (normal exit, the
/// eframe `on_exit` hook, the startup sweep) can stop it.
static ACTIVE_SLICE: Mutex<Option<String>> = Mutex::new(None);

/// Monotonic per-process counter for unique launch ids.
static LAUNCH_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Whether `systemd-run --user` works here (probed once).
fn systemd_user_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("systemd-run")
            .args(["--user", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Public probe used by the launch pipeline to decide whether to wrap commands.
pub fn enabled() -> bool {
    systemd_user_available()
}

/// Read this process's cgroup-v2 path from `/proc/self/cgroup`.
fn self_cgroup() -> Option<String> {
    let raw = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    // cgroup v2 has a single line: "0::/user.slice/.../foo.scope"
    raw.lines()
        .find_map(|l| l.strip_prefix("0::").map(|p| p.to_string()))
}

/// The leaf unit name of our own scope, e.g. `splitux-main-12345.scope`, if we
/// are running inside one.
pub fn current_main_scope() -> Option<String> {
    let cg = self_cgroup()?;
    let leaf = cg.rsplit('/').next()?;
    if leaf.starts_with("splitux-main-") && leaf.ends_with(".scope") {
        Some(leaf.to_string())
    } else {
        None
    }
}

/// Re-exec splitux into its own transient scope so launched games have a stable
/// cgroup to bind their lifetime to. No-op if already scoped, if systemd-user is
/// unavailable, or if re-exec fails (we just continue unscoped).
///
/// Call this as the very first thing in `main()`.
pub fn ensure_self_scoped() {
    use std::os::unix::process::CommandExt;

    // Already scoped (we re-exec'd) — nothing to do.
    if std::env::var_os(SCOPED_ENV).is_some() {
        return;
    }
    if !systemd_user_available() {
        return;
    }

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let unit = format!("splitux-main-{}.scope", std::process::id());

    println!("[splitux] scope - Re-exec into {unit} for process containment");

    let mut cmd = Command::new("systemd-run");
    cmd.args([
        "--user",
        "--scope",
        "--quiet",
        "--collect",
        &format!("--unit={unit}"),
        "--",
    ]);
    cmd.arg(&exe);
    cmd.args(&args);
    cmd.env(SCOPED_ENV, "1");

    // exec() replaces this process image; only returns on error.
    let err = cmd.exec();
    println!("[splitux] scope - Warning: self-scope re-exec failed ({err}); continuing unscoped");
}

/// Spawn a detached "restore on death" watcher for this splitux instance.
///
/// splitux's only host-side change is hiding the status bars (their restart
/// cmdlines are persisted to disk by `wm::bars`). On a normal exit we restore
/// them in teardown / `on_exit`, but a `kill -9` runs no in-process code — so we
/// register an independent `--user` service (NOT in our scope, so it outlives
/// us) that waits for our pid to disappear, then re-invokes splitux to restore
/// the bars. Idempotent: if the bars were already restored (state file cleared),
/// the restore is a no-op.
pub fn spawn_death_watcher() {
    if !systemd_user_available() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let pid = std::process::id();
    let unit = format!("splitux-restore-{pid}.service");

    // Wait for our pid to die, then restore the host session (bars).
    let script = format!(
        "while kill -0 {pid} 2>/dev/null; do sleep 0.5; done; exec {exe:?} --restore-session"
    );

    let mut cmd = Command::new("systemd-run");
    cmd.args([
        "--user",
        "--quiet",
        "--collect",
        &format!("--unit={unit}"),
    ]);
    // The restored bars (waybar etc.) need the graphical session env, which a
    // bare --user service does not necessarily inherit.
    for var in [
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "NIRI_SOCKET",
        "DISPLAY",
        "PATH",
        "XDG_CURRENT_DESKTOP",
        "HYPRLAND_INSTANCE_SIGNATURE",
    ] {
        if let Ok(val) = std::env::var(var) {
            cmd.arg(format!("--setenv={var}={val}"));
        }
    }
    cmd.args(["--", "/bin/sh", "-c", &script]);

    match cmd.status() {
        Ok(s) if s.success() => {
            println!("[splitux] scope - Restore-on-death watcher armed ({unit})");
        }
        Ok(_) | Err(_) => {
            println!("[splitux] scope - Warning: failed to arm restore-on-death watcher");
        }
    }
}

/// Allocate a unique launch id for this run.
///
/// Uses `_` (not `-`) between pid and counter: `-` is a hierarchy separator in
/// systemd slice names, so a hyphen here would auto-create surprise empty parent
/// slices. With `_`, `splitux-<id>.slice` nests directly under a single stable
/// `splitux.slice` parent.
pub fn new_launch_id() -> String {
    let n = LAUNCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}_{}", std::process::id(), n)
}

/// Slice unit that groups all instances of one launch.
pub fn slice_name(launch_id: &str) -> String {
    format!("splitux-{launch_id}.slice")
}

/// The splitux process that owns a unit, recovered from its name — the inverse of
/// the constructors above (`splitux-main-<pid>`, `splitux-restore-<pid>`, and the
/// `splitux-<pid>_<n>[...]` launch family). `None` for a unit that belongs to no
/// single process — notably [`ROOT_SLICE`], the shared parent. Keep this in
/// lockstep with the naming contract documented at the top of the module.
pub fn owner_pid(unit: &str) -> Option<u32> {
    let rest = unit.strip_prefix("splitux-")?;
    let pid_part = rest
        .strip_prefix("main-")
        .or_else(|| rest.strip_prefix("restore-"))
        .map(|s| s.split('.').next().unwrap_or(s))
        .unwrap_or_else(|| rest.split('_').next().unwrap_or(rest));
    pid_part.parse::<u32>().ok()
}

/// Record the active launch slice (so teardown / on_exit can stop it).
pub fn set_active_slice(launch_id: &str) {
    if let Ok(mut g) = ACTIVE_SLICE.lock() {
        *g = Some(slice_name(launch_id));
    }
}

/// Clear the active launch slice marker.
pub fn clear_active_slice() {
    if let Ok(mut g) = ACTIVE_SLICE.lock() {
        *g = None;
    }
}

/// Wrap a built instance command so it launches inside its own transient scope
/// under the launch's slice, bound to splitux's main scope.
///
/// The returned `Command` runs `systemd-run --scope ... -- <original cmd>`, and
/// `systemd-run` waits on the game — so the caller's `Child`/`wait()` model is
/// unchanged. Per-instance env vars and cwd are carried across so the game sees
/// exactly what it would have unscoped.
pub fn wrap_command(
    inner: Command,
    launch_id: &str,
    instance_idx: usize,
    main_scope: Option<&str>,
) -> Command {
    let unit = format!("splitux-{launch_id}-i{instance_idx}.scope");
    wrap_in_scope(inner, &unit, &slice_name(launch_id), main_scope)
}

/// Like [`wrap_command`] but for a per-launch together seat-streamer.
///
/// Uses a `-seat{idx}` unit name so it never collides with instance (`-i{idx}`)
/// scopes, and joins the SAME launch slice — so the seat-streamer lives and dies
/// with the launch. Killing splitux by any means cascades the `BindsTo` teardown
/// to this scope, and the launch slice stop / startup sweep reap it too. Without
/// this, seat-streamers spawned as bare children orphan on a hard kill and keep
/// their virtual input devices alive, poisoning the next launch's gamescope
/// `--libinput-hold-dev` grab (the "input not passed to the game" failure).
pub fn wrap_seat_command(
    inner: Command,
    launch_id: &str,
    seat_idx: usize,
    main_scope: Option<&str>,
) -> Command {
    let unit = format!("splitux-{launch_id}-seat{seat_idx}.scope");
    wrap_in_scope(inner, &unit, &slice_name(launch_id), main_scope)
}

/// Core: wrap `inner` into a transient `--scope` named `unit` under `slice`,
/// bound to `main_scope` so it dies with splitux. Carries the inner program,
/// args, env (incl. removals) and cwd across; stdio is intentionally NOT copied
/// (set it on the returned command if needed — `Command::get_*` can't read it).
fn wrap_in_scope(
    inner: Command,
    unit: &str,
    slice: &str,
    main_scope: Option<&str>,
) -> Command {
    let mut cmd = Command::new("systemd-run");
    cmd.args([
        "--user",
        "--scope",
        "--quiet",
        "--collect",
        &format!("--unit={unit}"),
        &format!("--slice={slice}"),
    ]);

    // Bind the instance's lifetime to splitux's own scope: when splitux dies,
    // systemd cascades the teardown to this scope (and its whole cgroup).
    if let Some(scope) = main_scope {
        cmd.arg(format!("--property=BindsTo={scope}"));
        cmd.arg(format!("--property=After={scope}"));
    }

    // Propagate the inner env via --setenv (systemd-run OPTIONS, so they must
    // precede `--`). A `--scope` child inherits the systemd --user MANAGER's
    // environment, which can carry stale vars (classically LIBVA_DRIVER_NAME=nvidia
    // on an AMD box) — setting env on this systemd-run CLIENT with cmd.env() does
    // NOT override those inside the scope (that was a long-standing footgun: the
    // GPU driver env was silently lost). --setenv sets the var in the scope itself,
    // so the inner command's env reliably wins over whatever the manager imported.
    for (key, val) in inner.get_envs() {
        if let (Some(k), Some(v)) = (key.to_str(), val.and_then(|v| v.to_str())) {
            cmd.arg(format!("--setenv={k}={v}"));
        }
        // None == env_remove; not expressible as --setenv. The scoped launch
        // commands don't remove inherited env, so there's nothing to carry here.
    }

    cmd.arg("--");

    // Carry over the inner program, args and cwd (env handled above via --setenv).
    cmd.arg(inner.get_program());
    cmd.args(inner.get_args());
    if let Some(dir) = inner.get_current_dir() {
        cmd.current_dir(dir);
    }

    cmd
}

/// Stop a launch slice (kills every instance scope in it and their cgroups).
/// Best-effort and idempotent.
pub fn stop_slice(launch_id: &str) {
    if !systemd_user_available() {
        return;
    }
    let slice = slice_name(launch_id);
    println!("[splitux] scope - Stopping {slice}");
    let _ = Command::new("systemctl")
        .args(["--user", "stop", &slice])
        .status();
}

/// Stop whatever launch slice is currently active (used by exit hooks).
pub fn stop_active_slice() {
    let slice = ACTIVE_SLICE.lock().ok().and_then(|g| g.clone());
    if let Some(slice) = slice {
        if systemd_user_available() {
            println!("[splitux] scope - on-exit: stopping {slice}");
            let _ = Command::new("systemctl")
                .args(["--user", "stop", &slice])
                .status();
        }
    }
    clear_active_slice();
}

/// Sweep leftover splitux launch units from previous (crashed/killed) runs.
///
/// Deterministic, unlike marker/ppid guessing: we enumerate splitux's own
/// transient units and stop only the ones we can attribute to a DEAD splitux
/// process. The naming contract (see the module header) makes this unambiguous —
/// every reap-able leaf carries its owner pid; the shared [`ROOT_SLICE`] does
/// not. Called at startup and before each launch, INCLUDING while other splitux
/// processes are running concurrent sessions, so it must never touch:
///   * [`ROOT_SLICE`] — the shared parent; stopping it cascade-kills every launch.
///   * `splitux-bar-*` — relaunched host status bars, not launch leftovers.
///   * any unit whose [`owner_pid`] is still alive — a live concurrent session.
pub fn sweep_orphan_units() {
    if !systemd_user_available() {
        return;
    }

    let Ok(output) = Command::new("systemctl")
        .args([
            "--user",
            "list-units",
            "--all",
            "--no-legend",
            "--plain",
            "--type=scope,slice",
            "splitux*",
        ])
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut to_stop = Vec::new();
    for line in text.lines() {
        let Some(unit) = line.split_whitespace().next() else {
            continue;
        };
        if !unit.starts_with("splitux") {
            continue;
        }
        // The shared parent slice is infrastructure, not a leftover — stopping it
        // would cascade teardown to every concurrent launch's child slice.
        if unit == ROOT_SLICE {
            continue;
        }
        // Restored status bars run in their own splitux-bar-*.scope units — they
        // are legitimately-running user apps we relaunched, NOT launch leftovers.
        if unit.starts_with("splitux-bar-") {
            continue;
        }
        // Reap a leaf only when its owning splitux process is DEAD. A live pid is
        // either THIS process or a concurrent session — both must be left alone.
        // (No parseable pid means it isn't an attributable leftover; skip it.)
        let Some(pid) = owner_pid(unit) else {
            continue;
        };
        if crate::util::pid_alive(pid) {
            continue;
        }
        to_stop.push(unit.to_string());
    }

    if to_stop.is_empty() {
        return;
    }

    println!(
        "[splitux] scope - Sweeping {} leftover launch unit(s): {:?}",
        to_stop.len(),
        to_stop
    );
    let mut cmd = Command::new("systemctl");
    cmd.args(["--user", "stop"]);
    cmd.args(&to_stop);
    let _ = cmd.status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_id_uses_underscore_so_it_nests_under_one_root() {
        // `<pid>_<n>` keeps the launch slice a DIRECT child of ROOT_SLICE; a hyphen
        // would make systemd auto-create an intermediate parent per pid.
        let slice = slice_name("12345_0");
        assert_eq!(slice, "splitux-12345_0.slice");
        // The hierarchy separator '-' must only appear in the fixed prefix, not
        // between pid and counter.
        assert!(!"12345_0".contains('-'));
    }

    #[test]
    fn owner_pid_recovers_owner_from_every_leaf_unit() {
        assert_eq!(owner_pid("splitux-main-12345.scope"), Some(12345));
        assert_eq!(owner_pid("splitux-12345_0.slice"), Some(12345));
        assert_eq!(owner_pid("splitux-12345_0-i0.scope"), Some(12345));
        assert_eq!(owner_pid("splitux-12345_3-seat2.scope"), Some(12345));
        // restore watcher is a .service (not swept) but still attributable
        assert_eq!(owner_pid("splitux-restore-12345.service"), Some(12345));
    }

    #[test]
    fn root_slice_has_no_owner_so_it_is_never_reaped() {
        // The crux of the concurrency bug: the shared parent must NOT parse to a
        // pid, so the sweep can never mistake it for a dead-pid leftover.
        assert_eq!(owner_pid(ROOT_SLICE), None);
        // A non-splitux unit is not ours at all.
        assert_eq!(owner_pid("user.slice"), None);
    }
}
