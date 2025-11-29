use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;

pub static PATH_RES: LazyLock<PathBuf> = LazyLock::new(|| {
    let localinstall = PathBuf::from("/usr/share/splitux");
    if localinstall.exists() {
        return localinstall;
    }
    env::current_exe().unwrap().parent().unwrap().join("res")
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
    let bin_candidates = [PathBuf::from("/usr/bin"), PathBuf::from("/usr/local/bin")];

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
    let bin_candidates = [PathBuf::from("/usr/bin"), PathBuf::from("/usr/local/bin")];

    for candidate in &bin_candidates {
        let bin = candidate.join("gamescope-splitux");
        if bin.exists() {
            return bin;
        }
    }

    let bin = env::current_exe().unwrap().parent().unwrap().join("bin");
    bin.join("gamescope-splitux")
});
