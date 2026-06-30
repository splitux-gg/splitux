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

fn rd_u16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}
fn rd_u32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}
fn rd_u64(d: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(d.get(o..o + 8)?.try_into().ok()?))
}

/// True if `data` is a NATIVE Linux x86-64 ELF carrying a SteamStub `.bind`
/// section (the DRM gate, with the ELF entry point inside it). Linux SteamStub
/// does NOT encrypt `.text` — it just gates on Steam then jumps to the real OEP.
fn has_steamstub_elf(data: &[u8]) -> bool {
    if !data.starts_with(b"\x7fELF") || data.get(4) != Some(&2) {
        return false; // not ELF64
    }
    let (shoff, shentsize, shnum, shstrndx) = match (
        rd_u64(data, 0x28),
        rd_u16(data, 0x3a),
        rd_u16(data, 0x3c),
        rd_u16(data, 0x3e),
    ) {
        (Some(a), Some(b), Some(c), Some(d)) => (a as usize, b as usize, c as usize, d as usize),
        _ => return false,
    };
    // shstrtab: section names blob
    let str_sh = shoff + shstrndx * shentsize;
    let strtab_off = match rd_u64(data, str_sh + 0x18) {
        Some(v) => v as usize,
        None => return false,
    };
    for i in 0..shnum {
        let sh = shoff + i * shentsize;
        let Some(name_off) = rd_u32(data, sh).map(|v| strtab_off + v as usize) else {
            continue;
        };
        let end = data[name_off..].iter().position(|&b| b == 0).unwrap_or(0);
        if &data[name_off..name_off + end] == b".bind" {
            return true;
        }
    }
    false
}

/// Find the original entry point (glibc `_start`) in a SteamStub'd ELF by its
/// fixed x86-64 prologue, mapping the file offset back to a vaddr via PT_LOAD.
/// Requires EXACTLY one match (the real `_start`) to avoid mis-patching.
fn find_oep_elf(data: &[u8]) -> Option<u64> {
    // xor ebp,ebp; mov r9,rdx; pop rsi; mov rdx,rsp; and rsp,-16
    const SIG: &[u8] = &[
        0x31, 0xED, 0x49, 0x89, 0xD1, 0x5E, 0x48, 0x89, 0xE2, 0x48, 0x83, 0xE4, 0xF0,
    ];
    let hits: Vec<usize> = data
        .windows(SIG.len())
        .enumerate()
        .filter(|(_, w)| *w == SIG)
        .map(|(i, _)| i)
        .collect();
    if hits.len() != 1 {
        return None;
    }
    let off = hits[0] as u64;
    // map file offset -> vaddr via the containing PT_LOAD segment
    let (phoff, phentsize, phnum) = (
        rd_u64(data, 0x20)? as usize,
        rd_u16(data, 0x36)? as usize,
        rd_u16(data, 0x38)? as usize,
    );
    for i in 0..phnum {
        let ph = phoff + i * phentsize;
        if rd_u32(data, ph)? != 1 {
            continue; // PT_LOAD
        }
        let p_off = rd_u64(data, ph + 0x08)?;
        let p_vaddr = rd_u64(data, ph + 0x10)?;
        let p_filesz = rd_u64(data, ph + 0x20)?;
        if off >= p_off && off < p_off + p_filesz {
            return Some(p_vaddr + (off - p_off));
        }
    }
    None
}

/// Strip a native-Linux SteamStub by repointing the ELF entry point to the real
/// `_start` (the stub then never runs). Writes the patched copy to `dst`. The
/// original is untouched (the overlay shadows it with `dst`).
fn strip_steamstub_elf(data: &[u8], dst: &Path) -> bool {
    let Some(oep) = find_oep_elf(data) else {
        println!("[splitux] SteamStub(ELF): couldn't locate a unique _start — not stripping");
        return false;
    };
    let cur_entry = rd_u64(data, 0x18).unwrap_or(0);
    let mut out = data.to_vec();
    out[0x18..0x20].copy_from_slice(&oep.to_le_bytes());
    match fs::write(dst, &out) {
        Ok(()) => {
            let _ = std::fs::set_permissions(dst, std::os::unix::fs::PermissionsExt::from_mode(0o755));
            println!(
                "[splitux] SteamStub(ELF): stripped — entry 0x{cur_entry:x} -> _start 0x{oep:x} → {}",
                dst.display()
            );
            true
        }
        Err(e) => {
            println!("[splitux] SteamStub(ELF): write failed: {e}");
            false
        }
    }
}

/// Ensure a DRM-free copy of `game_root/exec_rel` exists in the cache, stripping it
/// if needed. Native Linux ELF → repoint entry to `_start` (in-Rust). Windows PE →
/// Steamless under Proton. Returns the cached stripped exe path, or None if the exe
/// isn't DRM-wrapped (no strip needed) or the strip failed.
pub fn ensure_stripped(game_root: &Path, exec_rel: &Path, appid: u32) -> Option<PathBuf> {
    let exe_name = exec_rel.file_name()?;
    let cache_dir = PATH_PARTY.join("steamless").join(appid.to_string());
    let cache = cache_dir.join(exe_name);
    if cache.exists() {
        return Some(cache);
    }

    let src = game_root.join(exec_rel);
    if !src.exists() {
        return None;
    }

    // NATIVE Linux ELF SteamStub: strip in-process (no Steamless/Proton needed).
    if let Ok(data) = fs::read(&src) {
        if data.starts_with(b"\x7fELF") {
            if !has_steamstub_elf(&data) {
                return None; // not wrapped
            }
            println!(
                "[splitux] SteamStub(ELF) detected on {} — stripping (one-time)…",
                exec_rel.display()
            );
            let _ = fs::create_dir_all(&cache_dir);
            return strip_steamstub_elf(&data, &cache).then_some(cache);
        }
    }

    if !has_steamstub(&src) {
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
