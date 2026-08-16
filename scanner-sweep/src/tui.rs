use std::{
    fmt::Write as _,
    io::{self, Stdout},
    sync::mpsc::Receiver,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event as TerminalEvent, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction as LayoutDirection, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use sc2_core::native::inspect::{Direction, Record};

use crate::{Error, Result, session::FlowKey, session::SweepUpdate};

const BACKGROUND: Color = Color::Rgb(1, 8, 16);
const PANEL: Color = Color::Rgb(3, 14, 25);
const PANEL_SELECTED: Color = Color::Rgb(8, 43, 76);
const BORDER: Color = Color::Rgb(21, 103, 153);
const BORDER_MUTED: Color = Color::Rgb(15, 49, 70);
const CYAN: Color = Color::Rgb(62, 190, 244);
const CYAN_SOFT: Color = Color::Rgb(104, 188, 220);
const BLUE: Color = Color::Rgb(35, 126, 201);
const TEXT: Color = Color::Rgb(218, 229, 242);
const MUTED: Color = Color::Rgb(116, 143, 168);
const FAINT: Color = Color::Rgb(48, 70, 89);
const GREEN: Color = Color::Rgb(54, 218, 147);
const AMBER: Color = Color::Rgb(232, 169, 64);
const RED: Color = Color::Rgb(242, 91, 99);

pub enum Event {
    Update(SweepUpdate),
    Complete,
    Failed(Error),
}

struct App {
    status: String,
    flow: Option<FlowKey>,
    records: Vec<Record>,
    selected: usize,
    complete: bool,
    paused: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            status: "waiting for capture data".to_owned(),
            flow: None,
            records: Vec::new(),
            selected: 0,
            complete: false,
            paused: false,
        }
    }
}

impl App {
    fn update(&mut self, update: SweepUpdate) {
        match update {
            SweepUpdate::Status(status) => self.status = status,
            SweepUpdate::Activated(flow) => {
                "decrypting live traffic".clone_into(&mut self.status);
                self.flow = Some(flow);
            }
            SweepUpdate::Record(record) => {
                self.records.push(record);
                if !self.paused {
                    self.selected = self.records.len().saturating_sub(1);
                }
            }
        }
    }

    fn previous(&mut self) {
        self.paused = true;
        self.selected = self.selected.saturating_sub(1);
    }

    fn next(&mut self) {
        self.paused = true;
        self.selected = (self.selected + 1).min(self.records.len().saturating_sub(1));
    }

    fn resume(&mut self) {
        self.paused = false;
        self.selected = self.records.len().saturating_sub(1);
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

pub fn run(events: &Receiver<Event>) -> Result<()> {
    let (_guard, mut terminal) = TerminalGuard::enter()?;
    let mut app = App::default();
    loop {
        while let Ok(message) = events.try_recv() {
            match message {
                Event::Update(update) => app.update(update),
                Event::Complete => {
                    app.complete = true;
                    "replay complete".clone_into(&mut app.status);
                }
                Event::Failed(error) => return Err(error),
            }
        }
        terminal.draw(|frame| draw(frame, &app))?;
        if event::poll(Duration::from_millis(50))?
            && let TerminalEvent::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Up | KeyCode::Char('k') => app.previous(),
                KeyCode::Down | KeyCode::Char('j') => app.next(),
                KeyCode::End | KeyCode::Char(' ') => app.resume(),
                KeyCode::Home => {
                    app.paused = true;
                    app.selected = 0;
                }
                _ => {}
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
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());
    draw_header(frame, app, areas[0]);
    let body = Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([Constraint::Percentage(43), Constraint::Percentage(57)])
        .split(areas[1]);
    draw_records(frame, app, body[0]);
    draw_details(frame, app, body[1]);
    draw_footer(frame, app, areas[2]);
}

fn draw_header(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let flow = app.flow.as_ref().map_or_else(
        || "native flow not identified".to_owned(),
        ToString::to_string,
    );
    let status_color = if app.flow.is_some() { GREEN } else { AMBER };
    let header = Line::from(vec![
        Span::styled(" ● ", Style::default().fg(status_color)),
        Span::styled(&app.status, Style::default().fg(TEXT)),
        Span::styled("  │  ", Style::default().fg(BORDER_MUTED)),
        Span::styled(flow, Style::default().fg(MUTED)),
    ]);
    frame.render_widget(
        Paragraph::new(header)
            .style(Style::default().bg(PANEL))
            .block(
                Block::default()
                    .title(Line::from(Span::styled(
                        " scanner-sweep ",
                        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                    )))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_MUTED))
                    .style(Style::default().bg(PANEL)),
            ),
        area,
    );
}

fn draw_records(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let items = app
        .records
        .iter()
        .map(|record| {
            let (arrow, color) = direction_style(record.direction);
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {arrow} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("#{:04} ", record.sequence),
                    Style::default().fg(FAINT),
                ),
                Span::styled(
                    format!("{}/{} ", record.service, record.command_id),
                    Style::default().fg(TEXT),
                ),
                Span::styled(
                    format!("{} B", record.bytes.len()),
                    Style::default().fg(MUTED),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default();
    if !app.records.is_empty() {
        state.select(Some(app.selected));
    }
    frame.render_stateful_widget(
        List::new(items)
            .style(Style::default().fg(TEXT).bg(PANEL))
            .block(
                Block::default()
                    .title(Line::from(Span::styled(
                        format!(" records ({}) ", app.records.len()),
                        Style::default().fg(CYAN_SOFT),
                    )))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER))
                    .style(Style::default().bg(PANEL)),
            )
            .highlight_style(
                Style::default()
                    .fg(TEXT)
                    .bg(PANEL_SELECTED)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▸"),
        area,
        &mut state,
    );
}

fn draw_details(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let Some(record) = app.records.get(app.selected) else {
        frame.render_widget(
            Paragraph::new("no records yet")
                .style(Style::default().fg(FAINT).bg(PANEL))
                .block(
                    Block::default()
                        .title(Line::from(Span::styled(
                            " decoded record ",
                            Style::default().fg(CYAN_SOFT),
                        )))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(BORDER))
                        .style(Style::default().bg(PANEL)),
                ),
            area,
        );
        return;
    };
    let (arrow, direction_color) = direction_style(record.direction);
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!("{arrow}  "),
                Style::default()
                    .fg(direction_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} / {}", record.service, record.command),
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            format!(
                "{} · {} bytes · {} logical bits",
                record.type_name,
                record.bytes.len(),
                record.logical_bits
            ),
            Style::default().fg(MUTED),
        )),
        Line::default(),
    ];
    for field in &record.fields {
        let indent = "  ".repeat(field.depth);
        lines.push(Line::from(vec![
            Span::styled(indent, Style::default()),
            Span::styled(&field.path, Style::default().fg(CYAN_SOFT)),
            Span::styled(" = ", Style::default().fg(FAINT)),
            Span::styled(&field.value, Style::default().fg(TEXT)),
            Span::styled(
                format!("  [{}..{}]", field.start_bit, field.end_bit),
                Style::default().fg(FAINT),
            ),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "plaintext",
        Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
    )));
    lines.extend(hex_lines(&record.bytes));
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .title(Line::from(Span::styled(
                        " decoded record ",
                        Style::default().fg(CYAN_SOFT),
                    )))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER))
                    .style(Style::default().bg(PANEL)),
            ),
        area,
    );
}

fn draw_footer(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let mode = if app.paused {
        "paused"
    } else if app.complete {
        "replay complete"
    } else {
        "live"
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {mode} "),
                Style::default()
                    .fg(if app.paused { AMBER } else { GREEN })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  ↑↓/jk select  ·  home/end jump  ·  space follow  ·  q quit",
                Style::default().fg(MUTED),
            ),
        ]))
        .style(Style::default().bg(PANEL))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_MUTED))
                .style(Style::default().bg(PANEL)),
        ),
        area,
    );
}

fn direction_style(direction: Direction) -> (&'static str, Color) {
    match direction {
        Direction::Incoming => ("S→C", RED),
        Direction::Outgoing => ("C→S", CYAN),
    }
}

fn hex_lines(bytes: &[u8]) -> Vec<Line<'static>> {
    bytes
        .chunks(16)
        .enumerate()
        .map(|(row, chunk)| {
            let mut hex = String::new();
            let mut text = String::new();
            for (index, byte) in chunk.iter().enumerate() {
                if index == 8 {
                    hex.push(' ');
                }
                let _ = write!(hex, "{byte:02x} ");
                text.push(if byte.is_ascii_graphic() || *byte == b' ' {
                    char::from(*byte)
                } else {
                    '·'
                });
            }
            Line::from(vec![
                Span::styled(format!("{:08x}  ", row * 16), Style::default().fg(FAINT)),
                Span::styled(format!("{hex:<49}"), Style::default().fg(BLUE)),
                Span::styled(format!(" │{text}│"), Style::default().fg(MUTED)),
            ])
        })
        .collect()
}
