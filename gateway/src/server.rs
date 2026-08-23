use std::{
    collections::BTreeMap,
    io::{self, BufWriter, Read as _},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

use superiority_core::Error as ProtocolError;

use crate::{
    bncs::{
        self, Action, EID_CHANNEL, EID_CHANNEL_DOES_NOT_EXIST, EID_ERROR, EID_INFO, EID_JOIN,
        EID_LEAVE, EID_SHOW_USER, EID_TALK, EID_WHISPER, EID_WHISPER_SENT,
        FRIEND_LOCATION_NOT_IN_CHAT, FRIEND_LOCATION_OFFLINE, FRIEND_LOCATION_PUBLIC_GAME,
        FRIEND_STATUS_AWAY, FRIEND_STATUS_DND, FRIEND_STATUS_MUTUAL, FriendEntry, LegacyPeer,
        PRODUCT_STAR, Packet,
    },
    error::Result as GatewayResult,
    event::{ActivityKind, Event as GatewayEvent, Reporter},
    sc2::{ChannelSnapshot, Event, Friend, FriendPresence, Session},
    web_auth::{Cancellation, Requester},
};

const IDLE_POLL: Duration = Duration::from_millis(250);
const CLIENT_POLL: Duration = Duration::from_millis(75);
const SELECTOR_TIMEOUT: Duration = Duration::from_secs(30);
const CLIENT_WRITE_TIMEOUT: Duration = Duration::from_millis(500);
// bncs statstrings carry the product in wire byte order; legacy clients reverse
// this prefix, so `rats` is displayed as the `star` product.
const DEFAULT_STATS: &str = "RATS";

type BotPeer = LegacyPeer<BufWriter<TcpStream>>;

#[derive(Clone)]
pub struct Configuration {
    pub listen: SocketAddr,
    pub force_login: bool,
    pub startup_timeout: Duration,
    pub login_timeout: Duration,
    pub authentication: Requester,
}

pub fn run(
    configuration: &Configuration,
    stop: &Arc<AtomicBool>,
    reporter: &Reporter,
) -> GatewayResult<()> {
    let listener = TcpListener::bind(configuration.listen)?;
    listener.set_nonblocking(true)?;
    let local_address = listener.local_addr()?;
    let _ = reporter.send(GatewayEvent::Listening(local_address));
    reporter.activity(
        ActivityKind::System,
        format!("listening on {local_address}"),
    );
    if !configuration.listen.ip().is_loopback() {
        reporter.activity(
            ActivityKind::Warning,
            "local bncs authentication is exposed to the trusted lan",
        );
    }
    let mut workers = Vec::new();

    let result = loop {
        if stop.load(Ordering::Acquire) {
            break Ok(());
        }
        match listener.accept() {
            Ok((stream, address)) => {
                let _ = reporter.send(GatewayEvent::LegacyConnected(address));
                reporter.activity(
                    ActivityKind::Legacy,
                    format!("legacy bot connected from {address}"),
                );
                let configuration = configuration.clone();
                let stop = Arc::clone(stop);
                let reporter = reporter.clone();
                let worker = thread::Builder::new()
                    .name(format!("gateway-client-{address}"))
                    .spawn(move || {
                        let client_reporter = ClientReporter::new(reporter.clone(), address);
                        match serve_client(&configuration, stream, &stop, &client_reporter) {
                            Ok(()) => {}
                            Err(ServeError::Client(error)) => client_reporter.activity(
                                ActivityKind::Warning,
                                format!("legacy connection ended: {error}"),
                            ),
                            Err(ServeError::Upstream(error)) => client_reporter.activity(
                                ActivityKind::Warning,
                                format!("SC2 connection ended: {error}"),
                            ),
                        }
                        let _ = reporter.send(GatewayEvent::LegacyDisconnected(address));
                        client_reporter.activity(ActivityKind::Legacy, "legacy bot disconnected");
                    })?;
                workers.push(worker);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                reap_workers(&mut workers, reporter);
                thread::sleep(IDLE_POLL);
            }
            Err(error) => break Err(error.into()),
        }
    };
    if result.is_err() {
        stop.store(true, Ordering::Release);
    }
    reporter.status("stopping");
    join_workers(workers, reporter);
    result
}

fn reap_workers(workers: &mut Vec<thread::JoinHandle<()>>, reporter: &Reporter) {
    let mut index = 0;
    while index < workers.len() {
        if workers[index].is_finished() {
            let worker = workers.swap_remove(index);
            if worker.join().is_err() {
                reporter.activity(ActivityKind::Error, "client worker panicked");
            }
        } else {
            index += 1;
        }
    }
}

fn join_workers(workers: Vec<thread::JoinHandle<()>>, reporter: &Reporter) {
    for worker in workers {
        if worker.join().is_err() {
            reporter.activity(ActivityKind::Error, "client worker panicked");
        }
    }
}

#[derive(Clone)]
struct ClientReporter {
    reporter: Reporter,
    address: SocketAddr,
}

impl ClientReporter {
    const fn new(reporter: Reporter, address: SocketAddr) -> Self {
        Self { reporter, address }
    }

    fn activity(&self, kind: ActivityKind, message: impl std::fmt::Display) {
        self.reporter
            .activity(kind, format!("[{}] {message}", self.address));
    }

    fn send(&self, event: GatewayEvent) {
        let _ = self.reporter.send(event);
    }
}

#[derive(Debug)]
enum ServeError {
    Client(io::Error),
    Upstream(ProtocolError),
}

enum ClientInput {
    Packet(Packet),
    Closed(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UserView {
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChannelView {
    index: u8,
    label: String,
    users: BTreeMap<u32, UserView>,
}

struct ClientState {
    toon_name: String,
    entered_chat: bool,
    channel_view: Option<ChannelView>,
    friend_view: Option<Vec<FriendEntry>>,
}

fn serve_client(
    configuration: &Configuration,
    mut stream: TcpStream,
    stop: &Arc<AtomicBool>,
    reporter: &ClientReporter,
) -> Result<(), ServeError> {
    // accepted sockets inherit nonblocking mode on macos, but the dedicated
    // reader uses blocking reads after the protocol selector.
    stream.set_nonblocking(false).map_err(ServeError::Client)?;
    stream.set_nodelay(true).map_err(ServeError::Client)?;
    stream
        .set_write_timeout(Some(CLIENT_WRITE_TIMEOUT))
        .map_err(ServeError::Client)?;
    let selector = read_selector(&mut stream, stop).map_err(ServeError::Client)?;
    if selector != bncs::GAME_PROTOCOL {
        return Err(ServeError::Client(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported protocol selector 0x{selector:02X}"),
        )));
    }

    let reader = stream.try_clone().map_err(ServeError::Client)?;
    let writer = stream.try_clone().map_err(ServeError::Client)?;
    let connection_closed = Arc::new(AtomicBool::new(false));
    let (incoming, reader_thread) = spawn_reader(reader, Arc::clone(&connection_closed));
    let gateway_stopping = Arc::clone(stop);
    let connection_closed_for_authentication = Arc::clone(&connection_closed);
    let cancellation: Cancellation = Arc::new(move || {
        gateway_stopping.load(Ordering::Acquire)
            || connection_closed_for_authentication.load(Ordering::Acquire)
    });

    let outcome = run_client_session(
        configuration,
        writer,
        &incoming,
        cancellation,
        stop,
        reporter,
    );

    let _ = stream.shutdown(Shutdown::Both);
    connection_closed.store(true, Ordering::Release);
    if reader_thread.join().is_err() {
        reporter.activity(ActivityKind::Warning, "legacy reader thread panicked");
    }
    outcome
}

fn run_client_session(
    configuration: &Configuration,
    writer: TcpStream,
    incoming: &Receiver<ClientInput>,
    cancellation: Cancellation,
    stop: &Arc<AtomicBool>,
    reporter: &ClientReporter,
) -> Result<(), ServeError> {
    let mut progress = |message: &str| reporter.activity(ActivityKind::Upstream, message);
    let mut session = Session::connect(
        configuration.authentication.clone(),
        cancellation,
        configuration.force_login,
        configuration.startup_timeout,
        configuration.login_timeout,
        &mut progress,
    )
    .map_err(ServeError::Upstream)?;
    reporter.activity(
        ActivityKind::Upstream,
        format!("authenticated as {}", session.toon_name()),
    );
    report_upstream(reporter, &session);
    let mut state = ClientState {
        toon_name: session.toon_name().to_owned(),
        entered_chat: false,
        channel_view: None,
        friend_view: None,
    };
    let mut peer = LegacyPeer::new(BufWriter::new(writer), state.toon_name.clone());

    let mut outcome = client_loop(
        &mut session,
        &mut peer,
        incoming,
        &mut state,
        stop,
        reporter,
    );

    if outcome.is_ok()
        && let Err(error) = peer.flush()
    {
        outcome = Err(ServeError::Client(error));
    }
    session.close();
    outcome
}

fn read_selector(stream: &mut TcpStream, stop: &AtomicBool) -> io::Result<u8> {
    let deadline = Instant::now() + SELECTOR_TIMEOUT;
    stream.set_read_timeout(Some(CLIENT_POLL))?;
    let mut selector = [0u8; 1];
    loop {
        match stream.read(&mut selector) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "legacy client closed before the protocol selector",
                ));
            }
            Ok(_) => {
                stream.set_read_timeout(None)?;
                return Ok(selector[0]);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                if stop.load(Ordering::Acquire) {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "gateway is stopping",
                    ));
                }
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "legacy client did not send a protocol selector",
                    ));
                }
            }
            Err(error) => return Err(error),
        }
    }
}

fn client_loop(
    session: &mut Session,
    peer: &mut BotPeer,
    incoming: &Receiver<ClientInput>,
    state: &mut ClientState,
    stop: &Arc<AtomicBool>,
    reporter: &ClientReporter,
) -> Result<(), ServeError> {
    while !stop.load(Ordering::Acquire) {
        loop {
            match incoming.try_recv() {
                Ok(ClientInput::Packet(packet)) => {
                    reporter.activity(
                        ActivityKind::Legacy,
                        format!(
                            "received {} (0x{:02x}, {} bytes)",
                            packet.name(),
                            packet.id,
                            packet.payload.len()
                        ),
                    );
                    let actions = peer.handle(&packet).map_err(ServeError::Client)?;
                    for action in actions {
                        handle_action(session, peer, action, state, reporter)?;
                    }
                    peer.flush().map_err(ServeError::Client)?;
                }
                Ok(ClientInput::Closed(reason)) => {
                    if let Some(reason) = reason {
                        reporter.activity(
                            ActivityKind::Warning,
                            format!("legacy reader stopped: {reason}"),
                        );
                    }
                    return Ok(());
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }

        let events = session.poll(CLIENT_POLL).map_err(ServeError::Upstream)?;
        for event in &events {
            report_sc2_event(reporter, event);
        }
        if state.entered_chat {
            for event in &events {
                relay_event(
                    session,
                    peer,
                    event,
                    &state.toon_name,
                    &mut state.channel_view,
                    reporter,
                )
                .map_err(ServeError::Client)?;
            }

            peer.flush().map_err(ServeError::Client)?;
        }

        if peer
            .service_keepalive(Instant::now())
            .map_err(ServeError::Client)?
        {
            peer.flush().map_err(ServeError::Client)?;
        }
        if events.iter().any(event_changes_snapshot) {
            report_upstream(reporter, session);
        }
    }
    Ok(())
}

fn spawn_reader(
    mut stream: TcpStream,
    connection_closed: Arc<AtomicBool>,
) -> (Receiver<ClientInput>, thread::JoinHandle<()>) {
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        loop {
            match Packet::read_from(&mut stream) {
                Ok(Some(packet)) => {
                    if sender.send(ClientInput::Packet(packet)).is_err() {
                        break;
                    }
                }
                Ok(None) => {
                    let _ = sender.send(ClientInput::Closed(None));
                    break;
                }
                Err(error) => {
                    let _ = sender.send(ClientInput::Closed(Some(error.to_string())));
                    break;
                }
            }
        }
        connection_closed.store(true, Ordering::Release);
    });
    (receiver, handle)
}

fn handle_action(
    session: &mut Session,
    peer: &mut BotPeer,
    action: Action,
    state: &mut ClientState,
    reporter: &ClientReporter,
) -> Result<(), ServeError> {
    match action {
        Action::EnterChat { requested_name } => {
            debug_assert!(requested_name.eq_ignore_ascii_case(&state.toon_name));
            peer.send_enter_chat(&state.toon_name, DEFAULT_STATS)
                .map_err(ServeError::Client)?;
            state.entered_chat = true;
            reporter.activity(
                ActivityKind::Legacy,
                format!("entered chat as {}", state.toon_name),
            );
        }
        Action::RequestChannelList => {
            let channels = session.channel_names();
            peer.send_channel_list(&channels)
                .map_err(ServeError::Client)?;
            reporter.activity(
                ActivityKind::Legacy,
                format!("sent channel list: {} channels", channels.len()),
            );
        }
        Action::JoinChannel { flags, channel } => {
            reporter.activity(
                ActivityKind::Legacy,
                format!("join request for {channel} with flags 0x{flags:08x}"),
            );
            if state.entered_chat {
                handle_join_request(
                    session,
                    peer,
                    &state.toon_name,
                    &mut state.channel_view,
                    flags,
                    &channel,
                    reporter,
                )?;
            } else {
                peer.send_error("Enter chat before joining a channel.")
                    .map_err(ServeError::Client)?;
            }
        }
        Action::LeaveChat => {
            session
                .leave_active_channel()
                .map_err(ServeError::Upstream)?;
            state.channel_view = None;
            reporter.activity(ActivityKind::Legacy, "left the active channel");
        }
        Action::ChatCommand(command) => {
            if state.entered_chat {
                handle_chat_command(
                    session,
                    peer,
                    &state.toon_name,
                    &mut state.channel_view,
                    &command,
                    reporter,
                )?;
            } else {
                peer.send_error("Log in as the selected SC2 toon before entering chat.")
                    .map_err(ServeError::Client)?;
            }
        }
        Action::RequestFriendsList => {
            reporter.activity(ActivityKind::Legacy, "friends list requested");
            send_friends_snapshot(session, peer, &mut state.friend_view)
                .map_err(ServeError::Client)?;
        }
        Action::Pong { round_trip } => {
            if let Some(round_trip) = round_trip {
                reporter.activity(
                    ActivityKind::Legacy,
                    format!("ping reply: {} ms", round_trip.as_millis()),
                );
            } else {
                reporter.activity(ActivityKind::Legacy, "ping reply received");
            }
        }
        Action::UdpCapability { supported } => {
            reporter.activity(
                ActivityKind::Legacy,
                if supported {
                    "client reported udp support"
                } else {
                    "client reported no udp support"
                },
            );
        }
    }
    Ok(())
}

fn handle_join_request(
    session: &mut Session,
    peer: &mut BotPeer,
    toon_name: &str,
    channel_view: &mut Option<ChannelView>,
    flags: u32,
    channel: &str,
    reporter: &ClientReporter,
) -> Result<(), ServeError> {
    let channel = normalize_channel_name(channel);
    let channel = match channel {
        Ok(channel) => channel,
        Err(message) => {
            peer.send_error(message).map_err(ServeError::Client)?;
            return Ok(());
        }
    };
    match flags {
        0 => {
            // nocreate joins are used by legacy gateway-discovery probes; an
            // unmapped name must be reported absent without creating it in sc2.
            if channel_view
                .as_ref()
                .is_some_and(|current| current.label.eq_ignore_ascii_case(&channel))
            {
                send_channel_snapshot(session, peer, toon_name, channel_view, reporter)
                    .map_err(ServeError::Client)?;
            } else {
                peer.send_chat_event(EID_CHANNEL_DOES_NOT_EXIST, "Battle.net", &channel, 0, 0)
                    .map_err(ServeError::Client)?;
            }
        }
        1 | 5 => {
            // firstjoin requests a product room; the sc2 session already has
            // its regional general channel, so announce that active mapping.
            if session.active_channel().is_some() {
                send_channel_snapshot(session, peer, toon_name, channel_view, reporter)
                    .map_err(ServeError::Client)?;
            } else if let Err(error) = session.join_default() {
                peer.send_error(&format!("Could not join the default channel: {error}"))
                    .map_err(ServeError::Client)?;
            } else {
                reporter.activity(ActivityKind::Upstream, "joining the default channel");
            }
        }
        2 => {
            if let Err(error) = session.join(&channel) {
                peer.send_error(&format!("Could not join channel: {error}"))
                    .map_err(ServeError::Client)?;
            } else {
                reporter.activity(ActivityKind::Upstream, format!("joining {channel}"));
            }
        }
        other => {
            peer.send_error(&format!("Unsupported SID_JOINCHANNEL flags 0x{other:08X}."))
                .map_err(ServeError::Client)?;
        }
    }
    Ok(())
}

fn handle_chat_command(
    session: &mut Session,
    peer: &mut BotPeer,
    toon_name: &str,
    channel_view: &mut Option<ChannelView>,
    command: &str,
    reporter: &ClientReporter,
) -> Result<(), ServeError> {
    let command = command.trim();
    if !command.starts_with('/') {
        if channel_view.is_none() {
            peer.send_error("Join a channel before sending a message.")
                .map_err(ServeError::Client)?;
            return Ok(());
        }
        if let Err(error) = session.send_message(command) {
            peer.send_error(&format!("Message was not sent: {error}"))
                .map_err(ServeError::Client)?;
        } else {
            reporter.activity(ActivityKind::Legacy, format!("chat: {command}"));
        }
        return Ok(());
    }

    let (name, argument) = split_command(&command[1..]);
    match name.to_ascii_lowercase().as_str() {
        "join" | "j" | "channel" => {
            handle_join_command(session, peer, argument, reporter)?;
        }
        "rejoin" => {
            if channel_view.is_some() {
                send_channel_snapshot(session, peer, toon_name, channel_view, reporter)
                    .map_err(ServeError::Client)?;
            } else {
                peer.send_error("Join a channel first.")
                    .map_err(ServeError::Client)?;
            }
        }
        "who" | "users" => {
            if channel_view.is_none() {
                peer.send_error("Join a channel first.")
                    .map_err(ServeError::Client)?;
                return Ok(());
            }
            let names = session
                .active_channel()
                .map_or_else(String::new, |channel| {
                    channel
                        .users
                        .iter()
                        .map(|user| user.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                });
            peer.send_chat_event(EID_INFO, "SC2 Gateway", &names, 0, 0)
                .map_err(ServeError::Client)?;
        }
        "whoami" | "where" | "whereis" => {
            peer.send_chat_event(EID_INFO, "Battle.net", &whoami_message(toon_name), 0, 0)
                .map_err(ServeError::Client)?;
        }
        "w" | "whisper" | "msg" => {
            let (target, body) = split_command(argument);
            if let Err(error) = session.send_whisper(target, body) {
                peer.send_error(&format!("Whisper was not sent: {error}"))
                    .map_err(ServeError::Client)?;
            } else {
                reporter.activity(ActivityKind::Legacy, format!("whisper to {target}: {body}"));
            }
        }
        "me" | "emote" => {
            if channel_view.is_none() {
                peer.send_error("Join a channel before sending an emote.")
                    .map_err(ServeError::Client)?;
            } else if argument.is_empty() {
                peer.send_error("Emote text cannot be empty.")
                    .map_err(ServeError::Client)?;
            } else if let Err(error) = session.send_message(&format!("* {toon_name} {argument} *"))
            {
                peer.send_error(&format!("Emote was not sent: {error}"))
                    .map_err(ServeError::Client)?;
            } else {
                reporter.activity(ActivityKind::Legacy, format!("emote: {argument}"));
            }
        }
        "help" => {
            peer.send_chat_event(
                EID_INFO,
                "SC2 Gateway",
                "Supported: /join, /rejoin, /who, /whisper, /me",
                0,
                0,
            )
            .map_err(ServeError::Client)?;
        }
        _ => {
            peer.send_error("That slash command is not supported by the SC2 gateway.")
                .map_err(ServeError::Client)?;
        }
    }
    Ok(())
}

fn handle_join_command(
    session: &mut Session,
    peer: &mut BotPeer,
    requested: &str,
    reporter: &ClientReporter,
) -> Result<(), ServeError> {
    let channel = match normalize_channel_name(requested) {
        Ok(channel) => channel,
        Err(message) => {
            peer.send_error(message).map_err(ServeError::Client)?;
            return Ok(());
        }
    };
    if let Err(error) = session.join(&channel) {
        peer.send_error(&format!("Could not join channel: {error}"))
            .map_err(ServeError::Client)?;
    } else {
        reporter.activity(ActivityKind::Upstream, format!("joining {channel}"));
    }
    Ok(())
}

fn split_command(command: &str) -> (&str, &str) {
    command
        .split_once(char::is_whitespace)
        .map_or((command, ""), |(name, argument)| (name, argument.trim()))
}

fn normalize_channel_name(channel: &str) -> Result<String, &'static str> {
    let channel = channel.chars().take(31).collect::<String>();
    if channel.chars().any(char::is_control) {
        Err("Channel names cannot contain control characters.")
    } else {
        Ok(channel)
    }
}

fn whoami_message(toon_name: &str) -> String {
    // legacy gateway discovery extracts the final word as the namespace.
    format!("You are {toon_name}, using StarCraft SC2")
}

fn legacy_friends(session: &Session) -> Vec<FriendEntry> {
    session
        .friends()
        .iter()
        .take(usize::from(u8::MAX))
        .map(legacy_friend)
        .collect()
}

fn send_friends_snapshot(
    session: &Session,
    peer: &mut BotPeer,
    previous: &mut Option<Vec<FriendEntry>>,
) -> io::Result<()> {
    let friends = legacy_friends(session);
    peer.send_friends_list(&friends)?;
    *previous = Some(friends);
    Ok(())
}

fn legacy_friend(friend: &Friend) -> FriendEntry {
    let (status, location, product, location_name) = match friend.presence {
        FriendPresence::Available => (
            FRIEND_STATUS_MUTUAL,
            FRIEND_LOCATION_NOT_IN_CHAT,
            PRODUCT_STAR,
            "Battle.net",
        ),
        FriendPresence::Away => (
            FRIEND_STATUS_MUTUAL | FRIEND_STATUS_AWAY,
            FRIEND_LOCATION_NOT_IN_CHAT,
            PRODUCT_STAR,
            "Battle.net (Away)",
        ),
        FriendPresence::Busy => (
            FRIEND_STATUS_MUTUAL | FRIEND_STATUS_DND,
            FRIEND_LOCATION_NOT_IN_CHAT,
            PRODUCT_STAR,
            "Battle.net (Busy)",
        ),
        FriendPresence::InGame => (
            FRIEND_STATUS_MUTUAL,
            FRIEND_LOCATION_PUBLIC_GAME,
            PRODUCT_STAR,
            "StarCraft II",
        ),
        FriendPresence::Offline | FriendPresence::Unknown => {
            (FRIEND_STATUS_MUTUAL, FRIEND_LOCATION_OFFLINE, 0, "")
        }
    };
    FriendEntry {
        name: friend.name.clone(),
        status,
        location,
        product,
        location_name: location_name.into(),
    }
}

fn relay_event(
    session: &Session,
    peer: &mut BotPeer,
    event: &Event,
    toon_name: &str,
    channel_view: &mut Option<ChannelView>,
    reporter: &ClientReporter,
) -> io::Result<()> {
    match event {
        Event::Joined { .. } => {
            send_channel_snapshot(session, peer, toon_name, channel_view, reporter)
        }
        Event::RosterChanged { .. }
        | Event::MemberJoined { .. }
        | Event::MemberLeft { .. }
        | Event::FriendsChanged => Ok(()),
        Event::Message {
            channel_index,
            sender,
            body,
            outgoing,
        } => {
            if *outgoing {
                Ok(())
            } else if channel_view
                .as_ref()
                .is_some_and(|channel| channel.index == *channel_index)
            {
                peer.send_chat_event(EID_TALK, &sender.name, body, 0, 0)
            } else {
                Ok(())
            }
        }
        Event::Whisper {
            peer: whisper_peer,
            body,
            outgoing,
        } => peer.send_chat_event(
            if *outgoing {
                EID_WHISPER_SENT
            } else {
                EID_WHISPER
            },
            whisper_peer,
            body,
            0,
            0,
        ),
        Event::Error(message) => peer.send_chat_event(EID_ERROR, "SC2 Gateway", message, 0, 0),
    }
}

fn send_channel_snapshot(
    session: &Session,
    peer: &mut BotPeer,
    toon_name: &str,
    previous: &mut Option<ChannelView>,
    reporter: &ClientReporter,
) -> io::Result<()> {
    let Some(current) = session.active_channel().map(channel_view) else {
        peer.send_error("The SC2 session has no active channel.")?;
        return Ok(());
    };
    peer.send_chat_event(EID_CHANNEL, toon_name, &current.label, 0, 0)?;
    for user in current.users.values() {
        peer.send_chat_event(EID_SHOW_USER, &user.name, DEFAULT_STATS, 0, 0)?;
    }
    reporter.activity(
        ActivityKind::Legacy,
        format!("sent channel snapshot: {} users", current.users.len()),
    );
    *previous = Some(current);
    Ok(())
}

fn channel_view(channel: ChannelSnapshot) -> ChannelView {
    ChannelView {
        index: channel.index,
        label: channel.label,
        users: channel
            .users
            .into_iter()
            .map(|user| (user.handle, UserView { name: user.name }))
            .collect(),
    }
}

fn report_upstream(reporter: &ClientReporter, session: &Session) {
    let channel = session.active_channel();
    let (channel, roster) = channel.map_or_else(
        || (None, Vec::new()),
        |channel| {
            (
                Some(channel.label),
                channel.users.into_iter().map(|user| user.name).collect(),
            )
        },
    );
    reporter.send(GatewayEvent::Upstream {
        peer: reporter.address,
        toon: session.toon_name().to_owned(),
        channel,
        roster,
        friends: session.friends().len(),
    });
}

fn report_sc2_event(reporter: &ClientReporter, event: &Event) {
    match event {
        Event::Joined { .. } => reporter.activity(ActivityKind::Upstream, "channel joined"),
        Event::RosterChanged { channel_index } => {
            trace_roster_event(
                reporter,
                format!("roster snapshot changed in channel {channel_index}"),
            );
        }
        Event::MemberJoined {
            channel_index,
            user,
        } => {
            trace_roster_event(
                reporter,
                format!(
                    "member joined channel {channel_index}: {} ({})",
                    user.name, user.handle
                ),
            );
        }
        Event::MemberLeft {
            channel_index,
            user,
        } => {
            trace_roster_event(
                reporter,
                format!(
                    "member left channel {channel_index}: {} ({})",
                    user.name, user.handle
                ),
            );
        }
        Event::FriendsChanged => {}
        Event::Message {
            sender,
            body,
            outgoing,
            ..
        } => {
            let direction = if *outgoing { "sent" } else { "received" };
            reporter.activity(
                ActivityKind::Upstream,
                format!("{direction} {}: {body}", sender.name),
            );
        }
        Event::Whisper {
            peer,
            body,
            outgoing,
        } => {
            let direction = if *outgoing { "to" } else { "from" };
            reporter.activity(
                ActivityKind::Upstream,
                format!("whisper {direction} {peer}: {body}"),
            );
        }
        Event::Error(message) => reporter.activity(ActivityKind::Error, message),
    }
}

fn trace_roster_event(reporter: &ClientReporter, message: String) {
    if std::env::var_os("SUPERIORITY_GATEWAY_TRACE_ROSTER").is_some() {
        reporter.activity(ActivityKind::Upstream, message);
    }
}

fn event_changes_snapshot(event: &Event) -> bool {
    matches!(
        event,
        Event::Joined { .. }
            | Event::RosterChanged { .. }
            | Event::MemberJoined { .. }
            | Event::MemberLeft { .. }
            | Event::FriendsChanged
    )
}

#[derive(Debug, Default, Eq, PartialEq)]
#[expect(
    dead_code,
    reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
)]
struct RosterDelta {
    joined: usize,
    left: usize,
}

impl RosterDelta {
    #[expect(
        dead_code,
        reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
    )]
    const fn is_empty(&self) -> bool {
        self.joined == 0 && self.left == 0
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
#[expect(
    dead_code,
    reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
)]
struct FriendDelta {
    added: usize,
    removed: usize,
    moved: usize,
    updated: usize,
}

impl FriendDelta {
    #[expect(
        dead_code,
        reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
    )]
    const fn is_empty(&self) -> bool {
        self.added == 0 && self.removed == 0 && self.moved == 0 && self.updated == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    dead_code,
    reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
)]
enum FriendOperation {
    Add(FriendEntry),
    Remove(u8),
    Move { old: u8, new: u8 },
    Update { index: u8, friend: FriendEntry },
}

#[expect(
    dead_code,
    reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
)]
fn synchronize_friends(
    session: &Session,
    peer: &mut BotPeer,
    previous: &mut Option<Vec<FriendEntry>>,
) -> io::Result<FriendDelta> {
    let Some(old) = previous.as_ref() else {
        return Ok(FriendDelta::default());
    };
    let current = legacy_friends(session);
    let operations = friend_operations(old, &current);
    let mut delta = FriendDelta::default();
    for operation in operations {
        match operation {
            FriendOperation::Add(friend) => {
                peer.send_friend_add(&friend)?;
                delta.added += 1;
            }
            FriendOperation::Remove(index) => {
                peer.send_friend_remove(index)?;
                delta.removed += 1;
            }
            FriendOperation::Move { old, new } => {
                peer.send_friend_position(old, new)?;
                delta.moved += 1;
            }
            FriendOperation::Update { index, friend } => {
                peer.send_friend_update(index, &friend)?;
                delta.updated += 1;
            }
        }
    }
    *previous = Some(current);
    Ok(delta)
}

#[expect(
    dead_code,
    reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
)]
fn friend_operations(old: &[FriendEntry], current: &[FriendEntry]) -> Vec<FriendOperation> {
    let mut working = old.to_vec();
    let mut operations = Vec::new();

    for index in (0..working.len()).rev() {
        if !current
            .iter()
            .any(|friend| friend.name == working[index].name)
        {
            operations.push(FriendOperation::Remove(friend_index(index)));
            working.remove(index);
        }
    }

    for (target_index, desired) in current.iter().enumerate() {
        if working
            .get(target_index)
            .is_some_and(|friend| friend.name == desired.name)
        {
            update_friend_if_needed(&mut working, &mut operations, target_index, desired);
            continue;
        }

        if let Some(old_index) = working
            .iter()
            .position(|friend| friend.name == desired.name)
        {
            operations.push(FriendOperation::Move {
                old: friend_index(old_index),
                new: friend_index(target_index),
            });
            let moved = working.remove(old_index);
            working.insert(target_index, moved);
            update_friend_if_needed(&mut working, &mut operations, target_index, desired);
        } else {
            operations.push(FriendOperation::Add(desired.clone()));
            working.push(desired.clone());
            let added_index = working.len() - 1;
            if added_index != target_index {
                operations.push(FriendOperation::Move {
                    old: friend_index(added_index),
                    new: friend_index(target_index),
                });
                let added = working.remove(added_index);
                working.insert(target_index, added);
            }
        }
    }
    debug_assert_eq!(working, current);
    operations
}

#[expect(
    dead_code,
    reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
)]
fn update_friend_if_needed(
    working: &mut [FriendEntry],
    operations: &mut Vec<FriendOperation>,
    index: usize,
    desired: &FriendEntry,
) {
    if working[index] != *desired {
        operations.push(FriendOperation::Update {
            index: friend_index(index),
            friend: desired.clone(),
        });
        working[index] = desired.clone();
    }
}

#[expect(
    dead_code,
    reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
)]
fn friend_index(index: usize) -> u8 {
    u8::try_from(index).expect("friend snapshots are capped at 255 entries")
}

#[expect(
    dead_code,
    reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
)]
fn synchronize_roster(
    session: &Session,
    peer: &mut BotPeer,
    previous: &mut Option<ChannelView>,
) -> io::Result<RosterDelta> {
    let Some(current) = session.active_channel().map(channel_view) else {
        return Ok(RosterDelta::default());
    };
    let Some(old) = previous.as_ref() else {
        return Ok(RosterDelta::default());
    };
    if old.index != current.index {
        return Ok(RosterDelta::default());
    }

    let mut delta = RosterDelta::default();
    for (handle, user) in &old.users {
        if current
            .users
            .get(handle)
            .is_none_or(|current_user| current_user != user)
        {
            peer.send_chat_event(EID_LEAVE, &user.name, "", 0, 0)?;
            delta.left += 1;
        }
    }
    for (handle, user) in &current.users {
        if old
            .users
            .get(handle)
            .is_none_or(|old_user| old_user != user)
        {
            peer.send_chat_event(EID_JOIN, &user.name, DEFAULT_STATS, 0, 0)?;
            delta.joined += 1;
        }
    }
    *previous = Some(current);
    Ok(delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn friend_entry(name: &str, status: u8) -> FriendEntry {
        FriendEntry {
            name: name.into(),
            status,
            location: FRIEND_LOCATION_NOT_IN_CHAT,
            product: PRODUCT_STAR,
            location_name: "Battle.net".into(),
        }
    }

    #[test]
    fn command_split_preserves_the_message_tail() {
        assert_eq!(
            split_command("w Raynor hello there"),
            ("w", "Raynor hello there")
        );
        assert_eq!(
            split_command("Raynor hello there"),
            ("Raynor", "hello there")
        );
    }

    #[test]
    fn legacy_users_are_presented_as_starcraft_clients() {
        assert_eq!(DEFAULT_STATS, "RATS");
        assert_eq!(DEFAULT_STATS.chars().rev().collect::<String>(), "STAR");
    }

    #[test]
    fn battle_net_presence_maps_to_bncs_friend_status() {
        let away = legacy_friend(&Friend {
            name: "Kerrigan".into(),
            presence: FriendPresence::Away,
        });
        assert_eq!(away.status, FRIEND_STATUS_MUTUAL | FRIEND_STATUS_AWAY);
        assert_eq!(away.location, FRIEND_LOCATION_NOT_IN_CHAT);
        assert_eq!(away.product, PRODUCT_STAR);

        let in_game = legacy_friend(&Friend {
            name: "Raynor".into(),
            presence: FriendPresence::InGame,
        });
        assert_eq!(in_game.location, FRIEND_LOCATION_PUBLIC_GAME);
        assert_eq!(in_game.location_name, "StarCraft II");

        let offline = legacy_friend(&Friend {
            name: "Artanis".into(),
            presence: FriendPresence::Unknown,
        });
        assert_eq!(offline.location, FRIEND_LOCATION_OFFLINE);
        assert_eq!(offline.product, 0);
    }

    #[test]
    fn whoami_response_satisfies_stealthbot_gateway_discovery() {
        let message = whoami_message("ncarrillo");
        assert!(message.starts_with("You are "));
        assert!(message.contains(", using "));
        assert!(!message.contains(" in channel "));
        assert_eq!(message.split_whitespace().next_back(), Some("SC2"));
    }

    #[test]
    fn channel_names_follow_bncs_length_and_control_character_rules() {
        assert_eq!(
            normalize_channel_name("123456789012345678901234567890123456").expect("valid channel"),
            "1234567890123456789012345678901"
        );
        assert!(normalize_channel_name("bad\nchannel").is_err());
    }

    #[test]
    fn friend_diff_uses_updates_for_presence_only_changes() {
        let old = vec![friend_entry("Kerrigan", FRIEND_STATUS_MUTUAL)];
        let current = vec![friend_entry(
            "Kerrigan",
            FRIEND_STATUS_MUTUAL | FRIEND_STATUS_AWAY,
        )];

        assert_eq!(
            friend_operations(&old, &current),
            vec![FriendOperation::Update {
                index: 0,
                friend: current[0].clone(),
            }]
        );
    }

    #[test]
    fn friend_diff_preserves_bncs_entry_order() {
        let raynor = friend_entry("Raynor", FRIEND_STATUS_MUTUAL);
        let kerrigan = friend_entry("Kerrigan", FRIEND_STATUS_MUTUAL);
        let artanis = friend_entry("Artanis", FRIEND_STATUS_MUTUAL);

        assert_eq!(
            friend_operations(
                &[raynor.clone(), kerrigan.clone()],
                &[kerrigan.clone(), raynor.clone()],
            ),
            vec![FriendOperation::Move { old: 1, new: 0 }]
        );
        assert_eq!(
            friend_operations(&[raynor, kerrigan], std::slice::from_ref(&artanis)),
            vec![
                FriendOperation::Remove(1),
                FriendOperation::Remove(0),
                FriendOperation::Add(artanis),
            ]
        );
    }

    #[test]
    fn reader_reports_eof_and_marks_the_connection_closed() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let client = TcpStream::connect(listener.local_addr().expect("listener address"))
            .expect("connect test client");
        let (server, _) = listener.accept().expect("accept test client");
        let connection_closed = Arc::new(AtomicBool::new(false));
        let (incoming, reader) = spawn_reader(server, Arc::clone(&connection_closed));

        drop(client);

        assert!(matches!(
            incoming
                .recv_timeout(Duration::from_secs(1))
                .expect("reader completion"),
            ClientInput::Closed(None)
        ));
        reader.join().expect("reader thread");
        assert!(connection_closed.load(Ordering::Acquire));
    }
}
