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

/// Get the Wine prefix path for an instance, keyed by PROFILE name.
///
/// Per-profile (not per-instance-index) so that CONCURRENT splitux launches — each
/// of which has instance_idx 0 and would otherwise both map to prefix "1" — get
/// distinct prefixes and don't collide. Proton file-locks STEAM_COMPAT_DATA_PATH
/// (== the prefix), so a shared prefix means only one same-game instance boots.
/// Keyed by profile (vs launch namespace) so each user's prefix is STABLE/reusable
/// across launches — no Wine-prefix re-init cost per run.
///
/// `dup_idx` disambiguates the rare case of TWO instances in the SAME launch+game
/// sharing one profile name: the first (dup_idx 0) keeps the bare per-profile
/// prefix (so the common one-instance-per-profile case is unchanged and stays
/// reusable), and each additional same-profile sibling gets a `-p<n>` suffix so
/// they don't collide on — and file-lock — one prefix.
pub fn get_prefix_path(cfg: &SplituxConfig, profname: &str, game: usize, dup_idx: usize) -> PathBuf {
    let base = match cfg.proton_separate_pfxs {
        true => profname.replace(['/', '\\'], "_"),
        false => "1".to_string(),
    };
    // Multi-game: namespace games AFTER the first so two concurrent games that
    // reuse the same profile name don't share — and fight over — one Wine prefix.
    // Game 0 keeps the legacy name, so existing single-game prefixes stay valid
    // (no re-init) and single-game is byte-identical.
    let mut dir = if game == 0 {
        base
    } else {
        format!("{base}-g{game}")
    };
    if dup_idx > 0 {
        dir = format!("{dir}-p{dup_idx}");
    }
    PATH_PARTY.join("prefixes").join(dir)
}

/// Set up Proton environment variables on a command
///
/// This sets all the necessary environment variables for Proton to work correctly
/// outside of Steam, including WINEPREFIX, STEAM_COMPAT_DATA_PATH, etc.
pub fn setup_env(
    cmd: &mut Command,
    handler: &Handler,
    cfg: &SplituxConfig,
    profname: &str,
    game: usize,
    dup_idx: usize,
) {
    let path_pfx = get_prefix_path(cfg, profname, game, dup_idx);

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
pub fn get_prefix_user_path(cfg: &SplituxConfig, profname: &str, game: usize, dup_idx: usize) -> PathBuf {
    get_prefix_path(cfg, profname, game, dup_idx).join("drive_c/users/steamuser")
}
