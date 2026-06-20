//! Persistent, reusable **Session presets** for the TUI.
//!
//! A Session is the durable bundle a user comes back to: a game + the set of
//! profiles played + the last input layout (+ an optional save anchor). Launching
//! a config auto-records a Session, deduplicated by **(game + set of profiles)** —
//! input/together mode is NOT part of identity, so re-running the same game with
//! the same profiles updates the existing Session rather than appending a new one.
//!
//! A separate tiny **runtime registry** maps a Session to the single live systemd
//! runtime it spawned (`splitux-<pid>_0.slice`), so the TUI can show ● active and
//! end/kill the one runtime without the user ever seeing the child scopes.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::paths::PATH_PARTY;

/// Unpinned Sessions older than this (by last-used) are garbage-collected on load.
const GC_MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum StoredInput {
    KbMouse,
    Gamepad,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SavedPlayer {
    /// Profile name (not index — names are stable across handler/profile rescans).
    pub profile: String,
    pub input: StoredInput,
    /// false = local (drives the host), true = a remote Together seat.
    pub together: bool,
}

/// Optional save-file anchor: carry a real on-disk (Steam) save into the session
/// and sync it back at the end. See `save_sync` for the engine this drives.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct SaveAnchor {
    pub enabled: bool,
    /// Which profile owns the canonical (anchored) save — the "master".
    pub master_profile: String,
    /// Absolute path to the real original save. Empty = unresolved (TUI flags it).
    pub save_path: String,
    /// Remap Steam IDs in save filenames (DRG-style). Auto-set for known games.
    pub steam_id_remap: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SavedSession {
    /// Dedup key: `slug(game) "|" sorted_unique(profiles)`.
    pub id: String,
    /// Display name — auto-generated default, user-renamable.
    pub name: String,
    pub game: String,
    pub players: Vec<SavedPlayer>,
    #[serde(default)]
    pub anchor: Option<SaveAnchor>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub last_used: u64,
}

/// Seconds since the Unix epoch (0 if the clock is before 1970, which it isn't).
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Lowercase, non-alphanumeric → '-' (matches the TUI's log-path slugging).
fn slug(s: &str) -> String {
    s.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "-")
}

/// Sorted, de-duplicated profile names — the profile half of the dedup identity.
fn sorted_unique_profiles(players: &[SavedPlayer]) -> Vec<String> {
    let mut v: Vec<String> = players.iter().map(|p| p.profile.clone()).collect();
    v.sort();
    v.dedup();
    v
}

/// Build the dedup key for a (game, players) pair. Identity is game + profile SET
/// only — input/together deliberately excluded.
pub fn session_key(game: &str, players: &[SavedPlayer]) -> String {
    format!("{}|{}", slug(game), sorted_unique_profiles(players).join(","))
}

/// Default human name, e.g. `V Rising — Gabe·local + Jay-Z·together`.
pub fn auto_name(game: &str, players: &[SavedPlayer]) -> String {
    let who: Vec<String> = players
        .iter()
        .map(|p| {
            let scope = if p.together { "together" } else { "local" };
            format!("{}·{}", p.profile, scope)
        })
        .collect();
    if who.is_empty() {
        game.to_string()
    } else {
        format!("{} — {}", game, who.join(" + "))
    }
}

fn store_path() -> PathBuf {
    PATH_PARTY.join("sessions.json")
}

/// Load saved sessions, garbage-collecting unpinned ones older than a week. If any
/// were dropped, the pruned list is written back.
pub fn load() -> Vec<SavedSession> {
    let path = store_path();
    let mut sessions: Vec<SavedSession> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default();

    let now = now_secs();
    let before = sessions.len();
    sessions.retain(|s| s.pinned || now.saturating_sub(s.last_used) <= GC_MAX_AGE_SECS);
    if sessions.len() != before {
        save(&sessions);
    }
    sessions
}

/// Persist the full session list (pretty JSON for hand-inspection).
pub fn save(sessions: &[SavedSession]) {
    let path = store_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(sessions) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                eprintln!("[splitux] session_store - write {} failed: {e}", path.display());
            }
        }
        Err(e) => eprintln!("[splitux] session_store - serialize failed: {e}"),
    }
}

/// Insert or update the Session for `(game, players)`, deduped by [`session_key`].
/// On an existing match: refresh the layout (players) + `last_used`, keep the
/// user's name/pin/anchor. Returns the Session id.
pub fn upsert(sessions: &mut Vec<SavedSession>, game: &str, players: Vec<SavedPlayer>) -> String {
    let id = session_key(game, &players);
    let now = now_secs();
    if let Some(existing) = sessions.iter_mut().find(|s| s.id == id) {
        existing.players = players;
        existing.last_used = now;
        // The anchor's master_profile must still be one of the players; leave the
        // anchor as-is (the TUI re-validates when configuring it).
    } else {
        sessions.push(SavedSession {
            id: id.clone(),
            name: auto_name(game, &players),
            game: game.to_string(),
            players,
            anchor: None,
            pinned: false,
            created_at: now,
            last_used: now,
        });
    }
    save(sessions);
    id
}

// ---------------------------------------------------------------------------
// Runtime markers: which Session owns which live systemd runtime.
//
// A launch re-execs itself into `splitux-main-<P>.scope` via `systemd-run
// --scope`, so the real supervisor is a grandchild with a DIFFERENT pid (P2) and
// its launch slice is `splitux-<P2>_0.slice` — unknowable from the pid the TUI
// captured at spawn. So the supervisor itself writes a marker (it knows its slice,
// its main scope, and the `SPLITUX_SESSION_ID` the TUI passed in the env). The TUI
// reads markers to show ● active and to target end/kill at the exact units:
//   - End & sync : stop the launch SLICE  → games die, supervisor (in the main
//                  scope) survives and runs its built-in save sync-back.
//   - Force kill : stop the MAIN SCOPE     → everything dies at once, no sync.
// ---------------------------------------------------------------------------

/// Env var the TUI sets so the launch supervisor can tag its runtime marker.
pub const SESSION_ID_ENV: &str = "SPLITUX_SESSION_ID";

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RuntimeMarker {
    pub session_id: String,
    /// Launch slice holding the game + seat scopes (NOT the supervisor).
    pub slice: String,
    /// The supervisor's own scope; alive for the whole session incl. sync-back.
    pub main_scope: String,
}

fn runtime_dir() -> PathBuf {
    // NOT under tmp/: the supervisor's end-of-session `clear_tmp()` wipes all of
    // PATH_PARTY/tmp, which would delete a co-running session's marker.
    PATH_PARTY.join("runtime")
}

fn marker_file(session_id: &str) -> PathBuf {
    // session_id is `slug(game)|prof,prof` — filename-safe on Linux (no '/').
    runtime_dir().join(format!("{}.json", session_id.replace('/', "_")))
}

/// Supervisor side: record this live runtime for `session_id`.
pub fn write_marker(session_id: &str, slice: &str, main_scope: &str) {
    let _ = std::fs::create_dir_all(runtime_dir());
    let m = RuntimeMarker {
        session_id: session_id.to_string(),
        slice: slice.to_string(),
        main_scope: main_scope.to_string(),
    };
    if let Ok(json) = serde_json::to_string(&m) {
        let _ = std::fs::write(marker_file(session_id), json);
    }
}

/// Drop a Session's marker (clean teardown, or after a TUI force-kill).
pub fn remove_marker(session_id: &str) {
    let _ = std::fs::remove_file(marker_file(session_id));
}

/// All runtime markers currently on disk (one per believed-live session).
pub fn list_markers() -> Vec<RuntimeMarker> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(runtime_dir()) {
        for e in entries.flatten() {
            if let Ok(data) = std::fs::read_to_string(e.path()) {
                if let Ok(m) = serde_json::from_str::<RuntimeMarker>(&data) {
                    out.push(m);
                }
            }
        }
    }
    out
}

/// The marker for one Session, if present.
pub fn find_marker(session_id: &str) -> Option<RuntimeMarker> {
    std::fs::read_to_string(marker_file(session_id))
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(profile: &str, input: StoredInput, together: bool) -> SavedPlayer {
        SavedPlayer { profile: profile.into(), input, together }
    }

    #[test]
    fn dedup_key_ignores_input_and_together_but_not_profiles() {
        // Same game + same profile SET → same key, regardless of input/together.
        let a = vec![
            p("Gabe", StoredInput::Gamepad, false),
            p("Jay-Z", StoredInput::Gamepad, true),
        ];
        let b = vec![
            p("Jay-Z", StoredInput::KbMouse, false), // different order, input, together
            p("Gabe", StoredInput::KbMouse, false),
        ];
        assert_eq!(session_key("V Rising", &a), session_key("V Rising", &b));

        // Different profile set → different key.
        let c = vec![p("Gabe", StoredInput::Gamepad, false)];
        assert_ne!(session_key("V Rising", &a), session_key("V Rising", &c));

        // Different game → different key.
        assert_ne!(session_key("V Rising", &a), session_key("Deep Rock", &a));
    }

    #[test]
    fn upsert_dedups_and_refreshes_layout() {
        let mut v: Vec<SavedSession> = Vec::new();
        // Avoid touching disk: exercise the in-vec dedup via session_key directly.
        let game = "V Rising";
        let first = vec![p("Gabe", StoredInput::Gamepad, false)];
        let id1 = session_key(game, &first);
        v.push(SavedSession {
            id: id1.clone(),
            name: auto_name(game, &first),
            game: game.into(),
            players: first,
            anchor: None,
            pinned: false,
            created_at: 1,
            last_used: 1,
        });
        // A re-launch with the SAME game+profile but different input must map to the
        // same id and update the stored layout in place (no new entry).
        let again = vec![p("Gabe", StoredInput::KbMouse, true)];
        let id2 = session_key(game, &again);
        assert_eq!(id1, id2);
        if let Some(t) = v.iter_mut().find(|s| s.id == id2) {
            t.players = again.clone();
        }
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].players[0].input, StoredInput::KbMouse);
        assert!(v[0].players[0].together);
    }

    #[test]
    fn auto_name_lists_players_with_scope() {
        let players = vec![
            p("Gabe", StoredInput::Gamepad, false),
            p("Jay-Z", StoredInput::Gamepad, true),
        ];
        assert_eq!(
            auto_name("V Rising", &players),
            "V Rising — Gabe·local + Jay-Z·together"
        );
    }
}
