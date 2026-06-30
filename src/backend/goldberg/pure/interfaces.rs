//! Pure extraction of Steam interface version strings from a steam_api binary.
//!
//! goldberg reads `steam_settings/steam_interfaces.txt` to know which versioned
//! interface names (e.g. `SteamClient020`, `STEAMUNIFIEDMESSAGES_INTERFACE_VERSION001`)
//! a game may request. Without it, a game that asks for a version the emulator can't
//! infer hits `report_missing_impl` — in stock gbe_fork builds that fatally exits the
//! game. The list is a *derived fact about the binary*, not a config preference, so
//! splitux generates it at deploy time by scanning the game's own DLL/.so.
//!
//! The pattern set is copied verbatim from gbe_fork's
//! `tools/generate_interfaces/generate_interfaces.cpp` so output matches the
//! canonical generator. Kept pure (bytes in, strings out) for testability.

use regex::bytes::Regex;
use std::collections::HashSet;

/// Interface name regexes, verbatim from gbe_fork generate_interfaces.cpp.
/// Order is preserved in the emitted file to match the canonical generator.
const INTERFACE_PATTERNS: &[&str] = &[
    r"STEAMAPPS_INTERFACE_VERSION\d+",
    r"STEAMAPPLIST_INTERFACE_VERSION\d+",
    r"STEAMAPPTICKET_INTERFACE_VERSION\d+",
    r"SteamClient\d+",
    r"STEAMCONTROLLER_INTERFACE_VERSION",
    r"SteamController\d+",
    r"SteamFriends\d+",
    r"SteamGameServerStats\d+",
    r"SteamGameCoordinator\d+",
    r"SteamGameServer\d+",
    r"STEAMHTMLSURFACE_INTERFACE_VERSION_\d+",
    r"STEAMHTTP_INTERFACE_VERSION\d+",
    r"SteamInput\d+",
    r"STEAMINVENTORY_INTERFACE_V\d+",
    r"SteamMatchMakingServers\d+",
    r"SteamMatchMaking\d+",
    r"SteamMatchGameSearch\d+",
    r"SteamParties\d+",
    r"STEAMMUSIC_INTERFACE_VERSION\d+",
    r"STEAMMUSICREMOTE_INTERFACE_VERSION\d+",
    r"SteamNetworkingMessages\d+",
    r"SteamNetworkingSockets\d+",
    r"SteamNetworkingUtils\d+",
    r"SteamNetworking\d+",
    r"STEAMPARENTALSETTINGS_INTERFACE_VERSION\d+",
    r"STEAMREMOTEPLAY_INTERFACE_VERSION\d+",
    r"STEAMREMOTESTORAGE_INTERFACE_VERSION\d+",
    r"STEAMSCREENSHOTS_INTERFACE_VERSION\d+",
    r"STEAMTIMELINE_INTERFACE_V\d+",
    r"STEAMUGC_INTERFACE_VERSION\d+",
    r"SteamUser\d+",
    r"STEAMUSERSTATS_INTERFACE_VERSION\d+",
    r"SteamUtils\d+",
    r"STEAMVIDEO_INTERFACE_V\d+",
    r"STEAMUNIFIEDMESSAGES_INTERFACE_VERSION\d+",
    r"SteamMasterServerUpdater\d+",
];

/// Scan raw DLL/.so bytes and return the de-duplicated Steam interface version
/// strings, in pattern order (matching gbe_fork's emit order). Empty if nothing
/// matched (e.g. a packed/obfuscated binary).
pub fn extract_interfaces(bytes: &[u8]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for patt in INTERFACE_PATTERNS {
        // Patterns are compile-time constants and known-valid.
        let re = Regex::new(patt).expect("valid interface regex");
        for m in re.find_iter(bytes) {
            // Matches are ASCII by construction, so utf8 conversion never fails.
            if let Ok(s) = std::str::from_utf8(m.as_bytes())
                && seen.insert(s.to_owned()) {
                    out.push(s.to_owned());
                }
        }
    }
    out
}

/// Render the steam_interfaces.txt body (newline-separated, trailing newline) for a
/// binary's bytes, or `None` if no interfaces were found (so callers can skip the
/// write rather than emit an empty/misleading file).
pub fn interfaces_file_contents(bytes: &[u8]) -> Option<String> {
    let list = extract_interfaces(bytes);
    if list.is_empty() {
        None
    } else {
        Some(format!("{}\n", list.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_versioned_and_fixed_interfaces() {
        // interface strings embedded amid binary noise
        let blob = b"\x00\x01SteamClient020\x00garbageSteamUser023\xffSTEAMCONTROLLER_INTERFACE_VERSION\x00";
        let found = extract_interfaces(blob);
        assert!(found.contains(&"SteamClient020".to_string()));
        assert!(found.contains(&"SteamUser023".to_string()));
        assert!(found.contains(&"STEAMCONTROLLER_INTERFACE_VERSION".to_string()));
    }

    #[test]
    fn dedups_repeated_matches() {
        let blob = b"SteamClient020 SteamClient020 SteamClient020";
        assert_eq!(extract_interfaces(blob), vec!["SteamClient020".to_string()]);
    }

    #[test]
    fn networking_prefix_does_not_swallow_specific() {
        // "SteamNetworking\d+" must not match the "SteamNetworking" inside
        // "SteamNetworkingSockets012" (next char is 'S', not a digit).
        let blob = b"SteamNetworkingSockets012 SteamNetworking006";
        let found = extract_interfaces(blob);
        assert!(found.contains(&"SteamNetworkingSockets012".to_string()));
        assert!(found.contains(&"SteamNetworking006".to_string()));
    }

    #[test]
    fn empty_when_nothing_matches() {
        assert!(interfaces_file_contents(b"no interfaces here at all").is_none());
    }

    /// Manual parity harness: SPLITUX_ITF_DLL=<dll> SPLITUX_ITF_OUT=<path>
    /// `cargo test dump_dll_interfaces -- --ignored --nocapture`
    /// Writes our extractor's steam_interfaces.txt for diffing against gbe_fork's tool.
    #[test]
    #[ignore]
    fn dump_dll_interfaces() {
        let dll = std::env::var("SPLITUX_ITF_DLL").expect("set SPLITUX_ITF_DLL");
        let out = std::env::var("SPLITUX_ITF_OUT").expect("set SPLITUX_ITF_OUT");
        let bytes = std::fs::read(&dll).expect("read dll");
        let body = interfaces_file_contents(&bytes).unwrap_or_default();
        std::fs::write(&out, &body).expect("write out");
        eprintln!("wrote {} bytes ({} lines) to {}", body.len(), body.lines().count(), out);
    }

    #[test]
    fn file_contents_has_trailing_newline() {
        let body = interfaces_file_contents(b"SteamClient020").unwrap();
        assert!(body.ends_with('\n'));
        assert_eq!(body, "SteamClient020\n");
    }
}
