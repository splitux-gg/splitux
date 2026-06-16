//! Per-instance network namespaces for bridged LAN co-op (goldberg.bridged_lan).
//!
//! Co-located IP-LAN games (UE IpNetDriver listen servers, goldberg's broadcast
//! LAN discovery) can't co-op when every instance shares one network namespace:
//! they collide on the game port and goldberg hands the joiner 127.0.0.1 for the
//! host. This module gives each instance its own netns + veth into a shared Linux
//! bridge, so each is a distinct LAN host (own `lo`, own IP, own game port). It
//! carries BOTH goldberg's discovery and the game's IP transport.
//!
//! Mirrors `lutris-mp-bench/mp-bench.sh` (NETNS=1): bridge `splitux-br`, subnet
//! `10.77.0.0/24` (host = .1, instance i = .(10+i)), per-instance namespace
//! `splitux-ns<i>` with veth `splitux-h<i>` (host end, on the bridge) /
//! `splitux-n<i>` (ns end). All privileged work is `sudo -n ip ...`
//! (passwordless sudo is required); the game is launched inside the namespace
//! and dropped back to the invoking user via `setpriv`.
//!
//! NOTE: incompatible with the EOS emu's localhost mode (which expects a shared
//! 127.0.0.1) — only enable for goldberg-only IP-LAN games.

use std::error::Error;
use std::process::{Command, Stdio};

/// Shared Linux bridge all instance veths attach to.
const BRIDGE: &str = "splitux-br";
/// `/24` subnet prefix: host = `.1`, instance i = `.(10 + i)`.
const SUBNET: &str = "10.77.0";

/// Network namespace name for instance `i`.
pub fn ns_name(i: usize) -> String {
    format!("splitux-ns{i}")
}

/// Host-side veth name for instance `i` (attached to the bridge).
fn host_veth(i: usize) -> String {
    format!("splitux-h{i}")
}

/// Namespace-side veth name for instance `i` (moved into the netns).
fn ns_veth(i: usize) -> String {
    format!("splitux-n{i}")
}

/// IPv4 address for instance `i`.
fn instance_ip(i: usize) -> String {
    format!("{SUBNET}.{}", 10 + i)
}

/// Run `sudo -n <args>`, erroring (with captured stderr) on non-zero exit.
/// Used for operations whose failure must abort the bridged launch.
fn sudo_checked(args: &[&str]) -> Result<(), Box<dyn Error>> {
    let out = Command::new("sudo").arg("-n").args(args).output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("`sudo -n {}` failed: {}", args.join(" "), stderr.trim()).into());
    }
    Ok(())
}

/// Run `sudo -n <args>` best-effort, swallowing all output and any error.
/// Used for idempotent ("already exists") and stale-cleanup operations.
fn sudo_quiet(args: &[&str]) {
    let _ = Command::new("sudo")
        .arg("-n")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Verify the host can do bridged_lan: passwordless sudo plus `ip` and
/// `setpriv` on PATH. Returns a clear error so a requested bridged launch never
/// silently falls back to an un-isolated (port-colliding) launch.
pub fn preflight() -> Result<(), Box<dyn Error>> {
    let sudo_ok = Command::new("sudo")
        .args(["-n", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !sudo_ok {
        return Err(
            "goldberg.bridged_lan needs passwordless sudo (`sudo -n true` failed) for `ip netns`"
                .into(),
        );
    }
    for bin in ["ip", "setpriv"] {
        let found = Command::new("which")
            .arg(bin)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !found {
            return Err(format!(
                "goldberg.bridged_lan needs `{bin}` on PATH (not found)"
            )
            .into());
        }
    }
    Ok(())
}

/// Create the shared bridge `splitux-br` with `10.77.0.1/24` and bring it up.
/// Idempotent: the add/addr steps are best-effort (ignore "exists"); only the
/// final "up" is checked.
pub fn setup_bridge() -> Result<(), Box<dyn Error>> {
    println!("[splitux] netns: creating bridge {BRIDGE} ({SUBNET}.0/24)");
    // Idempotent — already-present bridge/addr just error out harmlessly.
    sudo_quiet(&["ip", "link", "add", BRIDGE, "type", "bridge"]);
    sudo_quiet(&["ip", "addr", "add", &format!("{SUBNET}.1/24"), "dev", BRIDGE]);
    sudo_checked(&["ip", "link", "set", BRIDGE, "up"])?;
    Ok(())
}

/// Create instance `i`'s namespace + veth pair, attach the host end to the
/// bridge, move the ns end in, address it, bring up `lo`, and add the
/// limited-broadcast route (so goldberg's 255.255.255.255 LAN discovery leaves
/// via the veth and the bridge floods it to the other instances). Cleans any
/// stale state from a previous run first.
pub fn add_instance(i: usize) -> Result<(), Box<dyn Error>> {
    let ns = ns_name(i);
    let hveth = host_veth(i);
    let nveth = ns_veth(i);
    let ip = instance_ip(i);

    // Clean stale state first (best-effort).
    sudo_quiet(&["ip", "netns", "del", &ns]);
    sudo_quiet(&["ip", "link", "del", &hveth]);

    sudo_checked(&["ip", "netns", "add", &ns])?;
    sudo_checked(&["ip", "link", "add", &hveth, "type", "veth", "peer", "name", &nveth])?;
    sudo_checked(&["ip", "link", "set", &hveth, "master", BRIDGE])?;
    sudo_checked(&["ip", "link", "set", &hveth, "up"])?;
    sudo_checked(&["ip", "link", "set", &nveth, "netns", &ns])?;
    sudo_checked(&["ip", "netns", "exec", &ns, "ip", "addr", "add", &format!("{ip}/24"), "dev", &nveth])?;
    sudo_checked(&["ip", "netns", "exec", &ns, "ip", "link", "set", &nveth, "up"])?;
    sudo_checked(&["ip", "netns", "exec", &ns, "ip", "link", "set", "lo", "up"])?;
    // Limited-broadcast route for goldberg LAN discovery; default route optional.
    sudo_quiet(&["ip", "netns", "exec", &ns, "ip", "route", "add", "255.255.255.255/32", "dev", &nveth]);
    sudo_quiet(&["ip", "netns", "exec", &ns, "ip", "route", "add", "default", "via", &format!("{SUBNET}.1")]);

    println!("[splitux] netns {ns}: {ip}/24 via {hveth}@{BRIDGE}");
    Ok(())
}

/// Tear down all `n` instance namespaces + their host veths, then the bridge.
/// Best-effort and idempotent (safe to call after a partial setup).
pub fn teardown(n: usize) {
    println!("[splitux] netns: tearing down {n} namespace(s) + bridge {BRIDGE}");
    for i in 0..n {
        sudo_quiet(&["ip", "netns", "del", &ns_name(i)]);
        sudo_quiet(&["ip", "link", "del", &host_veth(i)]);
    }
    sudo_quiet(&["ip", "link", "del", BRIDGE]);
}

/// Wrap a fully-built instance command so it runs inside instance `i`'s network
/// namespace as the invoking user:
///
///   sudo -n ip netns exec <ns> setpriv --reuid=<uid> --regid=<gid> \
///       --init-groups env K=V ... <inner program> <inner args>
///
/// Because `sudo` resets the environment, every env var the inner Command
/// carried (set via `cmd.env`) is re-emitted explicitly through `env K=V`.
/// Removed vars (`env_remove`, i.e. `None` values) are simply not emitted.
///
/// Display/runtime env that splitux relies on being INHERITED (DISPLAY,
/// XDG_RUNTIME_DIR, ...) is NOT on the inner Command, so it would be lost across
/// sudo — we additionally pull it from splitux's own process environment and
/// append it. These only touch DRM/display/audio over UNIX sockets, which the
/// network namespace does not isolate, so they are safe to carry in. A name
/// already present on the inner Command is left to the inner Command (we don't
/// clobber an explicit value with the inherited one).
pub fn wrap_command_in_netns(inner: Command, i: usize) -> Command {
    let ns = ns_name(i);
    // SAFETY: getuid/getgid are always-successful, side-effect-free syscalls.
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };

    let mut cmd = Command::new("sudo");
    cmd.arg("-n")
        .args(["ip", "netns", "exec"])
        .arg(&ns)
        .arg("setpriv")
        .arg(format!("--reuid={uid}"))
        .arg(format!("--regid={gid}"))
        .arg("--init-groups")
        .arg("env");

    // Collect inner env keys so we don't override an explicit value with the
    // inherited passthrough below.
    let inner_keys: std::collections::HashSet<std::ffi::OsString> =
        inner.get_envs().map(|(k, _)| k.to_os_string()).collect();

    // Re-emit the inner Command's env (sudo strips it). Skip removed vars.
    for (key, val) in inner.get_envs() {
        if let Some(v) = val {
            let mut kv = key.to_os_string();
            kv.push("=");
            kv.push(v);
            cmd.arg(kv);
        }
    }

    // Inherited display/runtime/audio env splitux never sets via cmd.env() —
    // safe across a network-only namespace (UNIX-socket transports).
    const PASSTHROUGH: [&str; 8] = [
        "DISPLAY",
        "XDG_RUNTIME_DIR",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "PULSE_SERVER",
        "DBUS_SESSION_BUS_ADDRESS",
        "HOME",
        "PATH",
    ];
    for name in PASSTHROUGH {
        if inner_keys.contains(std::ffi::OsStr::new(name)) {
            continue;
        }
        if let Ok(v) = std::env::var(name) {
            cmd.arg(format!("{name}={v}"));
        }
    }

    // Inner program + its args.
    cmd.arg(inner.get_program());
    cmd.args(inner.get_args());

    // Preserve the working directory.
    if let Some(dir) = inner.get_current_dir() {
        cmd.current_dir(dir);
    }

    cmd
}
