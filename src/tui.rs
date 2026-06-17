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

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

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
    /// The `input=` value for `splitux launch` in Local (non-together) mode.
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

    // build screen
    game: Option<usize>, // index into games
    together: bool,
    players: Vec<Player>,
    player_cursor: usize,

    // sessions
    sessions: Vec<Session>,
    session_cursor: usize,
    last_sessions_refresh: Instant,

    // remember last launch for restart
    last_launch: Option<(usize, bool, Vec<Player>)>,

    status: String,
    quit: bool,
}

impl App {
    fn new() -> Self {
        let games: Vec<String> = scan_handlers()
            .into_iter()
            .map(|h| h.display().to_string())
            .collect();
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
            game: None,
            together: false,
            players: Vec::new(),
            player_cursor: 0,
            sessions: Vec::new(),
            session_cursor: 0,
            last_sessions_refresh: Instant::now(),
            last_launch: None,
            status: "Pick a game — type to filter, Enter to select.".to_string(),
            quit: false,
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
        }
    }

    fn launch(&mut self) {
        let Some(gi) = self.game else { return };
        if self.players.is_empty() {
            self.status = "Add at least one player (a) before launching.".to_string();
            return;
        }
        match spawn_session(&self.games[gi], self.together, &self.players, &self.profiles) {
            Ok(()) => {
                self.last_launch = Some((gi, self.together, self.players.clone()));
                self.status = format!(
                    "Launched '{}' ({} player(s), {}). Press s for sessions.",
                    self.games[gi],
                    self.players.len(),
                    if self.together { "together" } else { "local" }
                );
                self.refresh_sessions();
            }
            Err(e) => self.status = format!("Launch failed: {e}"),
        }
    }
}

/// Entry point for the `splitux tui` subcommand.
pub fn run() -> i32 {
    let mut terminal = ratatui::init();
    let mut app = App::new();

    let res = loop {
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
            app.together = !app.together;
            app.status = if app.together {
                "Together: each seat gets both kb/m + gamepad (per-player input hidden).".into()
            } else {
                "Local: assign an input per player.".into()
            };
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
            if !app.together {
                if let Some(pl) = app.players.get_mut(app.player_cursor) {
                    pl.input = pl.input.toggled();
                }
            }
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
                app.status = format!("Stopped {unit}");
                app.refresh_sessions();
            }
        }
        KeyCode::Char('K') => {
            stop_all_sessions();
            app.status = "Stopped all splitux sessions.".into();
            app.refresh_sessions();
        }
        KeyCode::Char('R') => {
            // restart the last-launched config
            if let Some((gi, together, players)) = app.last_launch.clone() {
                stop_all_sessions();
                std::thread::sleep(Duration::from_millis(500));
                match spawn_session(&app.games[gi], together, &players, &app.profiles) {
                    Ok(()) => app.status = format!("Restarted '{}'.", app.games[gi]),
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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Min(5),    // body
            Constraint::Length(3), // footer
        ])
        .split(f.area());

    draw_title(f, chunks[0], app);
    match app.screen {
        Screen::Games => draw_games(f, chunks[1], app),
        Screen::Build => draw_build(f, chunks[1], app),
        Screen::Sessions => draw_sessions(f, chunks[1], app),
    }
    draw_footer(f, chunks[2], app);
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
    let filtered = app.filtered();
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|&i| ListItem::new(app.games[i].clone()))
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
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_build(f: &mut Frame, area: Rect, app: &mut App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // left: summary
    let game = app.game.map(|i| app.games[i].as_str()).unwrap_or("(none)");
    let mode = if app.together {
        Span::styled("TOGETHER", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("LOCAL", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    };
    let mut lines = vec![
        Line::from(vec![Span::raw("Game:  "), Span::styled(game, Style::default().add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::raw("Mode:  "), mode, Span::raw("   (t to toggle)")]),
        Line::from(""),
    ];
    if app.together {
        lines.push(Line::from(Span::styled(
            "Together streams kb/m + gamepad to every seat,",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "so per-player input is not assigned.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "Local: each player drives the host via",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "its assigned input (i to toggle).",
            Style::default().fg(Color::DarkGray),
        )));
    }
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
            let line = if app.together {
                format!("P{}  {}", i + 1, prof)
            } else {
                format!("P{}  {:<12}  [{}]", i + 1, prof, p.input.label())
            };
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
            "↑↓ player · a add · d del · p profile · i input · t local/together · Enter launch · Esc back · q quit"
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
fn launch_args(game: &str, together: bool, players: &[Player], profiles: &[String]) -> Vec<String> {
    let mut args = vec!["launch".to_string(), "--game".to_string(), game.to_string()];
    for p in players {
        let prof = profiles.get(p.profile).cloned().unwrap_or_else(|| "Guest".into());
        let input = if together {
            "together:gamepad".to_string()
        } else {
            p.input.local_spec().to_string()
        };
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
    together: bool,
    players: &[Player],
    profiles: &[String],
) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let args = launch_args(game, together, players, profiles);
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
    cmd.spawn().map(|_| ())
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

fn stop_all_sessions() {
    // Stopping the umbrella slice tears down every per-session child.
    let _ = systemctl_stop("splitux.slice");
    for s in scan_sessions() {
        let _ = systemctl_stop(&s.unit);
    }
}
