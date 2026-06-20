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

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
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

struct Session {
    unit: String,
    active: String,
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

    // sessions
    sessions: Vec<Session>,
    session_cursor: usize,
    last_sessions_refresh: Instant,

    // remember last launch for restart
    last_launch: Option<(usize, Vec<Player>)>,

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
            icons,
            icon_protos: HashMap::new(),
            picker: None,
            cover_proto: None,
            game: None,
            players: Vec::new(),
            player_cursor: 0,
            sessions: Vec::new(),
            session_cursor: 0,
            last_sessions_refresh: Instant::now(),
            last_launch: None,
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

    fn refresh_sessions(&mut self) {
        self.sessions = scan_sessions();
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

    fn launch(&mut self) {
        let Some(gi) = self.game else { return };
        if self.players.is_empty() {
            self.status = "Add at least one player (a) before launching.".to_string();
            return;
        }
        let n_together = self.players.iter().filter(|p| p.together).count();
        match spawn_session(&self.games[gi], &self.players, &self.profiles) {
            Ok(log) => {
                self.last_launch = Some((gi, self.players.clone()));
                // Watch the detached launch's log so the user gets live feedback.
                self.launch_log = Some(log);
                self.launch_deadline = Some(Instant::now() + Duration::from_secs(30));
                let n_local = self.players.len() - n_together;
                self.status = format!(
                    "Launched '{}' ({} local + {} together). Press s for sessions.",
                    self.games[gi], n_local, n_together
                );
                if n_together > 0 {
                    self.invite_url = None;
                    self.awaiting_invite = Some(Instant::now());
                    self.status = format!(
                        "Launched '{}' — fetching invite link(s) for {} together seat(s)…",
                        self.games[gi], n_together
                    );
                }
                self.refresh_sessions();
            }
            Err(e) => self.status = format!("Launch failed: {e}"),
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
                app.screen = Screen::Build;
                app.status =
                    "a add · d del · p profile · i input · t local/together · Enter launch".into();
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
        KeyCode::Char('s') => {
            app.screen = Screen::Sessions;
            app.refresh_sessions();
        }
        KeyCode::Enter | KeyCode::Char('l') => app.launch(),
        _ => {}
    }
}

fn handle_sessions(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => app.screen = if app.game.is_some() { Screen::Build } else { Screen::Games },
        KeyCode::Char('r') => app.refresh_sessions(),
        KeyCode::Up => app.session_cursor = app.session_cursor.saturating_sub(1),
        KeyCode::Down => {
            if app.session_cursor + 1 < app.sessions.len() {
                app.session_cursor += 1;
            }
        }
        KeyCode::Char('k') => {
            if let Some(s) = app.sessions.get(app.session_cursor) {
                let unit = s.unit.clone();
                let _ = systemctl_stop(&unit);
                app.refresh_sessions();
                // If that was the last session, run the same cleanup as kill-all
                // so no stale fuse mount / orphaned seat-streamer is left behind.
                if app.sessions.is_empty() {
                    cleanup_after_kill();
                    app.status = format!("Stopped {unit} (cleaned up — no sessions left).");
                } else {
                    app.status = format!("Stopped {unit}.");
                }
            }
        }
        KeyCode::Char('K') => {
            stop_all_sessions();
            app.refresh_sessions();
            app.status = "Stopped all sessions cleanly (units, fuse mounts, seat-streamers).".into();
        }
        KeyCode::Char('R') => {
            // restart the last-launched config
            if let Some((gi, players)) = app.last_launch.clone() {
                stop_all_sessions();
                std::thread::sleep(Duration::from_millis(500));
                match spawn_session(&app.games[gi], &players, &app.profiles) {
                    Ok(_) => app.status = format!("Restarted '{}'.", app.games[gi]),
                    Err(e) => app.status = format!("Restart failed: {e}"),
                }
                app.refresh_sessions();
            } else {
                app.status = "No previous launch to restart.".into();
            }
        }
        _ => {}
    }
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
    let lines = vec![
        Line::from(vec![Span::raw("Game:  "), Span::styled(game, Style::default().add_modifier(Modifier::BOLD))]),
        Line::from(vec![
            Span::raw("Mix:   "),
            Span::styled(format!("{n_local} local"), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" + "),
            Span::styled(format!("{n_together} together"), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        ]),
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
            "  (no active splitux sessions)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        app.sessions
            .iter()
            .map(|s| {
                let color = if s.active == "active" { Color::Green } else { Color::Yellow };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{:<10}", s.active), Style::default().fg(color)),
                    Span::raw(s.unit.clone()),
                ]))
            })
            .collect()
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Sessions ({}) ", app.sessions.len())),
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
    let keys = match app.screen {
        Screen::Games => "↑↓ move · type filter · Enter select · S sessions · q quit",
        Screen::Build => {
            "↑↓ player · a add · d del · p profile · i input · t local/together · Enter launch · s/Tab sessions · Esc back · q quit"
        }
        Screen::Sessions => "↑↓ move · k kill · K kill-all · R restart-last · r refresh · Esc back · q quit",
    };
    let text = vec![
        Line::from(Span::styled(keys, Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(app.status.clone(), Style::default().fg(Color::White))),
    ];
    f.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

// ---------------------------------------------------------------------------
// Process / systemd glue
// ---------------------------------------------------------------------------

/// Build the `splitux launch` arg vector for a session.
fn launch_args(game: &str, players: &[Player], profiles: &[String]) -> Vec<String> {
    let mut args = vec!["launch".to_string(), "--game".to_string(), game.to_string()];
    for p in players {
        let prof = profiles.get(p.profile).cloned().unwrap_or_else(|| "Guest".into());
        let input = p.input.spec(p.together);
        args.push("--player".to_string());
        args.push(format!("profile={prof},input={input}"));
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
) -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let args = launch_args(game, players, profiles);
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
        // Drop a dev GST_PLUGIN_PATH so the seat-streamer finds splitux's own
        // plugin dir (splitux sets GST_PLUGIN_PATH itself for together).
        .env_remove("GST_PLUGIN_PATH");
    cmd.spawn().map(|_| log_path)
}

/// Enumerate active splitux systemd user units (the launch self-scopes into a
/// `splitux-*.slice` / `.scope`).
fn scan_sessions() -> Vec<Session> {
    let out = Command::new("systemctl")
        .args([
            "--user",
            "list-units",
            "--no-legend",
            "--plain",
            "--all",
            "splitux*",
        ])
        .output();
    let Ok(out) = out else { return Vec::new() };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut sessions = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 3 {
            continue;
        }
        let unit = f[0];
        if !(unit.ends_with(".slice") || unit.ends_with(".scope")) {
            continue;
        }
        // Skip the umbrella slice; show the per-session ones.
        if unit == "splitux.slice" {
            continue;
        }
        sessions.push(Session {
            unit: unit.to_string(),
            active: f[2].to_string(),
        });
    }
    sessions
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
}

fn stop_all_sessions() {
    // Stopping the umbrella slice tears down every per-session child...
    let _ = systemctl_stop("splitux.slice");
    for s in scan_sessions() {
        let _ = systemctl_stop(&s.unit);
    }
    // ...then run the teardown the killed supervisors skipped, so no stale fuse
    // mounts or orphaned seat-streamers block the next launch.
    cleanup_after_kill();
}
