use std::{
    collections::VecDeque,
    error::Error,
    io::{self, Write},
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

use crossterm::event::{self, Event as TerminalEvent, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use superiority_core::{
    Error as CoreError, Result as CoreResult,
    bgs::SecretBytes,
    chat::{ChatChannel, ChatEvent, ChatUser, channel_title},
    connection::{
        ClientCommand, ClientEvent, ClientHandle, ConnectionStage, DEFAULT_PUBLIC_CHANNEL,
        spawn_client,
    },
    observer::NoObserver,
    product::Product,
};
use tao::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::{Window, WindowBuilder, WindowId},
};
use url::Url;
use wry::{NewWindowResponse, WebView, WebViewBuilder};

const TICK: Duration = Duration::from_millis(16);
const MAX_MESSAGES: usize = 500;
const BACKGROUND: Color = Color::Rgb(3, 10, 20);
const PANEL: Color = Color::Rgb(7, 19, 35);
const PANEL_BRIGHT: Color = Color::Rgb(10, 27, 48);
const BORDER: Color = Color::Rgb(25, 75, 106);
const BORDER_FOCUSED: Color = Color::Rgb(35, 194, 255);
const PRIMARY: Color = Color::Rgb(222, 239, 248);
const MUTED: Color = Color::Rgb(91, 124, 143);
const CYAN: Color = Color::Rgb(55, 211, 255);
const BLUE: Color = Color::Rgb(80, 145, 255);
const GREEN: Color = Color::Rgb(91, 221, 145);
const GOLD: Color = Color::Rgb(245, 194, 83);
const MAGENTA: Color = Color::Rgb(207, 130, 255);
const RED: Color = Color::Rgb(255, 102, 114);

type AnyResult<T> = Result<T, Box<dyn Error>>;

#[cfg(unix)]
struct BrowserErrorSilencer {
    stderr: std::os::fd::OwnedFd,
}

#[cfg(unix)]
impl BrowserErrorSilencer {
    fn new() -> io::Result<Self> {
        use std::{fs::OpenOptions, os::fd::AsRawFd};

        io::stderr().flush()?;
        let null = OpenOptions::new().write(true).open("/dev/null")?;
        let stderr = duplicate_descriptor(libc::STDERR_FILENO)?;
        replace_descriptor(null.as_raw_fd(), libc::STDERR_FILENO)?;
        Ok(Self { stderr })
    }
}

#[cfg(unix)]
impl Drop for BrowserErrorSilencer {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;

        let _ = replace_descriptor(self.stderr.as_raw_fd(), libc::STDERR_FILENO);
    }
}

#[cfg(unix)]
fn duplicate_descriptor(descriptor: i32) -> io::Result<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;

    let duplicate = unsafe { libc::dup(descriptor) };
    if duplicate < 0 {
        return Err(io::Error::last_os_error());
    }
    // duplicate is valid and uniquely owned after dup succeeds.
    Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(duplicate) })
}

#[cfg(unix)]
fn replace_descriptor(source: i32, target: i32) -> io::Result<()> {
    // both descriptors remain valid for the duration of this call.
    if unsafe { libc::dup2(source, target) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
struct BrowserErrorSilencer;

#[cfg(not(unix))]
impl BrowserErrorSilencer {
    fn new() -> io::Result<Self> {
        io::stderr().flush()?;
        Ok(Self)
    }
}

enum AuthenticationEvent {
    Complete(SecretBytes),
    Navigate(String),
}

#[derive(Clone, Copy)]
enum MessageKind {
    System,
    Success,
    Chat,
    Whisper,
    Presence,
    Warning,
    Error,
}

struct Message {
    kind: MessageKind,
    text: String,
}

struct AuthenticationWindow {
    webview: WebView,
    window: Window,
    reply: Option<Sender<CoreResult<SecretBytes>>>,
    _errors: BrowserErrorSilencer,
}

impl AuthenticationWindow {
    fn open(
        event_loop: &EventLoop<()>,
        url: &Url,
        reply: Sender<CoreResult<SecretBytes>>,
        callback: Sender<AuthenticationEvent>,
    ) -> AnyResult<Self> {
        let errors = BrowserErrorSilencer::new()?;
        let window = WindowBuilder::new()
            .with_title("Superiority — Battle.net Authentication")
            .with_inner_size(LogicalSize::new(980.0, 760.0))
            .build(event_loop)?;
        let navigation_callback = callback.clone();
        let webview = WebViewBuilder::new()
            .with_url(url.as_str())
            .with_devtools(false)
            .with_navigation_handler(move |location| {
                let Some(credential) = web_auth_credential(&location) else {
                    return true;
                };
                let _ = navigation_callback.send(AuthenticationEvent::Complete(credential));
                false
            })
            .with_new_window_req_handler(move |location, _| {
                if let Some(credential) = web_auth_credential(&location) {
                    let _ = callback.send(AuthenticationEvent::Complete(credential));
                } else {
                    let _ = callback.send(AuthenticationEvent::Navigate(location));
                }
                NewWindowResponse::Deny
            })
            .build(&window)?;
        window.set_focus();
        Ok(Self {
            webview,
            window,
            reply: Some(reply),
            _errors: errors,
        })
    }

    fn id(&self) -> WindowId {
        self.window.id()
    }

    fn navigate(&self, location: &str) -> wry::Result<()> {
        self.webview.load_url(location)
    }

    fn complete(mut self, credential: SecretBytes) {
        if let Some(reply) = self.reply.take() {
            let _ = reply.send(Ok(credential));
        }
    }

    fn cancel(mut self) {
        if let Some(reply) = self.reply.take() {
            let _ = reply.send(Err(CoreError::Authentication(
                "Battle.net authentication was cancelled".into(),
            )));
        }
    }
}

struct App {
    commands: Sender<ClientCommand>,
    events: Receiver<ClientEvent>,
    authentication: Option<AuthenticationWindow>,
    auth_callback_tx: Sender<AuthenticationEvent>,
    auth_callback_rx: Receiver<AuthenticationEvent>,
    stage: ConnectionStage,
    channel_index: Option<u8>,
    channel_name: String,
    roster: Vec<String>,
    messages: VecDeque<Message>,
    input: String,
    running: bool,
}

impl App {
    fn new(client: ClientHandle) -> Self {
        let (auth_callback_tx, auth_callback_rx) = mpsc::channel();
        Self {
            commands: client.commands,
            events: client.events,
            authentication: None,
            auth_callback_tx,
            auth_callback_rx,
            stage: ConnectionStage::Disconnected,
            channel_index: None,
            channel_name: "General".into(),
            roster: Vec::new(),
            messages: VecDeque::new(),
            input: String::new(),
            running: true,
        }
    }

    fn connect(&mut self, force_interactive: bool) -> AnyResult<()> {
        self.commands.send(ClientCommand::Connect {
            force_interactive,
            expected_account_id: None,
            expected_battle_tag: None,
            channels: vec![ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL)],
        })?;
        self.push_message(MessageKind::System, "connecting to Battle.net");
        Ok(())
    }

    fn tick(&mut self, event_loop: &EventLoop<()>) -> AnyResult<()> {
        self.receive_authentication_events();
        self.receive_client_events(event_loop);
        self.receive_terminal_events()?;
        Ok(())
    }

    fn receive_authentication_events(&mut self) {
        while let Ok(event) = self.auth_callback_rx.try_recv() {
            match event {
                AuthenticationEvent::Complete(credential) => {
                    if let Some(authentication) = self.authentication.take() {
                        authentication.complete(credential);
                        self.push_message(MessageKind::Success, "authentication completed");
                    }
                }
                AuthenticationEvent::Navigate(location) => {
                    let error = self
                        .authentication
                        .as_ref()
                        .and_then(|authentication| authentication.navigate(&location).err());
                    if let Some(error) = error {
                        self.push_message(
                            MessageKind::Error,
                            format!("authentication navigation failed: {error}"),
                        );
                    }
                }
            }
        }
    }

    fn receive_client_events(&mut self, event_loop: &EventLoop<()>) {
        while let Ok(event) = self.events.try_recv() {
            self.handle_client_event(event_loop, event);
        }
    }

    fn handle_client_event(&mut self, event_loop: &EventLoop<()>, event: ClientEvent) {
        match event {
            // this example is StarCraft II's, as its name says
            ClientEvent::Classic(_)
            | ClientEvent::ClassicChannel(_)
            | ClientEvent::ClassicFriends(_)
            | ClientEvent::ClassicWhisperSent { .. }
            | ClientEvent::Warcraft(_)
            | ClientEvent::WarcraftChannel(_)
            | ClientEvent::WarcraftChannels(_)
            | ClientEvent::WarcraftFriends(_)
            | ClientEvent::WarcraftClan(_)
            | ClientEvent::ProductCredential { .. } => {}
            ClientEvent::Stage(stage) => {
                self.stage = stage;
                self.push_message(MessageKind::System, format!("connection stage: {stage:?}"));
                if stage != ConnectionStage::WebAuthentication {
                    self.authentication.take();
                }
            }
            ClientEvent::Account(account) => self.push_message(
                MessageKind::System,
                format!(
                    "account licenses: {}",
                    account
                        .games
                        .as_deref()
                        .map_or("unavailable".to_owned(), |games| games.join(", "))
                ),
            ),
            ClientEvent::Authentication { url, reply, .. } => {
                if let Some(authentication) = self.authentication.take() {
                    authentication.cancel();
                }
                let failure_reply = reply.clone();
                match AuthenticationWindow::open(
                    event_loop,
                    &url,
                    reply,
                    self.auth_callback_tx.clone(),
                ) {
                    Ok(authentication) => {
                        self.authentication = Some(authentication);
                        self.push_message(
                            MessageKind::Warning,
                            "complete authentication in the browser window",
                        );
                    }
                    Err(error) => {
                        let message = format!("could not open authentication: {error}");
                        let _ = failure_reply.send(Err(CoreError::Authentication(message.clone())));
                        self.push_message(MessageKind::Error, message);
                    }
                }
            }
            ClientEvent::Chat(event) => self.handle_chat_event(event),
            ClientEvent::CommandError(error) | ClientEvent::Error(error) => {
                self.push_message(MessageKind::Error, format!("error: {error}"));
            }
        }
    }

    fn handle_chat_event(&mut self, event: ChatEvent) {
        match event {
            ChatEvent::Joined {
                channel_index,
                channel,
                ..
            } => {
                self.channel_index = Some(channel_index);
                self.channel_name = channel_title(&channel);
                self.push_message(
                    MessageKind::Success,
                    format!("joined {}", self.channel_name),
                );
            }
            ChatEvent::Roster(snapshot) => {
                if Some(snapshot.channel_index) == self.channel_index {
                    self.roster = snapshot.users.iter().map(ChatUser::visible_name).collect();
                    self.sort_roster();
                }
            }
            ChatEvent::Message { sender, body, .. } => {
                self.push_message(
                    MessageKind::Chat,
                    format!("{}: {body}", sender.visible_name()),
                );
            }
            ChatEvent::Whisper {
                peer,
                body,
                outgoing,
            } => {
                let direction = if outgoing { "to" } else { "from" };
                self.push_message(
                    MessageKind::Whisper,
                    format!("whisper {direction} {peer}: {body}"),
                );
            }
            ChatEvent::MemberJoined {
                channel_index,
                user,
            } => {
                let name = user.visible_name();
                if Some(channel_index) == self.channel_index && !self.roster.contains(&name) {
                    self.roster.push(name.clone());
                    self.sort_roster();
                }
                self.push_message(MessageKind::Presence, format!("{name} joined"));
            }
            ChatEvent::MemberLeft {
                channel_index,
                user,
                ..
            } => {
                let name = user.visible_name();
                if Some(channel_index) == self.channel_index {
                    self.roster.retain(|member| member != &name);
                }
                self.push_message(MessageKind::Presence, format!("{name} left"));
            }
            ChatEvent::Removed { channel_index, .. } => {
                if Some(channel_index) == self.channel_index {
                    self.channel_index = None;
                    self.roster.clear();
                }
                self.push_message(MessageKind::Warning, "removed from channel");
            }
            ChatEvent::JoinRejected { reason, .. } => {
                let reason = reason.map_or_else(
                    || "unspecified error".to_owned(),
                    superiority_core::native::errors::description,
                );
                self.push_message(
                    MessageKind::Error,
                    format!("channel join rejected: {reason}"),
                );
            }
            ChatEvent::WhisperFailed { peer, reason } => {
                self.push_message(
                    MessageKind::Error,
                    format!("whisper to {peer} failed: {reason}"),
                );
            }
            _ => {}
        }
    }

    fn receive_terminal_events(&mut self) -> AnyResult<()> {
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                TerminalEvent::Key(key)
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        self.running = false;
                        continue;
                    }
                    match key.code {
                        KeyCode::Esc => self.running = false,
                        KeyCode::Enter => self.send_input(),
                        KeyCode::Backspace => {
                            self.input.pop();
                        }
                        KeyCode::Char(character)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            self.input.push(character);
                        }
                        _ => {}
                    }
                }
                TerminalEvent::Paste(text) => self.input.push_str(&text),
                _ => {}
            }
        }
        Ok(())
    }

    fn send_input(&mut self) {
        let body = self.input.trim().to_owned();
        if body.is_empty() {
            return;
        }
        let Some(channel_index) = self.channel_index else {
            self.push_message(MessageKind::Warning, "not connected to a channel");
            return;
        };
        match self.commands.send(ClientCommand::SendMessage {
            channel_index,
            body,
        }) {
            Ok(()) => self.input.clear(),
            Err(error) => self.push_message(
                MessageKind::Error,
                format!("could not send message: {error}"),
            ),
        }
    }

    fn close_authentication(&mut self) {
        if let Some(authentication) = self.authentication.take() {
            authentication.cancel();
            self.push_message(MessageKind::Warning, "authentication cancelled");
        }
    }

    fn authentication_window_id(&self) -> Option<WindowId> {
        self.authentication.as_ref().map(AuthenticationWindow::id)
    }

    fn push_message(&mut self, kind: MessageKind, message: impl Into<String>) {
        self.messages.push_back(Message {
            kind,
            text: message.into(),
        });
        while self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
    }

    fn sort_roster(&mut self) {
        self.roster
            .sort_unstable_by_key(|name| name.to_ascii_lowercase());
    }

    fn draw(&self, terminal: &mut DefaultTerminal) -> AnyResult<()> {
        terminal.draw(|frame| self.render(frame))?;
        Ok(())
    }

    fn render(&self, frame: &mut Frame<'_>) {
        frame.render_widget(
            Block::default().style(Style::default().bg(BACKGROUND)),
            frame.area(),
        );
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(3),
                Constraint::Length(3),
            ])
            .split(frame.area());

        self.render_header(frame, areas[0]);
        self.render_content(frame, areas[1]);
        self.render_input(frame, areas[2]);
    }

    fn render_header(&self, frame: &mut Frame<'_>, area: Rect) {
        let status_color = stage_color(self.stage);
        let details = Line::from(vec![
            Span::styled(
                format!(" {} ", self.channel_name),
                Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
            ),
            Span::styled("│ ", Style::default().fg(BORDER)),
            Span::styled(
                format!("{} online", self.roster.len()),
                Style::default().fg(BLUE),
            ),
        ]);
        let status = Line::from(vec![
            Span::styled(
                "● ",
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                stage_label(self.stage),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL));
        let inner = block.inner(area);
        let sections = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(20)])
            .split(inner);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(details).style(Style::default().fg(PRIMARY).bg(PANEL)),
            sections[0],
        );
        frame.render_widget(
            Paragraph::new(status)
                .alignment(Alignment::Right)
                .style(Style::default().bg(PANEL_BRIGHT)),
            sections[1],
        );
    }

    fn render_content(&self, frame: &mut Frame<'_>, area: Rect) {
        if self.roster.is_empty() {
            self.render_transcript(frame, area);
            return;
        }
        if area.width >= 72 {
            let panes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(30), Constraint::Length(26)])
                .split(area);
            self.render_transcript(frame, panes[0]);
            self.render_roster(frame, panes[1]);
        } else {
            let panes = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(6)])
                .split(area);
            self.render_transcript(frame, panes[0]);
            self.render_roster(frame, panes[1]);
        }
    }

    fn render_transcript(&self, frame: &mut Frame<'_>, area: Rect) {
        let visible_lines = usize::from(area.height.saturating_sub(2));
        let first = self.messages.len().saturating_sub(visible_lines);
        let transcript = self
            .messages
            .iter()
            .skip(first)
            .map(message_line)
            .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(transcript)
                .style(Style::default().fg(PRIMARY).bg(BACKGROUND))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(BORDER))
                        .title(Span::styled(
                            format!(" {} ", self.channel_name),
                            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                        )),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_roster(&self, frame: &mut Frame<'_>, area: Rect) {
        let capacity = usize::from(area.height.saturating_sub(2));
        let shown = if self.roster.len() > capacity {
            capacity.saturating_sub(1)
        } else {
            capacity
        };
        let mut users = self
            .roster
            .iter()
            .take(shown)
            .map(|name| {
                Line::from(vec![
                    Span::styled(" ● ", Style::default().fg(GREEN)),
                    Span::styled(name.as_str(), Style::default().fg(PRIMARY)),
                ])
            })
            .collect::<Vec<_>>();
        if self.roster.len() > shown && capacity > 0 {
            users.push(Line::from(Span::styled(
                format!("   +{} more", self.roster.len() - shown),
                Style::default().fg(MUTED),
            )));
        }
        frame.render_widget(
            Paragraph::new(users)
                .style(Style::default().fg(PRIMARY).bg(PANEL))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(BORDER))
                        .title(Span::styled(
                            format!(" USERS {} ", self.roster.len()),
                            Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                        )),
                ),
            area,
        );
    }

    fn render_input(&self, frame: &mut Frame<'_>, area: Rect) {
        let input_line = if self.input.is_empty() {
            Line::from(Span::styled(
                "type a message…",
                Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
            ))
        } else {
            Line::from(Span::styled(
                self.input.as_str(),
                Style::default().fg(PRIMARY),
            ))
        };
        frame.render_widget(
            Paragraph::new(input_line)
                .style(Style::default().bg(PANEL))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(BORDER_FOCUSED))
                        .title(Span::styled(
                            " COMPOSE ",
                            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                        )),
                ),
            area,
        );

        let input_width = u16::try_from(self.input.chars().count()).unwrap_or(u16::MAX);
        let cursor_x = area
            .x
            .saturating_add(1)
            .saturating_add(input_width)
            .min(area.right().saturating_sub(2));
        frame.set_cursor_position(Position::new(cursor_x, area.y.saturating_add(1)));
    }

    fn shutdown(&mut self) {
        self.close_authentication();
        let _ = self.commands.send(ClientCommand::Quit);
    }
}

fn stage_label(stage: ConnectionStage) -> &'static str {
    match stage {
        ConnectionStage::Disconnected => "OFFLINE",
        ConnectionStage::WebAuthentication => "AUTHENTICATING",
        ConnectionStage::GameUtilities => "HANDOFF",
        ConnectionStage::NativeAuthentication => "CONNECTING",
        ConnectionStage::ChatBootstrap => "SYNCING",
        ConnectionStage::Connected => "CONNECTED",
    }
}

fn stage_color(stage: ConnectionStage) -> Color {
    match stage {
        ConnectionStage::Disconnected => MUTED,
        ConnectionStage::WebAuthentication | ConnectionStage::GameUtilities => GOLD,
        ConnectionStage::NativeAuthentication | ConnectionStage::ChatBootstrap => BLUE,
        ConnectionStage::Connected => GREEN,
    }
}

fn message_line(message: &Message) -> Line<'_> {
    let (marker, color, text_style) = match message.kind {
        MessageKind::System => (
            "·",
            MUTED,
            Style::default().fg(MUTED).add_modifier(Modifier::DIM),
        ),
        MessageKind::Success => ("◆", GREEN, Style::default().fg(PRIMARY)),
        MessageKind::Chat => ("›", CYAN, Style::default().fg(PRIMARY)),
        MessageKind::Whisper => ("↗", MAGENTA, Style::default().fg(MAGENTA)),
        MessageKind::Presence => ("•", GOLD, Style::default().fg(MUTED)),
        MessageKind::Warning => ("!", GOLD, Style::default().fg(GOLD)),
        MessageKind::Error => ("×", RED, Style::default().fg(RED)),
    };
    Line::from(vec![
        Span::styled(
            format!(" {marker} "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(message.text.as_str(), text_style),
    ])
}

struct TerminalRestore;

impl Drop for TerminalRestore {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

fn web_auth_credential(location: &str) -> Option<SecretBytes> {
    let url = Url::parse(location).ok()?;
    if url.scheme() != "http" || url.host_str() != Some("localhost") || url.port() != Some(0) {
        return None;
    }
    let credential = url
        .query_pairs()
        .find_map(|(name, value)| (name == "ST").then(|| value.into_owned()))?;
    let bytes = credential.into_bytes();
    if bytes.is_empty() || bytes.len() > 1024 || !bytes.iter().all(u8::is_ascii_graphic) {
        return None;
    }
    SecretBytes::new(bytes).ok()
}

fn run_event_loop(
    event_loop: &mut EventLoop<()>,
    app: &mut App,
    terminal: &mut DefaultTerminal,
) -> AnyResult<()> {
    while app.running {
        let auth_window_id = app.authentication_window_id();
        let mut auth_closed = false;
        let deadline = Instant::now() + TICK;
        event_loop.run_return(|event, _, control_flow| {
            *control_flow = ControlFlow::WaitUntil(deadline);
            match event {
                Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                    ..
                } if Some(window_id) == auth_window_id => {
                    auth_closed = true;
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            }
        });
        if auth_closed {
            app.close_authentication();
        }
        app.tick(event_loop)?;
        app.draw(terminal)?;
    }
    app.shutdown();
    Ok(())
}

fn main() -> AnyResult<()> {
    let force_interactive = std::env::args()
        .skip(1)
        .any(|argument| argument == "--reauth");
    let mut event_loop = EventLoop::new();
    let client = spawn_client(Product::StarCraft2, Box::new(NoObserver));
    let mut app = App::new(client);
    app.connect(force_interactive)?;

    let mut terminal = ratatui::init();
    let _restore = TerminalRestore;
    run_event_loop(&mut event_loop, &mut app, &mut terminal)
}
