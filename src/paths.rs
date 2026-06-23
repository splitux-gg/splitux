use std::env;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

pub static PATH_ASSETS: LazyLock<PathBuf> = LazyLock::new(|| {
    // Check system-wide install
    let systeminstall = PathBuf::from("/usr/share/splitux");
    if systeminstall.exists() {
        return systeminstall;
    }
    // Check user-local install (~/.local/share/splitux)
    if let Ok(home) = env::var("HOME") {
        let localinstall = PathBuf::from(home).join(".local/share/splitux");
        if localinstall.exists() {
            return localinstall;
        }
    }
    // Fallback to relative assets directory (for development)
    env::current_exe().unwrap().parent().unwrap().join("assets")
});

pub static PATH_HOME: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env::var("HOME").unwrap()));

pub static PATH_LOCAL_SHARE: LazyLock<PathBuf> = LazyLock::new(|| PATH_HOME.join(".local/share"));

pub static PATH_PARTY: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Ok(xdg_data_home) = env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg_data_home).join("splitux");
    }
    PATH_LOCAL_SHARE.join("splitux")
});

/// Per-process active-launch namespace. Each `splitux launch` runs as its own
/// process with ONE active launch, so all per-launch scratch (overlay mounts,
/// goldberg overlays, work dirs) lives under `tmp/<ns>` keyed by this — letting
/// several splitux processes run CONCURRENT sessions on one host without
/// colliding on `tmp/game-0` etc. Set once at launch start (launch::execute).
static LAUNCH_NS: Mutex<Option<String>> = Mutex::new(None);

/// Set the active launch namespace for this process (the launch_id).
pub fn set_launch_ns(ns: &str) {
    if let Ok(mut g) = LAUNCH_NS.lock() {
        *g = Some(ns.to_string());
    }
}

/// The active launch namespace, or a pid fallback if unset (defensive — the
/// launch flow always sets it before any scratch is created).
pub fn launch_ns() -> String {
    LAUNCH_NS
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| std::process::id().to_string())
}

/// Per-launch scratch root: `PATH_PARTY/tmp/<launch_ns>`. Holds every per-instance
/// launch scratch dir (`game-{i}`, `work-{i}`, `<backend>-overlay-{i}`,
/// `game-patches`) so concurrent launches never share a path.
pub fn launch_tmp_dir() -> PathBuf {
    PATH_PARTY.join("tmp").join(launch_ns())
}

pub static PATH_STEAM: LazyLock<PathBuf> = LazyLock::new(|| {
    // Check for native Steam installation first
    if PATH_LOCAL_SHARE.join("Steam").exists() {
        PATH_LOCAL_SHARE.join("Steam")
    } else if PATH_HOME.join(".steam/steam").exists() {
        // Follow the symlink at ~/.steam/steam
        PATH_HOME.join(".steam/steam")
    } else if PATH_HOME
        .join(".var/app/com.valvesoftware.Steam/.local/share/Steam")
        .exists()
    {
        // Flatpak Steam
        PATH_HOME.join(".var/app/com.valvesoftware.Steam/.local/share/Steam")
    } else {
        PATH_LOCAL_SHARE.join("Steam")
    }
});

pub static BIN_UMU_RUN: LazyLock<PathBuf> = LazyLock::new(|| {
    let bin_candidates = [
        PATH_HOME.join(".local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/usr/local/bin"),
    ];

    for candidate in &bin_candidates {
        let bin = candidate.join("umu-run");
        if bin.exists() {
            return bin;
        }
    }

    let bin = env::current_exe().unwrap().parent().unwrap().join("bin");
    bin.join("umu-run")
});

pub static BIN_GSC_SPLITUX: LazyLock<PathBuf> = LazyLock::new(|| {
    let bin_candidates = [
        PATH_HOME.join(".local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/usr/local/bin"),
    ];

    for candidate in &bin_candidates {
        let bin = candidate.join("gamescope-splitux");
        if bin.exists() {
            return bin;
        }
    }

    let bin = env::current_exe().unwrap().parent().unwrap().join("bin");
    bin.join("gamescope-splitux")
});

pub static BIN_GPTOKEYB: LazyLock<PathBuf> = LazyLock::new(|| {
    let bin_candidates = [
        PATH_HOME.join(".local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/usr/local/bin"),
    ];

    for candidate in &bin_candidates {
        let bin = candidate.join("gptokeyb");
        if bin.exists() {
            return bin;
        }
    }

    let bin = env::current_exe().unwrap().parent().unwrap().join("bin");
    bin.join("gptokeyb")
});

/// splitux-together seat-streamer: one sidecar per remote seat. Owns the
/// virtual gamepad/keyboard/mouse and runs the WebRTC encode pipeline.
pub static BIN_SEAT_STREAMER: LazyLock<PathBuf> = LazyLock::new(|| resolve_companion_bin("seat-streamer"));

/// splitux-together orchestrator: signalling broker + web host. Optional —
/// only spawned when together runs its orchestrator locally rather than
/// pointing at a remote service.
pub static BIN_ORCHESTRATOR: LazyLock<PathBuf> = LazyLock::new(|| resolve_companion_bin("orchestrator"));

/// Resolve a companion binary by name across the standard install locations,
/// preferring splitux's own data dir (`~/.local/share/splitux/bin`).
fn resolve_companion_bin(name: &str) -> PathBuf {
    let candidates = [
        PATH_PARTY.join("bin"),
        PATH_HOME.join(".local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    for candidate in &candidates {
        let bin = candidate.join(name);
        if bin.exists() {
            return bin;
        }
    }
    env::current_exe().unwrap().parent().unwrap().join("bin").join(name)
}
