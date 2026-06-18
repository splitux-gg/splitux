//! SteamStub DRM stripping (Steamless) for goldberg steamclient games.
//!
//! Some steamclient-path games (e.g. Patch Quest) ship a SteamStub-wrapped exe — a
//! `.bind` PE section that, run under goldberg instead of real Steam, fails to
//! validate/decrypt and pops "Application load error 3:0000065432". The fix is a
//! DRM-free exe produced by Steamless. To keep the real install pristine we never
//! patch in place: we produce the stripped exe once, cache it under
//! `{PATH_PARTY}/steamless/{appid}/`, and the overlay shadows the original with it.
//!
//! Steamless is a .NET Framework CLI bundled at `{PATH_PARTY}/tools/steamless/`; it
//! runs under Proton via umu-run in a dedicated throwaway prefix.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::paths::{BIN_UMU_RUN, PATH_PARTY, PATH_STEAM};

/// True if `exe` is SteamStub-wrapped (has a `.bind` PE section).
pub fn has_steamstub(exe: &Path) -> bool {
    let data = match fs::read(exe) {
        Ok(d) => d,
        Err(_) => return false,
    };
    if data.len() < 0x40 || &data[0..2] != b"MZ" {
        return false;
    }
    let e_lfanew =
        u32::from_le_bytes([data[0x3c], data[0x3d], data[0x3e], data[0x3f]]) as usize;
    if data.len() < e_lfanew + 24 || &data[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return false;
    }
    let num_sections = u16::from_le_bytes([data[e_lfanew + 6], data[e_lfanew + 7]]) as usize;
    let opt_size = u16::from_le_bytes([data[e_lfanew + 20], data[e_lfanew + 21]]) as usize;
    let sec_off = e_lfanew + 24 + opt_size;
    for i in 0..num_sections {
        let o = sec_off + i * 40;
        if o + 8 > data.len() {
            break;
        }
        if data[o..o + 8].starts_with(b".bind") {
            return true;
        }
    }
    false
}

/// Ensure a DRM-free copy of `game_root/exec_rel` exists in the cache, stripping it
/// with Steamless if needed. Returns the cached stripped exe path, or None if the
/// exe isn't DRM-wrapped (no strip needed) or the strip failed.
pub fn ensure_stripped(game_root: &Path, exec_rel: &Path, appid: u32) -> Option<PathBuf> {
    let exe_name = exec_rel.file_name()?;
    let cache_dir = PATH_PARTY.join("steamless").join(appid.to_string());
    let cache = cache_dir.join(exe_name);
    if cache.exists() {
        return Some(cache);
    }

    let src = game_root.join(exec_rel);
    if !src.exists() || !has_steamstub(&src) {
        return None; // not wrapped — nothing to strip
    }

    let cli = PATH_PARTY.join("tools/steamless/Steamless.CLI.exe");
    if !cli.exists() {
        println!(
            "[splitux] goldberg.steamclient: SteamStub DRM on {} but Steamless not bundled at {} — \
             game will error; strip it manually and cache at {}",
            exec_rel.display(),
            cli.display(),
            cache.display()
        );
        return None;
    }

    println!(
        "[splitux] goldberg.steamclient: SteamStub DRM detected on {} — running Steamless (one-time)…",
        exec_rel.display()
    );

    let work = cache_dir.join("work");
    let _ = fs::create_dir_all(&work);
    let work_exe = work.join(exe_name);
    if let Err(e) = fs::copy(&src, &work_exe) {
        println!("[splitux] goldberg.steamclient: couldn't stage exe for Steamless: {e}");
        return None;
    }

    let pfx = PATH_PARTY.join("tools").join("steamless-pfx");
    let _ = fs::create_dir_all(&pfx);

    // umu-run <Steamless.CLI.exe> Z:<work_exe>  (Steamless writes <work_exe>.unpacked.exe)
    let status = Command::new(&*BIN_UMU_RUN)
        .arg(&cli)
        .arg(format!("Z:{}", work_exe.display()))
        .env("WINEPREFIX", &pfx)
        .env("GAMEID", "umu-0")
        .env("PROTONPATH", "GE-Proton")
        .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &*PATH_STEAM)
        .env("UMU_RUNTIME_UPDATE", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let unpacked = work.join(format!("{}.unpacked.exe", exe_name.to_string_lossy()));
    if unpacked.exists() {
        let placed = fs::rename(&unpacked, &cache)
            .or_else(|_| fs::copy(&unpacked, &cache).map(|_| ()))
            .is_ok();
        let _ = fs::remove_dir_all(&work);
        if placed {
            println!(
                "[splitux] goldberg.steamclient: Steamless stripped the DRM → {}",
                cache.display()
            );
            return Some(cache);
        }
    }
    println!(
        "[splitux] goldberg.steamclient: Steamless did not produce an unpacked exe (status {:?}); \
         SteamStub strip failed — the game will likely show 'Application load error'",
        status.ok()
    );
    None
}
