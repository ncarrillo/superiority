use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::{self, Stdout},
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event as TerminalEvent, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::{
    error::Result,
    event::{ActivityKind, Event},
    web_auth,
};

const BACKGROUND: Color = Color::Rgb(1, 8, 16);
const PANEL: Color = Color::Rgb(3, 14, 25);
const BORDER_MUTED: Color = Color::Rgb(15, 49, 70);
const CYAN: Color = Color::Rgb(62, 190, 244);
const CYAN_SOFT: Color = Color::Rgb(104, 188, 220);
const TEXT: Color = Color::Rgb(218, 229, 242);
const MUTED: Color = Color::Rgb(116, 143, 168);
const FAINT: Color = Color::Rgb(48, 70, 89);
const GREEN: Color = Color::Rgb(54, 218, 147);
const AMBER: Color = Color::Rgb(232, 169, 64);
const RED: Color = Color::Rgb(242, 91, 99);
const MAX_ACTIVITY: usize = 500;

struct Activity {
    elapsed: Duration,
    kind: ActivityKind,
    message: String,
}

#[derive(Default)]
struct UpstreamState {
    toon: String,
    channel: Option<String>,
    roster: Vec<String>,
    friends: usize,
}

struct App {
    started: Instant,
    status: String,
    listener: Option<SocketAddr>,
    upstreams: BTreeMap<SocketAddr, UpstreamState>,
    legacy_peers: BTreeSet<SocketAddr>,
    selected_peer: Option<SocketAddr>,
    activity: VecDeque<Activity>,
    stopping: bool,
    failure: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            status: "starting".into(),
            listener: None,
            upstreams: BTreeMap::new(),
            legacy_peers: BTreeSet::new(),
            selected_peer: None,
            activity: VecDeque::new(),
            stopping: false,
            failure: None,
        }
    }
}

impl App {
    fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Status(status) => self.status = status,
            Event::Listening(address) => {
                self.listener = Some(address);
                self.status = "ready for legacy bots".into();
            }
            Event::Upstream {
                peer,
                toon,
                channel,
                mut roster,
                friends,
            } => {
                roster.sort_unstable_by_key(|name| name.to_ascii_lowercase());
                self.upstreams.insert(
                    peer,
                    UpstreamState {
                        toon,
                        channel,
                        roster,
                        friends,
                    },
                );
                if self.selected_peer.is_none() {
                    self.selected_peer = Some(peer);
                }
            }
            Event::LegacyConnected(address) => {
                self.legacy_peers.insert(address);
                self.selected_peer = Some(address);
                self.update_connection_status();
            }
            Event::LegacyDisconnected(address) => {
                self.legacy_peers.remove(&address);
                self.upstreams.remove(&address);
                if self.selected_peer == Some(address) {
                    self.selected_peer = self.legacy_peers.iter().next().copied();
                }
                self.update_connection_status();
            }
            Event::Activity { kind, message } => self.push_activity(kind, message),
            Event::Finished(failure) => {
                self.failure = failure;
                return true;
            }
        }
        false
    }

    fn push_activity(&mut self, kind: ActivityKind, message: String) {
        if self.activity.len() == MAX_ACTIVITY {
            self.activity.pop_front();
        }
        self.activity.push_back(Activity {
            elapsed: self.started.elapsed(),
            kind,
            message,
        });
    }

    fn update_connection_status(&mut self) {
        self.status = match self.legacy_peers.len() {
            0 => "ready for legacy bots".into(),
            1 => "bridging 1 legacy client".into(),
            count => format!("bridging {count} legacy clients"),
        };
    }

    fn selected_upstream(&self) -> Option<&UpstreamState> {
        self.selected_peer
            .and_then(|peer| self.upstreams.get(&peer))
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<(Self, Terminal<CrosstermBackend<Stdout>>)> {
        enable_raw_mode()?;
        let mut output = io::stdout();
        if let Err(error) = execute!(output, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        let terminal = Terminal::new(CrosstermBackend::new(output))?;
        Ok((Self, terminal))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    }
}

pub fn run(
    events: &Receiver<Event>,
    authentication: &Receiver<web_auth::Request>,
    stop: &Arc<AtomicBool>,
) -> Result<()> {
    let (_guard, mut terminal) = TerminalGuard::enter()?;
    let mut app = App::default();
    loop {
        web_auth::process_pending(authentication);
        loop {
            match events.try_recv() {
                Ok(message) => {
                    if app.apply(message) {
                        return Ok(());
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }
        if stop.load(Ordering::Acquire) && !app.stopping {
            app.stopping = true;
            app.status = "stopping".into();
        }
        terminal.draw(|frame| draw(frame, &app))?;
        if event::poll(Duration::from_millis(50))?
            && let TerminalEvent::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && (matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                || key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            stop.store(true, Ordering::Release);
            app.stopping = true;
            app.status = "stopping".into();
        }
    }
}

pub fn run_headless(events: &Receiver<Event>, authentication: &Receiver<web_auth::Request>) {
    loop {
        web_auth::process_pending(authentication);
        let event = match events.recv_timeout(Duration::from_millis(50)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match event {
            Event::Status(status) => eprintln!("status: {status}"),
            Event::Listening(address) => eprintln!("listening: {address}"),
            Event::Upstream {
                peer,
                toon,
                channel,
                roster,
                friends,
            } => eprintln!(
                "upstream {peer}: {toon} in {} ({} users, {friends} friends)",
                channel.as_deref().unwrap_or("no channel"),
                roster.len()
            ),
            Event::LegacyConnected(address) => eprintln!("legacy bot connected: {address}"),
            Event::LegacyDisconnected(address) => {
                eprintln!("legacy bot disconnected: {address}");
            }
            Event::Activity { kind, message } => {
                eprintln!("{}: {message}", activity_label(kind));
            }
            Event::Finished(failure) => {
                if let Some(failure) = failure {
                    eprintln!("failed: {failure}");
                }
                break;
            }
        }
    }
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &App) {
    frame.render_widget(
        Block::default().style(Style::default().bg(BACKGROUND)),
        frame.area(),
    );
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());
    draw_header(frame, app, areas[0]);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(areas[1]);
    draw_sidebar(frame, app, body[0]);
    draw_activity(frame, app, body[1]);
    draw_footer(frame, app, areas[2]);
}

fn draw_header(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let color = if app.failure.is_some() {
        RED
    } else if app.stopping || app.listener.is_none() {
        AMBER
    } else {
        GREEN
    };
    let listener = app
        .listener
        .map_or_else(|| "listener pending".into(), |address| address.to_string());
    let line = Line::from(vec![
        Span::styled(" ● ", Style::default().fg(color)),
        Span::styled(&app.status, Style::default().fg(TEXT)),
        Span::styled("  │  ", Style::default().fg(BORDER_MUTED)),
        Span::styled(listener, Style::default().fg(MUTED)),
    ]);
    frame.render_widget(
        Paragraph::new(line)
            .style(Style::default().bg(PANEL))
            .block(
                Block::default()
                    .title(Line::from(Span::styled(
                        " gateway ",
                        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                    )))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_MUTED))
                    .style(Style::default().bg(PANEL)),
            ),
        area,
    );
}

fn draw_sidebar(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(4)])
        .split(area);
    let upstream = app.selected_upstream();
    let toon = upstream.map_or("authenticating", |state| state.toon.as_str());
    let channel = upstream
        .and_then(|state| state.channel.as_deref())
        .unwrap_or("not joined");
    let legacy = app
        .selected_peer
        .map_or_else(|| "waiting".into(), |address| address.to_string());
    let members = upstream.map_or(0, |state| state.roster.len()).to_string();
    let friends = upstream.map_or(0, |state| state.friends).to_string();
    let overview = vec![
        key_value(" sc2 toon", toon),
        key_value(" channel", channel),
        key_value(" members", &members),
        key_value(" friends", &friends),
        key_value(" legacy", &legacy),
    ];
    frame.render_widget(
        Paragraph::new(overview)
            .style(Style::default().bg(PANEL))
            .block(panel(" upstream ")),
        sections[0],
    );

    let height = usize::from(sections[1].height.saturating_sub(2));
    let roster = upstream
        .into_iter()
        .flat_map(|state| state.roster.iter())
        .take(height)
        .map(|name| {
            ListItem::new(Line::from(vec![
                Span::styled(" ◦ ", Style::default().fg(CYAN)),
                Span::styled(name, Style::default().fg(TEXT)),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(roster)
            .style(Style::default().bg(PANEL))
            .block(panel(&format!(
                " roster · {} ",
                upstream.map_or(0, |state| state.roster.len())
            ))),
        sections[1],
    );
}

fn draw_activity(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let height = usize::from(area.height.saturating_sub(2));
    let skip = app.activity.len().saturating_sub(height);
    let items = app
        .activity
        .iter()
        .skip(skip)
        .map(|entry| {
            let (glyph, color) = activity_style(entry.kind);
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(
                        " {:02}:{:02} ",
                        entry.elapsed.as_secs() / 60,
                        entry.elapsed.as_secs() % 60
                    ),
                    Style::default().fg(FAINT),
                ),
                Span::styled(format!("{glyph} "), Style::default().fg(color)),
                Span::styled(&entry.message, Style::default().fg(TEXT)),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(items)
            .style(Style::default().bg(PANEL))
            .block(panel(" activity ")),
        area,
    );
}

fn draw_footer(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let hint = if app.stopping {
        " waiting for the gateway to stop "
    } else {
        " q / esc / ctrl-c  stop gateway "
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(hint, Style::default().fg(CYAN_SOFT)),
            Span::styled(
                format!("  │  {} active clients", app.legacy_peers.len()),
                Style::default().fg(MUTED),
            ),
        ]))
        .style(Style::default().bg(PANEL))
        .block(panel(" controls ")),
        area,
    );
}

fn panel(title: &str) -> Block<'_> {
    Block::default()
        .title(Line::from(Span::styled(
            title,
            Style::default().fg(CYAN_SOFT),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_MUTED))
        .style(Style::default().bg(PANEL))
}

fn key_value<'a>(key: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{key:<10} "), Style::default().fg(MUTED)),
        Span::styled(value, Style::default().fg(TEXT)),
    ])
}

fn activity_style(kind: ActivityKind) -> (&'static str, Color) {
    match kind {
        ActivityKind::System => ("◆", CYAN_SOFT),
        ActivityKind::Legacy => ("→", CYAN),
        ActivityKind::Upstream => ("←", GREEN),
        ActivityKind::Warning => ("!", AMBER),
        ActivityKind::Error => ("×", RED),
    }
}

fn activity_label(kind: ActivityKind) -> &'static str {
    match kind {
        ActivityKind::System => "system",
        ActivityKind::Legacy => "legacy",
        ActivityKind::Upstream => "upstream",
        ActivityKind::Warning => "warning",
        ActivityKind::Error => "error",
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    fn dashboard_renders_gateway_state() {
        let mut app = App::default();
        let peer = "192.0.2.1:5000".parse().expect("valid peer address");
        assert!(!app.apply(Event::Listening(
            "127.0.0.1:6112".parse().expect("valid socket address")
        )));
        assert!(!app.apply(Event::LegacyConnected(peer)));
        assert!(!app.apply(Event::Upstream {
            peer,
            toon: "Raynor".into(),
            channel: Some("General".into()),
            roster: vec!["Kerrigan".into(), "Raynor".into()],
            friends: 3,
        }));
        assert!(!app.apply(Event::Activity {
            kind: ActivityKind::Legacy,
            message: "legacy bot connected".into(),
        }));

        let mut terminal = Terminal::new(TestBackend::new(100, 28)).expect("test terminal");
        terminal.draw(|frame| draw(frame, &app)).expect("draw");
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();

        assert!(rendered.contains("gateway"));
        assert!(rendered.contains("Raynor"));
        assert!(rendered.contains("General"));
        assert!(rendered.contains("legacy bot connected"));
    }

    #[test]
    fn disconnect_removes_only_its_client_state() {
        let mut app = App::default();
        let first = "192.0.2.1:5000".parse().expect("valid peer address");
        let second = "192.0.2.2:5000".parse().expect("valid peer address");
        assert!(!app.apply(Event::LegacyConnected(first)));
        assert!(!app.apply(Event::LegacyConnected(second)));
        assert!(!app.apply(Event::Upstream {
            peer: first,
            toon: "Raynor".into(),
            channel: None,
            roster: Vec::new(),
            friends: 0,
        }));
        assert!(!app.apply(Event::Upstream {
            peer: second,
            toon: "Kerrigan".into(),
            channel: None,
            roster: Vec::new(),
            friends: 0,
        }));

        assert!(!app.apply(Event::LegacyDisconnected(second)));

        assert_eq!(app.legacy_peers.len(), 1);
        assert_eq!(app.upstreams.len(), 1);
        assert_eq!(app.selected_peer, Some(first));
        assert_eq!(
            app.selected_upstream().map(|state| state.toon.as_str()),
            Some("Raynor")
        );
    }
}
