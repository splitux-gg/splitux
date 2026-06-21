//! Interactive terminal UI for assembling and managing splitux sessions.
//!
//! A keyboard-driven alternative to the egui GUI, usable over SSH: pick a game,
//! choose a Local or Together mode, assign profiles (plus an input per player in
//! Local mode — Together streams both kb/m and gamepad to every seat, so the
//! per-player input choice is moot and is hidden), then launch the session
//! *detached* so the TUI stays live to watch, kill, and restart running sessions.
//!
//! It reuses the same scanners the GUI/CLI use (`scan_handlers`, `scan_profiles`)
//! and shells out to `splitux launch` (this same binary) + `systemctl --user` so
//! a launched session runs independently of the TUI process.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::StatefulImage;

use crate::handler::scan_handlers;
use crate::profiles::scan_profiles;
use crate::session_store::{self, SavedPlayer, SavedSession, SaveAnchor, StoredInput};

#[derive(Clone, Copy, PartialEq)]
enum InputMode {
    KbMouse,
    Gamepad,
}

impl InputMode {
    fn label(self) -> &'static str {
        match self {
            InputMode::KbMouse => "kb/m",
            InputMode::Gamepad => "gamepad",
        }
    }
    fn toggled(self) -> Self {
        match self {
            InputMode::KbMouse => InputMode::Gamepad,
            InputMode::Gamepad => InputMode::KbMouse,
        }
    }
    /// The `input=` value for `splitux launch`, honoring this player's
    /// local-vs-together scope (mixed sessions: some players local, some remote).
    fn spec(self, together: bool) -> String {
        let kind = match self {
            InputMode::KbMouse => "kbm",
            InputMode::Gamepad => "gamepad",
        };
        let scope = if together { "together" } else { "local" };
        format!("{scope}:{kind}")
    }
    /// Unused legacy local-only spec, kept for reference.
    #[allow(dead_code)]
    fn local_spec(self) -> &'static str {
        match self {
            InputMode::KbMouse => "local:kbm",
            InputMode::Gamepad => "local:gamepad",
        }
    }
}

#[derive(Clone)]
struct Player {
    profile: usize, // index into App::profiles
    input: InputMode,
    /// false = local (drives the host directly), true = a remote Together seat.
    /// Per-player so one session can mix local and together players.
    together: bool,
}

#[derive(PartialEq, Clone, Copy)]
enum Screen {
    Games,
    Build,
    Sessions,
}

struct EditState {
    /// Index into `App::sessions` being renamed.
    target: usize,
    buf: String,
}

/// Map TUI players (profile-by-index) → store players (profile-by-name).
fn to_saved_players(players: &[Player], profiles: &[String]) -> Vec<SavedPlayer> {
    players
        .iter()
        .map(|p| SavedPlayer {
            profile: profiles.get(p.profile).cloned().unwrap_or_else(|| "Guest".into()),
            input: match p.input {
                InputMode::KbMouse => StoredInput::KbMouse,
                InputMode::Gamepad => StoredInput::Gamepad,
            },
            together: p.together,
        })
        .collect()
}

/// Map store players (by name) back to TUI players (by profile index).
fn from_saved_players(saved: &[SavedPlayer], profiles: &[String]) -> Vec<Player> {
    saved
        .iter()
        .map(|s| Player {
            profile: profiles.iter().position(|p| p == &s.profile).unwrap_or(0),
            input: match s.input {
                StoredInput::KbMouse => InputMode::KbMouse,
                StoredInput::Gamepad => InputMode::Gamepad,
            },
            together: s.together,
        })
        .collect()
}

/// Compact relative-time label for a unix-secs timestamp.
fn rel_time(then: u64) -> String {
    let now = session_store::now_secs();
    let d = now.saturating_sub(then);
    if d < 60 {
        "just now".into()
    } else if d < 3600 {
        format!("{}m ago", d / 60)
    } else if d < 86400 {
        format!("{}h ago", d / 3600)
    } else {
        format!("{}d ago", d / 86400)
    }
}

struct App {
    games: Vec<String>,
    profiles: Vec<String>,

    screen: Screen,

    // games picker
    filter: String,
    game_cursor: usize, // index into the *filtered* list

    // Per-game cover image FILE (parallel to `games`); None when a game has no
    // resolvable local/Steam-cache art. Rendered in the picker preview pane via
    // a terminal graphics protocol (kitty/sixel/iterm), when one is available.
    covers: Vec<Option<PathBuf>>,
    // Per-game preview text (handler.info, the uniform Play/Avoid block), parallel to `games`.
    infos: Vec<String>,
    // Per-game Steam appid + handler's declared save sub-path (parallel to `games`),
    // used to auto-discover the real on-disk save when anchoring a Session.
    appids: Vec<Option<u32>>,
    save_subpaths: Vec<String>,
    // Per-game: save lives in Steam Cloud / Remote Storage (resolve from userdata).
    save_cloud: Vec<bool>,
    // Per-game small icon FILE (parallel to `games`) for the list, + a lazy cache
    // of encoded icon protocols keyed by game index (built as rows scroll in).
    icons: Vec<Option<PathBuf>>,
    icon_protos: HashMap<usize, StatefulProtocol>,
    // Graphics-protocol picker (queried once from the terminal). None when the
    // terminal can't do inline images — the preview pane then shows a hint.
    picker: Option<Picker>,
    // Cached render protocol for the currently-previewed game: (games index,
    // protocol). Rebuilt only when the selection changes (encoding is costly),
    // not every frame.
    cover_proto: Option<(usize, StatefulProtocol)>,

    // build screen
    game: Option<usize>, // index into games
    players: Vec<Player>,
    player_cursor: usize,
    /// Whether the session being built anchors the master profile's real save
    /// (carry it in at start, sync back at end). Part of the session's config.
    build_anchor: bool,

    // sessions (saved presets correlated with live runtimes)
    sessions: Vec<SavedSession>,
    /// Session ids with a live runtime (marker's main scope still active).
    active_ids: HashSet<String>,
    session_cursor: usize,
    last_sessions_refresh: Instant,
    /// Inline text editor (rename / anchor path) on the Sessions screen.
    editing: Option<EditState>,

    status: String,
    quit: bool,

    // Together invite link(s), read from together-invites.txt after a together
    // launch (the detached `splitux launch` writes it asynchronously).
    invite_url: Option<String>,
    awaiting_invite: Option<Instant>,

    // After a (detached) launch, tail this log into the status bar so the user
    // sees real progress/errors instead of nothing. Watched until the deadline,
    // an error line, or a session/invite shows up.
    launch_log: Option<PathBuf>,
    launch_deadline: Option<Instant>,
}

impl App {
    fn new() -> Self {
        // Scan once and keep both the display name and the cover path per game
        // (same art resolution the GUI uses), parallel to `games`.
        let handlers = scan_handlers();
        let games: Vec<String> = handlers.iter().map(|h| h.display().to_string()).collect();
        let covers: Vec<Option<PathBuf>> = handlers.iter().map(|h| h.cover_path()).collect();
        let infos: Vec<String> = handlers.iter().map(|h| h.info.trim().to_string()).collect();
        let icons: Vec<Option<PathBuf>> = handlers.iter().map(|h| h.icon_path()).collect();
        let appids: Vec<Option<u32>> = handlers.iter().map(|h| h.get_steam_appid()).collect();
        let save_subpaths: Vec<String> =
            handlers.iter().map(|h| h.original_save_path.clone()).collect();
        let save_cloud: Vec<bool> = handlers.iter().map(|h| h.save_steam_cloud).collect();
        let mut profiles = scan_profiles(true);
        if profiles.is_empty() {
            profiles.push("Guest".to_string());
        }
        let mut app = App {
            games,
            profiles,
            screen: Screen::Games,
            filter: String::new(),
            game_cursor: 0,
            covers,
            infos,
            appids,
            save_subpaths,
            save_cloud,
            icons,
            icon_protos: HashMap::new(),
            picker: None,
            cover_proto: None,
            game: None,
            players: Vec::new(),
            player_cursor: 0,
            build_anchor: false,
            sessions: Vec::new(),
            active_ids: HashSet::new(),
            session_cursor: 0,
            last_sessions_refresh: Instant::now(),
            editing: None,
            status: "Pick a game — type to filter, Enter to select.".to_string(),
            quit: false,
            invite_url: None,
            awaiting_invite: None,
            launch_log: None,
            launch_deadline: None,
        };
        app.refresh_sessions();
        app
    }

    /// Indices into `games` matching the current filter (case-insensitive).
    fn filtered(&self) -> Vec<usize> {
        let f = self.filter.to_lowercase();
        self.games
            .iter()
            .enumerate()
            .filter(|(_, g)| f.is_empty() || g.to_lowercase().contains(&f))
            .map(|(i, _)| i)
            .collect()
    }

    /// Reload saved sessions and correlate them with live runtimes. A session is
    /// active iff its runtime marker's main scope is still an active systemd unit;
    /// markers whose runtime is gone are pruned. Active sessions sort to the top.
    fn refresh_sessions(&mut self) {
        let live = scan_active_units();
        let mut active = HashSet::new();
        for m in session_store::list_markers() {
            if live.iter().any(|u| u == &m.main_scope) {
                active.insert(m.session_id);
            } else {
                // Runtime gone (normal exit, crash, or already reaped) — drop it.
                session_store::remove_marker(&m.session_id);
            }
        }
        self.active_ids = active;

        let active_ids = self.active_ids.clone();
        let mut sessions = session_store::load();
        sessions.sort_by(|a, b| {
            let aa = active_ids.contains(&a.id);
            let ab = active_ids.contains(&b.id);
            ab.cmp(&aa).then(b.last_used.cmp(&a.last_used))
        });
        self.sessions = sessions;
        if self.session_cursor >= self.sessions.len() {
            self.session_cursor = self.sessions.len().saturating_sub(1);
        }
        self.last_sessions_refresh = Instant::now();
    }

    fn default_player(&self) -> Player {
        // Prefer a non-Guest profile if one exists.
        let profile = self
            .profiles
            .iter()
            .position(|p| !p.eq_ignore_ascii_case("Guest"))
            .unwrap_or(0);
        Player {
            profile,
            input: InputMode::Gamepad,
            together: false,
        }
    }

    /// Toggle save anchoring for the session being built and report the resolved
    /// real-save path (or a ⚑ flag if it can't be resolved).
    fn toggle_build_anchor(&mut self) {
        self.build_anchor = !self.build_anchor;
        if !self.build_anchor {
            self.status = "Save-anchor OFF — session runs on its own profile save.".into();
            return;
        }
        let Some(gi) = self.game else { return };
        let game = self.games[gi].clone();
        match self.resolve_anchor_path(&game) {
            Ok(p) => self.status = format!("⚓ save-anchor ON → {p}"),
            Err(e) => self.status = format!(
                "⚑ save-anchor ON but can't resolve the real save ({e}) — declare original_save_path in the handler."
            ),
        }
    }

    /// Build the SaveAnchor for the current Build config, if anchoring is on.
    fn current_build_anchor(&self, game: &str) -> Option<SaveAnchor> {
        if !self.build_anchor {
            return None;
        }
        let master = self
            .players
            .iter()
            .filter_map(|p| self.profiles.get(p.profile).cloned())
            .find(|p| !p.eq_ignore_ascii_case("Guest"))
            .or_else(|| self.players.first().and_then(|p| self.profiles.get(p.profile).cloned()))
            .unwrap_or_default();
        Some(SaveAnchor {
            enabled: true,
            master_profile: master,
            save_path: self.resolve_anchor_path(game).unwrap_or_default(),
            steam_id_remap: game.to_lowercase().contains("deep rock"),
        })
    }

    /// Launch from the Build screen. Records (or refreshes) the saved Session preset
    /// WITH its configured save anchor (authoritative from the Build screen).
    fn launch(&mut self) {
        let Some(gi) = self.game else { return };
        if self.players.is_empty() {
            self.status = "Add at least one player (a) before launching.".to_string();
            return;
        }
        let game = self.games[gi].clone();
        let players = self.players.clone();
        let saved = to_saved_players(&players, &self.profiles);
        let id = session_store::session_key(&game, &saved);
        // The Build screen is authoritative for the anchor config.
        let anchor = self.current_build_anchor(&game);
        let live_anchor = anchor.clone().filter(|a| a.enabled && !a.save_path.is_empty());
        let n_together = players.iter().filter(|p| p.together).count();

        match spawn_session(&game, &players, &self.profiles, live_anchor.as_ref(), &id) {
            Ok((log, _pid)) => {
                let mut sessions = session_store::load();
                session_store::upsert(&mut sessions, &game, saved);
                // Persist the Build anchor choice onto the session.
                if let Some(t) = sessions.iter_mut().find(|s| s.id == id) {
                    t.anchor = anchor;
                }
                session_store::save(&sessions);
                self.launch_log = Some(log);
                self.launch_deadline = Some(Instant::now() + Duration::from_secs(30));
                let n_local = players.len() - n_together;
                self.status = format!(
                    "Launched '{}' ({} local + {} together). Press s for sessions.",
                    game, n_local, n_together
                );
                if n_together > 0 {
                    self.invite_url = None;
                    self.awaiting_invite = Some(Instant::now());
                    self.status = format!(
                        "Launched '{}' — fetching invite link(s) for {} together seat(s)…",
                        game, n_together
                    );
                }
                self.refresh_sessions();
            }
            Err(e) => self.status = format!("Launch failed: {e}"),
        }
    }

    /// Start an (inactive) saved Session from the Sessions screen.
    fn start_saved(&mut self, idx: usize) {
        let Some(s) = self.sessions.get(idx).cloned() else { return };
        if self.active_ids.contains(&s.id) {
            self.status = format!("'{}' is already running.", s.name);
            return;
        }
        let players = from_saved_players(&s.players, &self.profiles);
        let anchor = s.anchor.clone().filter(|a| a.enabled);
        match spawn_session(&s.game, &players, &self.profiles, anchor.as_ref(), &s.id) {
            Ok((log, _pid)) => {
                let mut sessions = session_store::load();
                session_store::upsert(&mut sessions, &s.game, s.players.clone()); // bump last_used
                self.launch_log = Some(log);
                self.launch_deadline = Some(Instant::now() + Duration::from_secs(30));
                let anchored = if anchor.is_some() { " (save-anchored)" } else { "" };
                self.status = format!("Starting '{}'{}…", s.name, anchored);
                self.refresh_sessions();
            }
            Err(e) => self.status = format!("Start failed: {e}"),
        }
    }

    /// Gracefully end an active Session: stop the launch SLICE (closes the games so
    /// they flush their saves) but leave the supervisor in its main scope alive to
    /// run its built-in save sync-back. The marker is cleared by the supervisor on
    /// clean exit; we keep showing ● until its main scope is gone.
    fn end_and_sync(&mut self, idx: usize) {
        let Some(s) = self.sessions.get(idx).cloned() else { return };
        let Some(m) = session_store::find_marker(&s.id) else {
            self.status = format!("'{}' has no live runtime.", s.name);
            return;
        };
        let _ = systemctl_stop(&m.slice);
        let synced = match &s.anchor {
            Some(a) if a.enabled => " — syncing save back to Steam",
            _ => "",
        };
        self.status = format!("Ending '{}'{}… (closing game, then sync)", s.name, synced);
        self.refresh_sessions();
    }

    /// Force-kill an active Session: stop the MAIN SCOPE (everything dies at once),
    /// NO save sync. Cleans up + restores bars when no session remains active.
    fn force_kill(&mut self, idx: usize) {
        let Some(s) = self.sessions.get(idx).cloned() else { return };
        if let Some(m) = session_store::find_marker(&s.id) {
            let _ = systemctl_stop(&m.main_scope);
            let _ = systemctl_stop(&m.slice); // belt-and-suspenders
        }
        session_store::remove_marker(&s.id);
        self.refresh_sessions();
        if self.active_ids.is_empty() {
            cleanup_after_kill();
            self.status = format!("Force-killed '{}' (no save sync; cleaned up).", s.name);
        } else {
            self.status = format!("Force-killed '{}' (no save sync).", s.name);
        }
    }

    /// Resolve the real on-disk save path to anchor for a game: a directed absolute
    /// path wins; otherwise auto-discover the Proton compatdata steamuser dir from
    /// the appid and append the handler's declared sub-path. Err = flag in the TUI.
    fn resolve_anchor_path(&self, game: &str) -> Result<String, String> {
        let Some(gi) = self.games.iter().position(|g| g == game) else {
            return Err("unknown game".into());
        };
        // Steam Cloud / Remote Storage games: real save is the userdata remote dir.
        if self.save_cloud[gi] {
            let appid = self.appids[gi].ok_or_else(|| "no steam_appid".to_string())?;
            return crate::platform::find_steam_userdata_remote(appid)
                .map(|p| p.to_string_lossy().into_owned())
                .map_err(|e| e.to_string());
        }
        let sub = self.save_subpaths[gi].clone();
        // A directed absolute/home path is used as-is.
        if sub.starts_with('/') || sub.starts_with('~') || sub.contains("$HOME") {
            return Ok(sub);
        }
        match self.appids[gi] {
            Some(appid) => match crate::platform::find_compat_steamuser(appid) {
                Ok(root) => {
                    let p = if sub.trim().is_empty() {
                        root
                    } else {
                        root.join(sub.trim_start_matches('/'))
                    };
                    Ok(p.to_string_lossy().into_owned())
                }
                Err(e) => Err(format!("appid {appid}: {e}")),
            },
            None => Err("no steam_appid for this game".into()),
        }
    }

    /// After a together launch the detached `splitux launch` writes the invite
    /// link(s) to together-invites.txt asynchronously; poll for it and surface the
    /// URL(s). Gives up after a timeout. Cheap to call every tick.
    /// Tail the detached launch's log into the status bar so Enter→launch gives
    /// real feedback (progress + errors) instead of nothing. Stops on an error
    /// line, once a session is up, or at the deadline.
    fn poll_launch(&mut self) {
        let Some(deadline) = self.launch_deadline else {
            return;
        };
        if Instant::now() > deadline {
            self.launch_log = None;
            self.launch_deadline = None;
            return;
        }
        let Some(path) = &self.launch_log else { return };
        let Ok(content) = std::fs::read_to_string(path) else { return };
        let Some(line) = content.lines().rev().find(|l| !l.trim().is_empty()) else { return };
        let line = line.trim();
        let low = line.to_lowercase();
        if low.contains("error") || low.contains("failed") || low.contains("not found") {
            self.status = format!("⚠ launch failed — {line}");
            self.launch_log = None; // stop watching; surface the error
            self.launch_deadline = None;
        } else {
            // Show the latest progress line (mounting, overlays, proton, etc.).
            // The footer Paragraph wraps, so no manual truncation needed.
            self.status = format!("launching… {line}");
        }
    }

    fn poll_invite(&mut self) {
        let Some(since) = self.awaiting_invite else {
            return;
        };
        if since.elapsed() > Duration::from_secs(25) {
            self.awaiting_invite = None;
            self.status =
                "Together launched, but no invite link appeared (check the launch log).".to_string();
            return;
        }
        let file = crate::paths::PATH_PARTY.join("together-invites.txt");
        let Ok(meta) = std::fs::metadata(&file) else {
            return;
        };
        // Only accept a file written AFTER we launched (ignore a stale one).
        let fresh = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .map(|age| age < since.elapsed())
            .unwrap_or(false);
        if !fresh {
            return;
        }
        let Ok(body) = std::fs::read_to_string(&file) else {
            return;
        };
        let urls: Vec<String> = body
            .lines()
            .map(|l| l.trim())
            .filter(|l| l.starts_with("https://"))
            .map(|l| l.to_string())
            .collect();
        if urls.is_empty() {
            return;
        }
        self.awaiting_invite = None;
        self.invite_url = Some(urls.join("\n"));
        self.status = "Together ready — share the invite link below.".to_string();
    }
}

/// Entry point for the `splitux tui` subcommand.
pub fn run() -> i32 {
    // Query the terminal for its graphics protocol + font size BEFORE entering
    // the alternate screen, so cover art can render inline (kitty/sixel/iterm).
    // Falls back to None on terminals that don't support it — the preview pane
    // then shows a hint instead of an image.
    let picker = Picker::from_query_stdio().ok();
    let mut terminal = ratatui::init();
    // Enable bracketed paste so the inline editors (rename / anchor path) receive
    // pasted text as one Event::Paste instead of dropping it.
    let _ = execute!(std::io::stdout(), EnableBracketedPaste);
    let mut app = App::new();
    app.picker = picker;

    let res = loop {
        app.poll_launch();
        app.poll_invite();
        if let Err(e) = terminal.draw(|f| draw(f, &mut app)) {
            break Err(e);
        }
        // Poll so the sessions view can auto-refresh even without keypresses.
        match event::poll(Duration::from_millis(1000)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => handle_key(&mut app, k.code),
                Ok(Event::Paste(s)) => handle_paste(&mut app, s),
                Ok(_) => {}
                Err(e) => break Err(e),
            },
            Ok(false) => {
                if app.screen == Screen::Sessions
                    && app.last_sessions_refresh.elapsed() >= Duration::from_secs(2)
                {
                    app.refresh_sessions();
                }
            }
            Err(e) => break Err(e),
        }
        if app.quit {
            break Ok(());
        }
    };

    let _ = execute!(std::io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    match res {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[splitux] tui error: {e}");
            1
        }
    }
}

fn handle_key(app: &mut App, code: KeyCode) {
    // Inline text editor (rename / anchor path) captures ALL keys first, so typing
    // 'q' etc. edits text instead of quitting.
    if app.editing.is_some() {
        handle_edit(app, code);
        return;
    }
    // Global
    match code {
        KeyCode::Char('q') => {
            app.quit = true;
            return;
        }
        KeyCode::Char('S') | KeyCode::Tab => {
            // jump to sessions from anywhere
            app.screen = Screen::Sessions;
            app.refresh_sessions();
            return;
        }
        _ => {}
    }

    match app.screen {
        Screen::Games => handle_games(app, code),
        Screen::Build => handle_build(app, code),
        Screen::Sessions => handle_sessions(app, code),
    }
}

fn handle_games(app: &mut App, code: KeyCode) {
    let filtered = app.filtered();
    match code {
        KeyCode::Up => {
            app.game_cursor = app.game_cursor.saturating_sub(1);
        }
        KeyCode::Down => {
            if app.game_cursor + 1 < filtered.len() {
                app.game_cursor += 1;
            }
        }
        KeyCode::Enter => {
            if let Some(&gi) = filtered.get(app.game_cursor) {
                app.game = Some(gi);
                if app.players.is_empty() {
                    let p = app.default_player();
                    app.players.push(p);
                }
                app.player_cursor = 0;
                // Carry forward a prior anchor choice for this game, if any.
                let game = app.games[gi].clone();
                app.build_anchor = session_store::load()
                    .iter()
                    .any(|s| s.game == game && s.anchor.as_ref().map(|a| a.enabled).unwrap_or(false));
                app.screen = Screen::Build;
                app.status =
                    "a add · d del · p profile · i input · t local/together · c save-anchor · Enter launch".into();
            }
        }
        KeyCode::Backspace => {
            app.filter.pop();
            app.game_cursor = 0;
        }
        KeyCode::Char(c) => {
            app.filter.push(c);
            app.game_cursor = 0;
        }
        _ => {}
    }
}

fn handle_build(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.screen = Screen::Games;
            app.status = "Pick a game — type to filter, Enter to select.".into();
        }
        KeyCode::Char('t') => {
            if let Some(pl) = app.players.get_mut(app.player_cursor) {
                pl.together = !pl.together;
                app.status = if pl.together {
                    "Player set to TOGETHER (remote seat — gets an invite link).".into()
                } else {
                    "Player set to LOCAL (drives the host directly).".into()
                };
            } else {
                app.status = "Add a player first (a), then t toggles local/together.".into();
            }
        }
        KeyCode::Char('a') => {
            let p = app.default_player();
            app.players.push(p);
            app.player_cursor = app.players.len() - 1;
        }
        KeyCode::Char('d') => {
            if !app.players.is_empty() {
                app.players.remove(app.player_cursor);
                if app.player_cursor >= app.players.len() {
                    app.player_cursor = app.players.len().saturating_sub(1);
                }
            }
        }
        KeyCode::Up => app.player_cursor = app.player_cursor.saturating_sub(1),
        KeyCode::Down => {
            if app.player_cursor + 1 < app.players.len() {
                app.player_cursor += 1;
            }
        }
        KeyCode::Char('p') => {
            let n = app.profiles.len();
            if let Some(pl) = app.players.get_mut(app.player_cursor) {
                if n > 0 {
                    pl.profile = (pl.profile + 1) % n;
                }
            }
        }
        KeyCode::Char('i') => {
            if let Some(pl) = app.players.get_mut(app.player_cursor) {
                pl.input = pl.input.toggled();
            }
        }
        // Toggle save anchoring for this session (carry the master profile's real
        // save in/out). Part of the session config — persisted on launch.
        KeyCode::Char('c') => app.toggle_build_anchor(),
        KeyCode::Char('s') => {
            app.screen = Screen::Sessions;
            app.refresh_sessions();
        }
        KeyCode::Enter | KeyCode::Char('l') => app.launch(),
        _ => {}
    }
}

fn handle_sessions(app: &mut App, code: KeyCode) {
    let cursor = app.session_cursor;
    let sel = app.sessions.get(cursor).cloned();
    match code {
        KeyCode::Esc => app.screen = if app.game.is_some() { Screen::Build } else { Screen::Games },
        KeyCode::Up => app.session_cursor = app.session_cursor.saturating_sub(1),
        KeyCode::Down => {
            if app.session_cursor + 1 < app.sessions.len() {
                app.session_cursor += 1;
            }
        }
        // Start an inactive session (active = no-op, handled in start_saved).
        KeyCode::Enter | KeyCode::Char('l') => {
            if sel.is_some() {
                app.start_saved(cursor);
            }
        }
        // End & sync (graceful — closes the game, supervisor syncs save back).
        KeyCode::Char('E') => {
            if let Some(s) = &sel {
                if app.active_ids.contains(&s.id) {
                    app.end_and_sync(cursor);
                } else {
                    app.status = format!("'{}' isn't running.", s.name);
                }
            }
        }
        // Force-kill (no sync).
        KeyCode::Char('k') => {
            if let Some(s) = &sel {
                if app.active_ids.contains(&s.id) {
                    app.force_kill(cursor);
                } else {
                    app.status = format!("'{}' isn't running.", s.name);
                }
            }
        }
        // Restart: force-kill if active, then start fresh.
        KeyCode::Char('R') => {
            if let Some(s) = &sel {
                if app.active_ids.contains(&s.id) {
                    app.force_kill(cursor);
                    std::thread::sleep(Duration::from_millis(600));
                }
                app.start_saved(cursor);
                app.status = format!("Restarted '{}'.", s.name);
            }
        }
        // Rename (inline editor).
        KeyCode::Char('r') => {
            if let Some(s) = &sel {
                app.editing = Some(EditState {
                    target: cursor,
                    buf: s.name.clone(),
                });
                app.status = format!("Rename → {} (Enter save · Esc cancel)", s.name);
            }
        }
        // Pin / unpin (protects from the 1-week GC).
        KeyCode::Char('p') => {
            if let Some(s) = &sel {
                let mut sessions = session_store::load();
                if let Some(t) = sessions.iter_mut().find(|x| x.id == s.id) {
                    t.pinned = !t.pinned;
                    let now_pinned = t.pinned;
                    session_store::save(&sessions);
                    app.status = format!(
                        "'{}' {}.",
                        s.name,
                        if now_pinned { "pinned" } else { "unpinned" }
                    );
                }
                app.refresh_sessions();
            }
        }
        // Delete (refused while active).
        KeyCode::Char('d') => {
            if let Some(s) = &sel {
                if app.active_ids.contains(&s.id) {
                    app.status = format!("'{}' is running — End/kill it before deleting.", s.name);
                } else {
                    let mut sessions = session_store::load();
                    sessions.retain(|x| x.id != s.id);
                    session_store::save(&sessions);
                    app.status = format!("Deleted '{}'.", s.name);
                    app.refresh_sessions();
                }
            }
        }
        // Edit → load this session's config back into the Build screen.
        KeyCode::Char('e') => {
            if let Some(s) = &sel {
                if let Some(gi) = app.games.iter().position(|g| g == &s.game) {
                    app.game = Some(gi);
                    app.players = from_saved_players(&s.players, &app.profiles);
                    app.player_cursor = 0;
                    app.build_anchor = s.anchor.as_ref().map(|a| a.enabled).unwrap_or(false);
                    app.screen = Screen::Build;
                    app.status = format!("Editing '{}' — tweak and Enter to launch.", s.name);
                } else {
                    app.status = format!("Game '{}' not installed/handler missing.", s.game);
                }
            }
        }
        // (Save anchoring is configured in the Build screen, not here — it's part
        // of a session's config. The badge ⚓/⚑ on each row reflects it.)
        // Kill-all (global safety net).
        KeyCode::Char('K') => {
            stop_all_sessions();
            app.refresh_sessions();
            app.status =
                "Stopped ALL sessions (no sync; units, fuse mounts, seat-streamers, bars restored)."
                    .into();
        }
        _ => {}
    }
}

/// Pasted text (bracketed paste). Routes into the active inline editor, or the
/// game filter on the Games screen. Newlines are stripped (these are single-line).
fn handle_paste(app: &mut App, data: String) {
    let data: String = data.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    if let Some(ed) = app.editing.as_mut() {
        ed.buf.push_str(&data);
    } else if app.screen == Screen::Games {
        app.filter.push_str(&data);
        app.game_cursor = 0;
    }
}

/// Drive the inline text editor (rename / anchor path). Enter commits, Esc cancels.
fn handle_edit(app: &mut App, code: KeyCode) {
    let Some(ed) = app.editing.as_mut() else { return };
    match code {
        KeyCode::Esc => {
            app.editing = None;
            app.status = "Cancelled.".into();
        }
        KeyCode::Enter => commit_edit(app),
        KeyCode::Backspace => {
            ed.buf.pop();
        }
        KeyCode::Char(c) => ed.buf.push(c),
        _ => {}
    }
}

/// Persist the inline rename to the store.
fn commit_edit(app: &mut App) {
    let Some(ed) = app.editing.take() else { return };
    let Some(sel) = app.sessions.get(ed.target).cloned() else { return };
    let mut sessions = session_store::load();
    let Some(target) = sessions.iter_mut().find(|x| x.id == sel.id) else {
        app.refresh_sessions();
        return;
    };
    let name = ed.buf.trim();
    if !name.is_empty() {
        target.name = name.to_string();
        app.status = format!("Renamed → {}", target.name);
        session_store::save(&sessions);
    }
    app.refresh_sessions();
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn draw(f: &mut Frame, app: &mut App) {
    let has_invite = app.invite_url.is_some();
    let mut constraints = vec![
        Constraint::Length(3), // title
        Constraint::Min(5),    // body
    ];
    if has_invite {
        constraints.push(Constraint::Length(4)); // invite box
    }
    constraints.push(Constraint::Length(3)); // footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.area());

    draw_title(f, chunks[0], app);
    match app.screen {
        Screen::Games => draw_games(f, chunks[1], app),
        Screen::Build => draw_build(f, chunks[1], app),
        Screen::Sessions => draw_sessions(f, chunks[1], app),
    }
    let mut idx = 2;
    if has_invite {
        draw_invite(f, chunks[idx], app);
        idx += 1;
    }
    draw_footer(f, chunks[idx], app);
}

fn draw_invite(f: &mut Frame, area: Rect, app: &App) {
    let url = app.invite_url.as_deref().unwrap_or("");
    let p = Paragraph::new(url)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Together invite — share this link (select to copy) "),
        )
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn draw_title(f: &mut Frame, area: Rect, app: &App) {
    let tab = |name: &str, active: bool| {
        if active {
            Span::styled(
                format!(" {name} "),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(format!(" {name} "), Style::default().fg(Color::DarkGray))
        }
    };
    let line = Line::from(vec![
        Span::styled("  splitux ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("│ "),
        tab("Games", app.screen == Screen::Games),
        Span::raw(" "),
        tab("Build", app.screen == Screen::Build),
        Span::raw(" "),
        tab("Sessions", app.screen == Screen::Sessions),
        Span::raw(format!("   ({} active)", app.sessions.len())),
    ]);
    f.render_widget(
        Paragraph::new(line).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn draw_games(f: &mut Frame, area: Rect, app: &mut App) {
    // Split: game list on the left, cover-art preview on the right.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    let filtered = app.filtered();
    // When the terminal can draw images, pad each row so a small per-game icon
    // can be overlaid in the gap (after the highlight gutter).
    let icons_on = app.picker.is_some();
    let pad = if icons_on { "   " } else { "" };
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|&i| ListItem::new(format!("{pad}{}", app.games[i])))
        .collect();
    let title = if app.filter.is_empty() {
        format!(" Games ({}) ", app.games.len())
    } else {
        format!(" Games  filter: {} ", app.filter)
    };
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    let mut state = ListState::default();
    if !filtered.is_empty() {
        state.select(Some(app.game_cursor.min(filtered.len() - 1)));
    }
    f.render_stateful_widget(list, cols[0], &mut state);

    // Overlay a small icon on each visible row, in the padding after the
    // 2-cell highlight gutter. Protocols are built lazily and cached per game.
    if icons_on {
        let inner = Block::default().borders(Borders::ALL).inner(cols[0]);
        let (gutter, icon_w) = (2u16, 2u16);
        if inner.width > gutter + icon_w {
            let offset = state.offset();
            for row in 0..inner.height as usize {
                let fi = offset + row;
                if fi >= filtered.len() {
                    break;
                }
                let gi = filtered[fi];
                if !app.icon_protos.contains_key(&gi) {
                    let built = match (&app.picker, app.icons.get(gi).and_then(|o| o.as_ref())) {
                        (Some(picker), Some(path)) => image::ImageReader::open(path)
                            .ok()
                            .and_then(|r| r.decode().ok())
                            .map(|im| picker.new_resize_protocol(im)),
                        _ => None,
                    };
                    if let Some(p) = built {
                        app.icon_protos.insert(gi, p);
                    }
                }
                if let Some(proto) = app.icon_protos.get_mut(&gi) {
                    let rect = Rect {
                        x: inner.x + gutter,
                        y: inner.y + row as u16,
                        width: icon_w,
                        height: 1,
                    };
                    f.render_stateful_widget(StatefulImage::default(), rect, proto);
                }
            }
        }
    }

    draw_game_preview(f, cols[1], app);
}

/// Cover-art preview for the highlighted game. Loads the cover lazily and caches
/// the encoded protocol, rebuilding only when the selection changes. Degrades to
/// a text hint when the game has no art or the terminal can't render images.
fn draw_game_preview(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default().borders(Borders::ALL).title(" Preview ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let filtered = app.filtered();
    if filtered.is_empty() {
        app.cover_proto = None;
        return;
    }
    let gi = filtered[app.game_cursor.min(filtered.len() - 1)];
    let info = app.infos.get(gi).cloned().unwrap_or_default();

    // Split the pane: cover art on top, the handler's Play/Avoid text below. The
    // text gets just the rows it needs (capped to half the pane); the cover takes
    // the rest. With no art the cover area shows a short hint, so the text always
    // shows regardless of whether art is available.
    let text_rows = if info.is_empty() {
        0
    } else {
        ((info.lines().count() as u16) + 1).min(inner.height / 2)
    };
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(text_rows)])
        .split(inner);
    let art_area = parts[0];
    let text_area = parts[1];

    let hint = |f: &mut Frame, msg: &str| {
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(Color::DarkGray)).wrap(Wrap { trim: true }),
            art_area,
        );
    };

    // Cover art (or a hint) in the top area.
    let cover = app.covers.get(gi).cloned().flatten();
    if app.picker.is_none() {
        app.cover_proto = None;
        hint(f, "(no inline images in this terminal — try kitty)");
    } else if let Some(path) = cover {
        let need = !matches!(&app.cover_proto, Some((idx, _)) if *idx == gi);
        if need {
            let proto = app.picker.as_ref().and_then(|picker| {
                image::ImageReader::open(&path).ok().and_then(|r| r.decode().ok())
                    .map(|img| picker.new_resize_protocol(img))
            });
            app.cover_proto = proto.map(|p| (gi, p));
        }
        match &mut app.cover_proto {
            Some((_, proto)) => f.render_stateful_widget(StatefulImage::default(), art_area, proto),
            None => hint(f, "Couldn't load this game's cover image."),
        }
    } else {
        app.cover_proto = None;
        hint(f, "No cover art yet for this game.");
    }

    // Play / Avoid text in the bottom area.
    if !info.is_empty() {
        f.render_widget(
            Paragraph::new(info).wrap(Wrap { trim: true }),
            text_area,
        );
    }
}

fn draw_build(f: &mut Frame, area: Rect, app: &mut App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // left: summary
    let game = app.game.map(|i| app.games[i].as_str()).unwrap_or("(none)");
    let n_together = app.players.iter().filter(|p| p.together).count();
    let n_local = app.players.len() - n_together;
    // Save-anchor summary line (config, set with `c`).
    let save_line = if app.build_anchor {
        let master = app
            .players
            .iter()
            .filter_map(|p| app.profiles.get(p.profile))
            .find(|p| !p.eq_ignore_ascii_case("Guest"))
            .map(|s| s.as_str())
            .unwrap_or("?");
        Line::from(vec![
            Span::raw("Save:  "),
            Span::styled(
                format!("⚓ anchored — master {master}"),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(vec![
            Span::raw("Save:  "),
            Span::styled("fresh (press c to anchor real save)", Style::default().fg(Color::DarkGray)),
        ])
    };
    let lines = vec![
        Line::from(vec![Span::raw("Game:  "), Span::styled(game, Style::default().add_modifier(Modifier::BOLD))]),
        Line::from(vec![
            Span::raw("Mix:   "),
            Span::styled(format!("{n_local} local"), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" + "),
            Span::styled(format!("{n_together} together"), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        ]),
        save_line,
        Line::from(""),
        Line::from(Span::styled(
            "Per player: t = local/together, i = kb-m/gamepad.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "Local drives the host; together is a remote seat",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "with an invite link. Mix freely.",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title(" Session ")),
        cols[0],
    );

    // right: players
    let items: Vec<ListItem> = app
        .players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let prof = app.profiles.get(p.profile).map(|s| s.as_str()).unwrap_or("?");
            let scope = if p.together { "together" } else { "local" };
            let line = format!("P{}  {:<12}  [{:<8} {}]", i + 1, prof, scope, p.input.label());
            ListItem::new(line)
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Players ({}) ", app.players.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    let mut state = ListState::default();
    if !app.players.is_empty() {
        state.select(Some(app.player_cursor.min(app.players.len() - 1)));
    }
    f.render_stateful_widget(list, cols[1], &mut state);
}

fn draw_sessions(f: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = if app.sessions.is_empty() {
        vec![ListItem::new(Span::styled(
            "  (no saved sessions yet — launch one from a game's Build screen)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        app.sessions
            .iter()
            .map(|s| {
                let active = app.active_ids.contains(&s.id);
                let (dot, dot_col) = if active {
                    ("●", Color::Green)
                } else {
                    ("○", Color::DarkGray)
                };
                let players = s
                    .players
                    .iter()
                    .map(|p| {
                        let scope = if p.together { "together" } else { "local" };
                        format!("{}·{}", p.profile, scope)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                // Anchor badge: ⚓ resolved, ⚑ enabled-but-unresolved.
                let anchor = match &s.anchor {
                    Some(a) if a.enabled && !a.save_path.is_empty() => " ⚓",
                    Some(a) if a.enabled => " ⚑",
                    _ => "",
                };
                let pin = if s.pinned { " ★" } else { "" };
                let mut spans = vec![
                    Span::styled(format!("{dot} "), Style::default().fg(dot_col)),
                    Span::styled(
                        s.name.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ];
                if !anchor.is_empty() {
                    spans.push(Span::styled(anchor, Style::default().fg(Color::Cyan)));
                }
                if !pin.is_empty() {
                    spans.push(Span::styled(pin, Style::default().fg(Color::Yellow)));
                }
                spans.push(Span::styled(
                    format!("  {} · {}", players, rel_time(s.last_used)),
                    Style::default().fg(Color::DarkGray),
                ));
                ListItem::new(Line::from(spans))
            })
            .collect()
    };
    let active_n = app.active_ids.len();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Sessions ({} · {active_n} active) ", app.sessions.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    let mut state = ListState::default();
    if !app.sessions.is_empty() {
        state.select(Some(app.session_cursor.min(app.sessions.len() - 1)));
    }
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let keys = if app.editing.is_some() {
        "type to edit · Enter save · Esc cancel"
    } else {
        match app.screen {
            Screen::Games => "↑↓ move · type filter · Enter select · S sessions · q quit",
            Screen::Build => {
                "↑↓ player · a add · d del · p profile · i input · t local/together · c save-anchor · Enter launch · s sessions · Esc back"
            }
            Screen::Sessions => {
                "↑↓ · Enter start · E end&sync · k force-kill · R restart · r rename · p pin · d del · e edit · K kill-all · Esc back"
            }
        }
    };
    // While editing, show the live buffer with a cursor instead of the static line.
    let status_line = if let Some(ed) = &app.editing {
        format!("{}_", ed.buf)
    } else {
        app.status.clone()
    };
    let text = vec![
        Line::from(Span::styled(keys, Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(status_line, Style::default().fg(Color::White))),
    ];
    f.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

// ---------------------------------------------------------------------------
// Process / systemd glue
// ---------------------------------------------------------------------------

/// Build the `splitux launch` arg vector for a session, including save-anchor
/// overrides when the Session has an enabled anchor.
fn launch_args(
    game: &str,
    players: &[Player],
    profiles: &[String],
    anchor: Option<&SaveAnchor>,
) -> Vec<String> {
    let mut args = vec!["launch".to_string(), "--game".to_string(), game.to_string()];
    for p in players {
        let prof = profiles.get(p.profile).cloned().unwrap_or_else(|| "Guest".into());
        let input = p.input.spec(p.together);
        args.push("--player".to_string());
        args.push(format!("profile={prof},input={input}"));
    }
    if let Some(a) = anchor.filter(|a| a.enabled) {
        if !a.master_profile.is_empty() {
            args.push("--master".to_string());
            args.push(a.master_profile.clone());
        }
        if !a.save_path.is_empty() {
            args.push("--save-anchor".to_string());
            args.push(a.save_path.clone());
        }
        args.push("--save-sync-back".to_string());
        if a.steam_id_remap {
            args.push("--save-steam-id-remap".to_string());
        }
    }
    args
}

/// Spawn `splitux launch ...` fully detached (new session via setsid), with its
/// output going to a log file so the TUI keeps the terminal. `run_session`
/// blocks for the session's lifetime, so it must not be a child we wait on.
fn spawn_session(
    game: &str,
    players: &[Player],
    profiles: &[String],
    anchor: Option<&SaveAnchor>,
    session_id: &str,
) -> std::io::Result<(PathBuf, u32)> {
    let exe = std::env::current_exe()?;
    let args = launch_args(game, players, profiles, anchor);
    let log_path = std::env::temp_dir().join(format!(
        "splitux-tui-{}.log",
        game.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "-")
    ));
    let log = std::fs::File::create(&log_path)?;
    let log_err = log.try_clone()?;

    let mut cmd = Command::new("setsid");
    cmd.arg(exe)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        // Tag the launch so its supervisor writes a runtime marker the TUI uses to
        // show this session ● active and target End/Kill at its exact units.
        .env(session_store::SESSION_ID_ENV, session_id)
        // Drop a dev GST_PLUGIN_PATH so the seat-streamer finds splitux's own
        // plugin dir (splitux sets GST_PLUGIN_PATH itself for together).
        .env_remove("GST_PLUGIN_PATH");
    let child = cmd.spawn()?;
    Ok((log_path, child.id()))
}

/// Names of currently-active splitux systemd user units (slices + scopes). Used to
/// decide which Sessions are live (their marker's main scope appears here).
fn scan_active_units() -> Vec<String> {
    let out = Command::new("systemctl")
        .args([
            "--user",
            "list-units",
            "--no-legend",
            "--plain",
            "--state=active",
            "splitux*",
        ])
        .output();
    let Ok(out) = out else { return Vec::new() };
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|u| u.ends_with(".slice") || u.ends_with(".scope"))
        .map(|u| u.to_string())
        .collect()
}

fn systemctl_stop(unit: &str) -> std::io::Result<()> {
    Command::new("systemctl")
        .args(["--user", "stop", unit])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|_| ())
}

/// Cleanup the force-killed launch supervisor never ran itself: unmount stale
/// fuse-overlayfs game dirs (else the NEXT launch fails to mount — the bug that
/// stranded a relaunch) and reap any orphaned seat-streamers left behind. Safe
/// to call only when no session should remain (it's global).
fn cleanup_after_kill() {
    if let Err(e) = crate::util::fuse_overlayfs_unmount_gamedirs() {
        eprintln!("[splitux] tui - fuse unmount during kill: {e}");
    }
    let _ = Command::new("pkill")
        .args(["-f", "share/splitux/bin/seat-streamer"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    // Restore the host status bars (waybar etc.). The launch supervisor hid them
    // and armed a restore-on-death watcher, but a TUI `systemctl stop` kills the
    // supervisor mid-session so it never runs its own `restore_all`, and the
    // watcher doesn't reliably fire — so the bars stayed hidden after a kill.
    // This reads the persisted bar state and restarts any bar not already running
    // (idempotent: clears the state file, skips bars still up), so killing the
    // last session always brings the desktop back.
    crate::wm::bars::restore_from_previous_session();
}

fn stop_all_sessions() {
    // Stopping the umbrella slice tears down every per-session child...
    let _ = systemctl_stop("splitux.slice");
    for unit in scan_active_units() {
        let _ = systemctl_stop(&unit);
    }
    // ...drop every runtime marker (these are hard kills — no sync)...
    for m in session_store::list_markers() {
        session_store::remove_marker(&m.session_id);
    }
    // ...then run the teardown the killed supervisors skipped, so no stale fuse
    // mounts or orphaned seat-streamers block the next launch.
    cleanup_after_kill();
}
