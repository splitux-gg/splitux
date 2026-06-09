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

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Env guard that marks an already-self-scoped splitux process, preventing an
/// infinite re-exec loop.
const SCOPED_ENV: &str = "SPLITUX_SCOPED";

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
    let slice = slice_name(launch_id);

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

    cmd.arg("--");

    // Carry over the inner program, args, env (including removals) and cwd.
    cmd.arg(inner.get_program());
    cmd.args(inner.get_args());
    for (key, val) in inner.get_envs() {
        match val {
            Some(v) => {
                cmd.env(key, v);
            }
            None => {
                cmd.env_remove(key);
            }
        }
    }
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
/// transient units and stop any that are NOT our current main scope. Called at
/// startup and before each launch.
pub fn sweep_orphan_units() {
    if !systemd_user_available() {
        return;
    }

    // Units belonging to THIS instance (must never be swept): our main scope,
    // our restore-on-death watcher, and our launch slice(s). All carry our pid.
    let pid = std::process::id();
    let mine: [String; 3] = [
        format!("splitux-main-{pid}."),
        format!("splitux-restore-{pid}."),
        format!("splitux-{pid}_"),
    ];
    let is_mine = |unit: &str| mine.iter().any(|p| unit.starts_with(p.as_str()));

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
        // Restored status bars run in their own splitux-bar-*.scope units — they
        // are legitimately-running user apps we relaunched, NOT launch leftovers.
        if unit.starts_with("splitux-bar-") {
            continue;
        }
        // Never tear down units belonging to this running instance.
        if is_mine(unit) {
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
