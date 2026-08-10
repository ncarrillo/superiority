use std::{
    io::ErrorKind,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use url::Url;

use crate::{
    Error, Result,
    auth::{CredentialStore, FileCredentialStore, authenticate_cached},
    bgs::{Client, Endpoint, SecretBytes},
    chat::{ChatChannel, ChatEvent, LiveChat},
    native::{Connector, Protocol, WhisperTarget, protocol::MAX_JOINED_CHANNELS},
    observer::{SessionObserver, SessionObserverFactory},
};

pub const DEFAULT_PUBLIC_CHANNEL: u16 = 1033;
const LIVE_POLL_INTERVAL: Duration = Duration::from_millis(200);
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(60);
const TRANSPORT_MAINTENANCE_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone, Debug)]
pub enum ClientCommand {
    Connect {
        force_interactive: bool,
        channels: Vec<ChatChannel>,
    },
    Disconnect,
    SignOut,
    JoinChannel(ChatChannel),
    LeaveChannel {
        channel_index: u8,
    },
    SendMessage {
        channel_index: u8,
        body: String,
    },
    SendWhisper {
        target: WhisperTarget,
        display_name: String,
        body: String,
    },
    AnswerGroupInvitation {
        club_id: u32,
        accept: bool,
    },
    AnswerPartyInvitation {
        channel_index: u8,
        accept: bool,
    },
    SearchGroups {
        query: String,
    },
    Quit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionStage {
    Disconnected,
    WebAuthentication,
    GameUtilities,
    NativeAuthentication,
    ChatBootstrap,
    Connected,
}

#[derive(Debug)]
pub enum ClientEvent {
    Stage(ConnectionStage),
    Authentication {
        url: Url,
        reply: Sender<Result<SecretBytes>>,
    },
    Chat(ChatEvent),
    CommandError(String),
    Error(String),
}

pub struct ClientHandle {
    pub commands: Sender<ClientCommand>,
    pub events: Receiver<ClientEvent>,
}

#[must_use]
pub fn spawn_client(observer: Box<dyn SessionObserverFactory>) -> ClientHandle {
    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    thread::Builder::new()
        .name("sc2-network".into())
        .spawn(move || run_worker(&command_rx, &event_tx, observer.as_ref()))
        .expect("network thread must start");
    ClientHandle {
        commands: command_tx,
        events: event_rx,
    }
}

fn run_worker(
    commands: &Receiver<ClientCommand>,
    events: &Sender<ClientEvent>,
    observer: &dyn SessionObserverFactory,
) {
    while let Ok(command) = commands.recv() {
        match command {
            ClientCommand::Connect {
                force_interactive,
                channels,
            } => {
                if let Err(error) =
                    connect_once(commands, events, observer, force_interactive, channels)
                {
                    emit(events, ClientEvent::Error(error.to_string()));
                    emit(events, ClientEvent::Stage(ConnectionStage::Disconnected));
                }
            }
            ClientCommand::SignOut => {
                if let Err(error) = FileCredentialStore::default().delete() {
                    emit(events, ClientEvent::Error(error.to_string()));
                }
                emit(events, ClientEvent::Stage(ConnectionStage::Disconnected));
            }
            ClientCommand::Quit => break,
            ClientCommand::Disconnect
            | ClientCommand::JoinChannel(_)
            | ClientCommand::LeaveChannel { .. }
            | ClientCommand::SendMessage { .. }
            | ClientCommand::SendWhisper { .. }
            | ClientCommand::AnswerGroupInvitation { .. }
            | ClientCommand::AnswerPartyInvitation { .. }
            | ClientCommand::SearchGroups { .. } => {}
        }
    }
}

fn connect_once(
    commands: &Receiver<ClientCommand>,
    events: &Sender<ClientEvent>,
    observer: &dyn SessionObserverFactory,
    force_interactive: bool,
    channels: Vec<ChatChannel>,
) -> Result<()> {
    let channels = normalized_channels(channels);
    let protocol = load_protocol()?;
    emit(
        events,
        ClientEvent::Stage(ConnectionStage::WebAuthentication),
    );
    let mut bgs = Client::open(&Endpoint::default())?;
    bgs.establish()?;

    let mut browser = |url: &Url| {
        let (reply, response) = mpsc::channel();
        events
            .send(ClientEvent::Authentication {
                url: url.clone(),
                reply,
            })
            .map_err(|_| application_closed())?;
        response.recv().map_err(|_| application_closed())?
    };
    let authentication = authenticate_cached(
        &mut bgs,
        &FileCredentialStore::default(),
        &mut browser,
        force_interactive,
    )?;

    emit(events, ClientEvent::Stage(ConnectionStage::GameUtilities));
    let bootstrap = bgs.process_client_request(&authentication.session)?;
    emit(
        events,
        ClientEvent::Stage(ConnectionStage::NativeAuthentication),
    );
    let native = Connector::new(protocol, Endpoint::default().timeout)
        .authenticate_with_handoff(&bootstrap, || bgs.close())?;
    emit(events, ClientEvent::Stage(ConnectionStage::ChatBootstrap));
    let (mut chat, initial_events) = LiveChat::establish(native, channels[0].clone())?;
    for channel in channels.iter().skip(1).cloned() {
        chat.join_channel(channel)?;
    }
    chat.set_read_timeout(Some(LIVE_POLL_INTERVAL))?;
    let mut tap = observer.begin_session();
    for event in initial_events {
        tap.observe(&event);
        emit(events, ClientEvent::Chat(event));
    }
    emit(events, ClientEvent::Stage(ConnectionStage::Connected));
    run_live(commands, events, chat, tap.as_mut())
}

fn normalized_channels(channels: Vec<ChatChannel>) -> Vec<ChatChannel> {
    let mut normalized = Vec::new();
    for channel in channels {
        if !normalized.contains(&channel) {
            normalized.push(channel);
        }
    }
    if normalized.is_empty() {
        normalized.push(ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL));
    }
    normalized.truncate(MAX_JOINED_CHANNELS);
    normalized
}

fn run_live(
    commands: &Receiver<ClientCommand>,
    events: &Sender<ClientEvent>,
    mut chat: LiveChat,
    tap: &mut dyn SessionObserver,
) -> Result<()> {
    let mut next_keep_alive = Instant::now();
    let mut next_transport_maintenance = Instant::now();
    loop {
        loop {
            match commands.try_recv() {
                Ok(ClientCommand::Disconnect) => {
                    chat.close()?;
                    emit(events, ClientEvent::Stage(ConnectionStage::Disconnected));
                    return Ok(());
                }
                Ok(ClientCommand::SignOut) => {
                    let _ = chat.close();
                    FileCredentialStore::default().delete()?;
                    emit(events, ClientEvent::Stage(ConnectionStage::Disconnected));
                    return Ok(());
                }
                Ok(ClientCommand::Quit) | Err(TryRecvError::Disconnected) => {
                    let _ = chat.close();
                    return Err(application_closed());
                }
                Ok(ClientCommand::JoinChannel(channel)) => {
                    chat.join_channel(channel)?;
                }
                Ok(ClientCommand::LeaveChannel { channel_index }) => {
                    chat.leave_channel(channel_index)?;
                    tap.observe_left(channel_index);
                }
                Ok(ClientCommand::SendMessage {
                    channel_index,
                    body,
                }) => {
                    chat.send_message(channel_index, body.trim())?;
                }
                Ok(ClientCommand::SendWhisper {
                    target,
                    display_name,
                    body,
                }) => {
                    if let Err(error) = chat.send_whisper(&target, &display_name, body.trim()) {
                        match error {
                            Error::Native(message) => {
                                emit(events, ClientEvent::CommandError(message));
                            }
                            error => return Err(error),
                        }
                    }
                }
                Ok(ClientCommand::AnswerGroupInvitation { club_id, accept }) => {
                    if let Err(error) = chat.answer_group_invitation(club_id, accept) {
                        emit(events, ClientEvent::CommandError(error.to_string()));
                    }
                }
                Ok(ClientCommand::AnswerPartyInvitation {
                    channel_index,
                    accept,
                }) => {
                    if let Err(error) = chat.answer_party_invitation(channel_index, accept) {
                        emit(events, ClientEvent::CommandError(error.to_string()));
                    }
                }
                Ok(ClientCommand::SearchGroups { query }) => {
                    let _ = chat.search_groups(&query);
                }
                Ok(ClientCommand::Connect { .. }) => {}
                Err(TryRecvError::Empty) => break,
            }
        }

        if Instant::now() >= next_keep_alive {
            chat.keep_alive()?;
            tap.heartbeat();
            next_keep_alive = Instant::now() + KEEP_ALIVE_INTERVAL;
        }
        if Instant::now() >= next_transport_maintenance {
            chat.maintain_transport()?;
            next_transport_maintenance = Instant::now() + TRANSPORT_MAINTENANCE_INTERVAL;
        }

        match chat.receive() {
            Ok(event) => {
                tap.observe(&event);
                emit(events, ClientEvent::Chat(event));
            }
            Err(Error::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(error) => return Err(error),
        }
    }
}

fn load_protocol() -> Result<Protocol> {
    Protocol::current()
}

fn emit(events: &Sender<ClientEvent>, event: ClientEvent) {
    let _ = events.send(event);
}

fn application_closed() -> Error {
    Error::Transport("application closed".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_client_protocol_is_compiled_in() {
        let protocol = load_protocol().unwrap();
        assert!(protocol.codec().schema().runtime_metadata().is_none());
    }

    #[test]
    fn channels_keep_order_remove_duplicates_and_have_a_fallback() {
        let op = ChatChannel::Private("Op Test".into());
        assert_eq!(
            normalized_channels(vec![op.clone(), ChatChannel::Public(1033), op.clone()]),
            vec![op, ChatChannel::Public(1033)]
        );
        assert_eq!(
            normalized_channels(Vec::new()),
            vec![ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL)]
        );
    }

    #[test]
    fn channels_stop_at_the_number_the_service_can_address() {
        let asked = (0..12)
            .map(|index| ChatChannel::Private(format!("Room {index}")))
            .collect::<Vec<_>>();

        let joined = normalized_channels(asked.clone());

        assert_eq!(joined.len(), MAX_JOINED_CHANNELS);
        assert_eq!(joined, asked[..MAX_JOINED_CHANNELS]);
    }
}
