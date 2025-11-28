//! Proton/Wine environment setup
//!
//! This module handles configuring environment variables and paths for running
//! Windows games through Proton/Wine.

use std::path::PathBuf;
use std::process::Command;

use crate::app::PartyConfig;
use crate::handler::Handler;
use crate::paths::{PATH_PARTY, PATH_STEAM, BIN_UMU_RUN};
use crate::util::{get_steam_compat_data_path, resolve_proton_path};

/// Get the Wine prefix path for an instance
pub fn get_prefix_path(cfg: &PartyConfig, instance_idx: usize) -> PathBuf {
    PATH_PARTY.join("prefixes").join(match cfg.proton_separate_pfxs {
        true => (instance_idx + 1).to_string(),
        false => "1".to_string(),
    })
}

/// Set up Proton environment variables on a command
///
/// This sets all the necessary environment variables for Proton to work correctly
/// outside of Steam, including WINEPREFIX, STEAM_COMPAT_DATA_PATH, etc.
pub fn setup_env(
    cmd: &mut Command,
    handler: &Handler,
    cfg: &PartyConfig,
    instance_idx: usize,
) {
    let path_pfx = get_prefix_path(cfg, instance_idx);

    // Proton version to use
    let protonpath = match cfg.proton_version.is_empty() {
        true => "GE-Proton",
        false => &cfg.proton_version,
    };

    // Core Proton environment
    cmd.env("WINEPREFIX", &path_pfx);
    cmd.env("PROTON_VERB", "waitforexitandrun");
    cmd.env("PROTONPATH", protonpath);

    // Steam compatibility paths
    // Use Steam's compatdata path if available for this game, otherwise use splitux's prefix parent
    let compat_data_path = handler
        .steam_appid
        .and_then(get_steam_compat_data_path)
        .unwrap_or_else(|| path_pfx.parent().unwrap_or(&path_pfx).to_path_buf());
    cmd.env("STEAM_COMPAT_DATA_PATH", &compat_data_path);
    cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &*PATH_STEAM);

    // Steam App IDs (required for some games/Proton features)
    if let Some(appid) = handler.steam_appid {
        cmd.env("SteamAppId", appid.to_string());
        cmd.env("SteamGameId", appid.to_string());
    }
}

/// Get the Proton binary path
///
/// If proton_path is set in the handler, resolve it to a full path.
/// Otherwise, returns the umu-run path for automatic Proton management.
pub fn get_binary(handler: &Handler) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !handler.proton_path.is_empty() {
        if let Some(proton_bin) = resolve_proton_path(&handler.proton_path) {
            Ok(proton_bin)
        } else {
            Err(format!(
                "Proton not found at '{}'. Check proton_path in handler.",
                handler.proton_path
            )
            .into())
        }
    } else {
        Ok(BIN_UMU_RUN.clone())
    }
}

/// Check if using direct Proton invocation (vs umu-run)
///
/// When using direct Proton, we need to add "waitforexitandrun" as an argument.
/// When using umu-run, it handles this internally.
pub fn uses_direct_proton(handler: &Handler) -> bool {
    !handler.proton_path.is_empty()
}

/// Get the Wine prefix user directory path for binding profile data
pub fn get_prefix_user_path(cfg: &PartyConfig, instance_idx: usize) -> PathBuf {
    get_prefix_path(cfg, instance_idx).join("drive_c/users/steamuser")
}
