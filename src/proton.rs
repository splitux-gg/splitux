//! Proton/Wine environment setup
//!
//! This module handles configuring environment variables and paths for running
//! Windows games through Proton/Wine.

use std::path::PathBuf;
use std::process::Command;

use crate::app::SplituxConfig;
use crate::handler::Handler;
use crate::paths::{PATH_PARTY, PATH_STEAM, BIN_UMU_RUN};
use crate::util::resolve_proton_path;

/// Get the Wine prefix path for an instance
pub fn get_prefix_path(cfg: &SplituxConfig, instance_idx: usize) -> PathBuf {
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
    cfg: &SplituxConfig,
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
    // Always use splitux's prefix for STEAM_COMPAT_DATA_PATH to avoid conflicts
    // between multiple instances (Proton locks files in this directory)
    cmd.env("STEAM_COMPAT_DATA_PATH", &path_pfx);
    cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &*PATH_STEAM);

    // Steam App IDs (required for some games/Proton features)
    if let Some(appid) = handler.get_steam_appid() {
        cmd.env("SteamAppId", appid.to_string());
        cmd.env("SteamGameId", appid.to_string());

        // umu-run REQUIRES a GAMEID and, when unset, defaults it to "umu-default"
        // — from which it derives STEAM_COMPAT_APP_ID/SteamAppId/SteamGameId =
        // "default" (umu_run.py: SteamAppId = the substring after "umu-"). That
        // bogus appid is what actually reaches the game inside pressure-vessel
        // (umu overwrites the SteamAppId we set above). steam_api goldberg games
        // shrug it off — their DLL reads the appid from steam_settings/steam_appid.txt
        // — but a goldberg STEAMCLIENT game (and its Steamworks wrapper) takes the
        // appid from the env, gets "default", fails Steam init and pops a modal
        // "Steam Error" dialog. Setting GAMEID=umu-<appid> makes umu derive the
        // real appid. Scope it to steamclient games to leave the validated
        // steam_api games' umu behaviour (protonfix lookup) untouched.
        let wants_steamclient = handler
            .goldberg_ref()
            .map(|g| g.steamclient)
            .unwrap_or(false);
        if wants_steamclient {
            cmd.env("GAMEID", format!("umu-{appid}"));

            // Force the game to use goldberg's WINDOWS steamclient64.dll (deployed
            // into the prefix), NOT Proton's native bridge. By default Proton's
            // builtin lsteamclient.dll bridges every steam interface to the host's
            // native real-Steam steamclient.so ({Steam}/linux64/steamclient.so) —
            // so goldberg's steamclient, though loaded, is never called and the
            // game errors (verified via the `+steamclient` WINEDEBUG channel).
            // Disabling lsteamclient makes the game load goldberg's steamclient
            // directly in-prefix, exactly like goldberg's ColdClientLoader scenario.
            cmd.env("WINEDLLOVERRIDES", "lsteamclient=d");

            // Steam launch-context env that the ColdClientLoader normally sets, so
            // the game's Steamworks wrapper believes it was launched by Steam.
            cmd.env("SteamClientLaunch", "1");
            cmd.env("SteamEnv", "1");
            cmd.env("SteamAppUser", "splitux");
            cmd.env("SteamUser", "splitux");
        }
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
pub fn get_prefix_user_path(cfg: &SplituxConfig, instance_idx: usize) -> PathBuf {
    get_prefix_path(cfg, instance_idx).join("drive_c/users/steamuser")
}
