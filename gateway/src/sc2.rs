use std::{
    collections::BTreeMap,
    sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError},
    time::{Duration, Instant},
};

use superiority_core::{
    Error, Result,
    chat::{ChatChannel, ChatEvent, ChatUser, PublicChannel, channel_title, strip_character_code},
    connection::{
        ClientCommand, ClientEvent, ConnectionStage, DEFAULT_PUBLIC_CHANNEL, spawn_client,
    },
    native::{PresenceState, WhisperTarget},
    observer::NoObserver,
    product::Product,
};

use crate::web_auth::{Cancellation, Requester};

const CONNECT_POLL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct User {
    pub handle: u32,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChannelSnapshot {
    pub index: u8,
    pub label: String,
    pub users: Vec<User>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FriendPresence {
    Available,
    Away,
    Busy,
    InGame,
    Offline,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Friend {
    pub name: String,
    pub presence: FriendPresence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    Joined {
        channel_index: u8,
    },
    RosterChanged {
        channel_index: u8,
    },
    MemberJoined {
        channel_index: u8,
        user: User,
    },
    MemberLeft {
        channel_index: u8,
        user: User,
    },
    Message {
        channel_index: u8,
        sender: User,
        body: String,
        outgoing: bool,
    },
    Whisper {
        peer: String,
        body: String,
        outgoing: bool,
    },
    FriendsChanged,
    Error(String),
}

#[derive(Debug, Default)]
struct ChannelState {
    channel: Option<ChatChannel>,
    local_member_handle: Option<u32>,
    users: BTreeMap<u32, ChatUser>,
}

pub struct Session {
    commands: Sender<ClientCommand>,
    events: Receiver<ClientEvent>,
    authentication: Requester,
    cancellation: Cancellation,
    login_timeout: Duration,
    public_channels: Vec<PublicChannel>,
    channels: BTreeMap<u8, ChannelState>,
    active_channel_index: Option<u8>,
    pending_channel: Option<ChatChannel>,
    pending_announcement: Option<u8>,
    toon_name: String,
    friends: Vec<Friend>,
}

impl Session {
    pub fn connect(
        authentication: Requester,
        cancellation: Cancellation,
        force_interactive: bool,
        startup_timeout: Duration,
        login_timeout: Duration,
        progress: &mut dyn FnMut(&str),
    ) -> Result<Self> {
        let handle = spawn_client(Product::StarCraft2, Box::new(NoObserver));
        handle
            .commands
            .send(ClientCommand::Connect {
                force_interactive,
                expected_account_id: None,
                expected_battle_tag: None,
                channels: vec![ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL)],
            })
            .map_err(|_| Error::Transport("SC2 network worker did not start".into()))?;

        let mut session = Self {
            commands: handle.commands,
            events: handle.events,
            authentication,
            cancellation,
            login_timeout,
            public_channels: Vec::new(),
            channels: BTreeMap::new(),
            active_channel_index: None,
            pending_channel: None,
            pending_announcement: None,
            toon_name: String::new(),
            friends: Vec::new(),
        };
        let mut deadline = Instant::now() + startup_timeout;
        let mut connected = false;

        while !connected || session.toon_name.is_empty() {
            if (session.cancellation)() {
                session.stop_worker();
                return Err(connection_closed());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                session.stop_worker();
                return Err(Error::Transport(format!(
                    "SC2 connection did not become ready within {} seconds",
                    startup_timeout.as_secs()
                )));
            }
            let event = match session.events.recv_timeout(remaining.min(CONNECT_POLL)) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    session.stop_worker();
                    return Err(Error::Transport(
                        "SC2 network worker stopped during connection".into(),
                    ));
                }
            };
            match event {
                // who signed in and what they own: nothing the gateway serves
                ClientEvent::Account(_) | ClientEvent::ProductCredential { .. } => {}
                // the gateway serves StarCraft II only
                ClientEvent::Classic(_)
                | ClientEvent::ClassicChannel(_)
                | ClientEvent::ClassicFriends(_)
                | ClientEvent::ClassicWhisperSent { .. }
                | ClientEvent::Warcraft(_)
                | ClientEvent::WarcraftChannel(_)
                | ClientEvent::WarcraftFriends(_)
                | ClientEvent::WarcraftClan(_)
                | ClientEvent::WarcraftChannels(_) => {}
                ClientEvent::Stage(stage) => {
                    progress(stage_label(stage));
                    connected = stage == ConnectionStage::Connected;
                    if stage == ConnectionStage::Disconnected && !connected {
                        session.stop_worker();
                        return Err(Error::Transport(
                            "SC2 session disconnected before chat became ready".into(),
                        ));
                    }
                }
                ClientEvent::Authentication { url, reply, .. } => {
                    let result =
                        session
                            .authentication
                            .request(&url, login_timeout, &session.cancellation);
                    reply.send(result).map_err(|_| {
                        Error::Transport("SC2 authentication worker stopped".into())
                    })?;
                    // browser interaction uses the login deadline; subsequent
                    // network phases receive their own bounded interval.
                    deadline = Instant::now() + startup_timeout;
                }
                ClientEvent::Chat(event) => {
                    let _ = session.apply_chat(event)?;
                }
                ClientEvent::CommandError(message) | ClientEvent::Error(message) => {
                    session.stop_worker();
                    return Err(Error::Transport(message));
                }
            }
        }
        Ok(session)
    }

    #[must_use]
    pub fn toon_name(&self) -> &str {
        &self.toon_name
    }

    #[must_use]
    pub fn active_channel(&self) -> Option<ChannelSnapshot> {
        let index = self.active_channel_index?;
        self.channel_snapshot(index)
    }

    #[must_use]
    pub fn friends(&self) -> &[Friend] {
        &self.friends
    }

    #[must_use]
    pub fn channel_names(&self) -> Vec<String> {
        let mut names = self
            .public_channels
            .iter()
            .map(|channel| channel.name.clone())
            .collect::<Vec<_>>();
        if let Some(active) = self.active_channel()
            && !names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&active.label))
        {
            names.push(active.label);
        }
        names
    }

    #[must_use]
    pub fn channel_snapshot(&self, index: u8) -> Option<ChannelSnapshot> {
        let state = self.channels.get(&index)?;
        let channel = state.channel.as_ref()?;
        Some(ChannelSnapshot {
            index,
            label: self.channel_label(channel),
            users: state.users.values().map(user).collect(),
        })
    }

    pub fn poll(&mut self, timeout: Duration) -> Result<Vec<Event>> {
        let mut output = Vec::new();
        match self.events.recv_timeout(timeout) {
            Ok(event) => {
                if let Some(event) = self.handle_client_event(event)? {
                    output.push(event);
                }
            }
            Err(RecvTimeoutError::Timeout) => return Ok(output),
            Err(RecvTimeoutError::Disconnected) => {
                return Err(Error::Transport("SC2 network worker stopped".into()));
            }
        }

        loop {
            match self.events.try_recv() {
                Ok(event) => {
                    if let Some(event) = self.handle_client_event(event)? {
                        output.push(event);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err(Error::Transport("SC2 network worker stopped".into()));
                }
            }
        }
        Ok(output)
    }

    pub fn send_message(&self, body: &str) -> Result<()> {
        let body = body.trim();
        if body.is_empty() {
            return Ok(());
        }
        let channel_index = self
            .active_channel_index
            .ok_or_else(|| Error::Transport("the SC2 session has no active chat channel".into()))?;
        self.send(ClientCommand::SendMessage {
            channel_index,
            body: body.to_owned(),
        })
    }

    pub fn send_whisper(&self, target: &str, body: &str) -> Result<()> {
        let target = target.trim();
        let body = body.trim();
        if target.is_empty() || body.is_empty() {
            return Err(Error::Transport(
                "whisper requires a target and message".into(),
            ));
        }
        self.send(ClientCommand::SendWhisper {
            target: WhisperTarget::Name(target.to_owned()),
            display_name: target.to_owned(),
            body: body.to_owned(),
        })
    }

    pub fn join(&mut self, requested: &str) -> Result<()> {
        let requested = requested.trim();
        if requested.is_empty() {
            return Err(Error::Transport("channel name cannot be empty".into()));
        }
        if self.pending_channel.is_some() {
            return Err(Error::Transport(
                "another SC2 channel join is already pending".into(),
            ));
        }
        let channel = self.resolve_channel(requested);
        self.send(ClientCommand::JoinChannel(channel.clone()))?;
        self.pending_channel = Some(channel);
        Ok(())
    }

    pub fn join_default(&mut self) -> Result<()> {
        self.join(&DEFAULT_PUBLIC_CHANNEL.to_string())
    }

    pub fn leave_active_channel(&mut self) -> Result<bool> {
        let Some(channel_index) = self.active_channel_index else {
            return Ok(false);
        };
        self.send(ClientCommand::LeaveChannel { channel_index })?;
        self.active_channel_index = None;
        if self.pending_announcement == Some(channel_index) {
            self.pending_announcement = None;
        }
        self.channels.remove(&channel_index);
        Ok(true)
    }

    pub fn close(&mut self) {
        self.stop_worker();
    }

    fn resolve_channel(&self, requested: &str) -> ChatChannel {
        if let Ok(identifier) = requested.parse::<u16>() {
            return ChatChannel::Public(identifier);
        }
        self.public_channels
            .iter()
            .find(|channel| channel.name.eq_ignore_ascii_case(requested))
            .map_or_else(
                || ChatChannel::Private(requested.to_owned()),
                |channel| ChatChannel::Public(channel.identifier),
            )
    }

    fn handle_client_event(&mut self, event: ClientEvent) -> Result<Option<Event>> {
        match event {
            ClientEvent::Stage(ConnectionStage::Disconnected) => {
                Err(Error::Transport("SC2 session disconnected".into()))
            }
            ClientEvent::Stage(_) => Ok(None),
            // who signed in and what they own; the gateway serves one account
            // and has no use for either
            ClientEvent::Account(_) | ClientEvent::ProductCredential { .. } => Ok(None),
            ClientEvent::Authentication { url, reply, .. } => {
                let result =
                    self.authentication
                        .request(&url, self.login_timeout, &self.cancellation);
                reply
                    .send(result)
                    .map_err(|_| Error::Transport("SC2 authentication worker stopped".into()))?;
                Ok(None)
            }
            ClientEvent::Chat(event) => self.apply_chat(event),
            // the gateway serves StarCraft II only; its bridge has no shape for
            // Remastered's chat, and inventing one here would be a guess
            ClientEvent::Classic(_)
            | ClientEvent::ClassicChannel(_)
            | ClientEvent::ClassicFriends(_)
            | ClientEvent::ClassicWhisperSent { .. }
            | ClientEvent::Warcraft(_)
            | ClientEvent::WarcraftChannel(_)
            | ClientEvent::WarcraftFriends(_)
            | ClientEvent::WarcraftClan(_)
            | ClientEvent::WarcraftChannels(_) => Ok(None),
            ClientEvent::CommandError(message) => Ok(Some(Event::Error(message))),
            ClientEvent::Error(message) => Err(Error::Transport(message)),
        }
    }

    fn apply_chat(&mut self, event: ChatEvent) -> Result<Option<Event>> {
        match event {
            ChatEvent::PublicChannelCatalog(channels) => {
                self.public_channels = channels;
                Ok(None)
            }
            ChatEvent::Joined {
                channel_index,
                channel,
                local_member_handle,
                shard_index: _,
            } => self.apply_joined(channel_index, channel, local_member_handle),
            ChatEvent::JoinRejected { channel, reason } => {
                self.pending_channel = None;
                let name = channel
                    .as_ref()
                    .map_or_else(|| "requested channel".into(), channel_title);
                let suffix = reason.map_or_else(String::new, |value| format!(" (reason {value})"));
                Ok(Some(Event::Error(format!(
                    "Battle.net rejected {name}{suffix}"
                ))))
            }
            ChatEvent::Roster(snapshot) => Ok(self.apply_roster(snapshot)),
            ChatEvent::MemberJoined {
                channel_index,
                user: joined,
            } => Ok(self.apply_member_joined(channel_index, &joined)),
            ChatEvent::MemberLeft {
                channel_index,
                user: left,
                ..
            } => Ok(self.apply_member_left(channel_index, &left)),
            ChatEvent::Removed {
                channel_index,
                reason,
            } => Ok(self.apply_removed(channel_index, reason)),
            ChatEvent::Message {
                channel_index,
                sender,
                body,
            } => {
                let outgoing = self
                    .channels
                    .get(&channel_index)
                    .and_then(|channel| channel.local_member_handle)
                    == Some(sender.handle);
                Ok(
                    (self.active_channel_index == Some(channel_index)).then(|| Event::Message {
                        channel_index,
                        sender: user(&sender),
                        body,
                        outgoing,
                    }),
                )
            }
            ChatEvent::Whisper {
                peer,
                body,
                outgoing,
            } => Ok(Some(Event::Whisper {
                peer,
                body,
                outgoing,
            })),
            ChatEvent::WhisperFailed { peer, reason } => Ok(Some(Event::Error(format!(
                "Whisper to {peer} failed: {reason}"
            )))),
            ChatEvent::Friends(friends) => {
                let friends = friends
                    .into_iter()
                    .map(|friend| Friend {
                        name: friend.name,
                        presence: friend_presence(friend.presence),
                    })
                    .collect::<Vec<_>>();
                if self.friends == friends {
                    return Ok(None);
                }
                self.friends = friends;
                Ok(Some(Event::FriendsChanged))
            }
            ChatEvent::ConferenceDescriptions { .. }
            | ChatEvent::ConferenceMemberCounts { .. }
            | ChatEvent::BlockedAccounts(_)
            | ChatEvent::Activity { .. }
            | ChatEvent::GroupInvitation { .. }
            | ChatEvent::PartyInvitation { .. }
            | ChatEvent::GroupSummary { .. }
            | ChatEvent::GroupSearch { .. } => Ok(None),
        }
    }

    fn apply_joined(
        &mut self,
        channel_index: u8,
        channel: ChatChannel,
        local_member_handle: u32,
    ) -> Result<Option<Event>> {
        let should_activate =
            self.active_channel_index.is_none() || self.pending_channel.as_ref() == Some(&channel);
        let state = self.channels.entry(channel_index).or_default();
        state.channel = Some(channel);
        state.local_member_handle = Some(local_member_handle);
        self.identify_toon(channel_index);

        if !should_activate {
            return Ok(None);
        }
        let previous = self.active_channel_index.replace(channel_index);
        self.pending_channel = None;
        if let Some(previous) = previous.filter(|previous| *previous != channel_index) {
            self.send(ClientCommand::LeaveChannel {
                channel_index: previous,
            })?;
        }
        self.pending_announcement = Some(channel_index);
        Ok(None)
    }

    fn apply_roster(&mut self, snapshot: superiority_core::chat::RosterSnapshot) -> Option<Event> {
        let channel_index = snapshot.channel_index;
        let initial_complete = snapshot.initial_complete;
        let state = self.channels.entry(snapshot.channel_index).or_default();
        let users = snapshot
            .users
            .into_iter()
            .map(|user| (user.handle, user))
            .collect::<BTreeMap<_, _>>();
        let legacy_changed = !same_legacy_roster(&state.users, &users);
        state.users = users;
        self.identify_toon(channel_index);
        if self.active_channel_index != Some(channel_index) {
            return None;
        }
        if self.pending_announcement == Some(channel_index) {
            if initial_complete {
                self.pending_announcement = None;
                return Some(Event::Joined { channel_index });
            }
            return None;
        }
        legacy_changed.then_some(Event::RosterChanged { channel_index })
    }

    fn apply_member_joined(&mut self, channel_index: u8, joined: &ChatUser) -> Option<Event> {
        let previous = self
            .channels
            .entry(channel_index)
            .or_default()
            .users
            .insert(joined.handle, joined.clone());
        self.identify_toon(channel_index);
        (self.active_channel_index == Some(channel_index)
            && previous
                .as_ref()
                .is_none_or(|previous| legacy_user_name(previous) != legacy_user_name(joined)))
        .then(|| Event::MemberJoined {
            channel_index,
            user: user(joined),
        })
    }

    fn apply_member_left(&mut self, channel_index: u8, left: &ChatUser) -> Option<Event> {
        let removed = self
            .channels
            .get_mut(&channel_index)
            .and_then(|channel| channel.users.remove(&left.handle));
        (self.active_channel_index == Some(channel_index) && removed.is_some()).then(|| {
            Event::MemberLeft {
                channel_index,
                user: removed.as_ref().map_or_else(|| user(left), user),
            }
        })
    }

    fn apply_removed(&mut self, channel_index: u8, reason: Option<u16>) -> Option<Event> {
        self.channels.remove(&channel_index);
        if self.pending_announcement == Some(channel_index) {
            self.pending_announcement = None;
        }
        if self.active_channel_index != Some(channel_index) {
            return None;
        }
        self.active_channel_index = None;
        Some(Event::Error(reason.map_or_else(
            || "Battle.net removed this session from the active channel".into(),
            |value| {
                format!("Battle.net removed this session from the active channel (reason {value})")
            },
        )))
    }

    fn identify_toon(&mut self, channel_index: u8) {
        if !self.toon_name.is_empty() {
            return;
        }
        let Some(channel) = self.channels.get(&channel_index) else {
            return;
        };
        let Some(handle) = channel.local_member_handle else {
            return;
        };
        let Some(name) = channel
            .users
            .get(&handle)
            .and_then(|user| user.name.as_deref())
        else {
            return;
        };
        strip_character_code(name).clone_into(&mut self.toon_name);
    }

    fn channel_label(&self, channel: &ChatChannel) -> String {
        match channel {
            ChatChannel::Public(identifier) => self
                .public_channels
                .iter()
                .find(|candidate| candidate.identifier == *identifier)
                .map_or_else(
                    || channel_title(channel),
                    |candidate| candidate.name.clone(),
                ),
            _ => channel_title(channel),
        }
    }

    fn send(&self, command: ClientCommand) -> Result<()> {
        self.commands
            .send(command)
            .map_err(|_| Error::Transport("SC2 network worker stopped".into()))
    }

    fn stop_worker(&self) {
        let _ = self.commands.send(ClientCommand::Disconnect);
        let _ = self.commands.send(ClientCommand::Quit);
    }
}

fn connection_closed() -> Error {
    Error::Transport("legacy connection closed during SC2 startup".into())
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop_worker();
    }
}

fn user(source: &ChatUser) -> User {
    User {
        handle: source.handle,
        name: source.name.as_deref().map_or_else(
            || format!("User {}", source.handle),
            |name| strip_character_code(name).to_owned(),
        ),
    }
}

fn same_legacy_roster(left: &BTreeMap<u32, ChatUser>, right: &BTreeMap<u32, ChatUser>) -> bool {
    left.len() == right.len()
        && left.iter().all(|(handle, left_user)| {
            right.get(handle).is_some_and(|right_user| {
                legacy_user_name(left_user) == legacy_user_name(right_user)
            })
        })
}

fn legacy_user_name(source: &ChatUser) -> Option<&str> {
    source.name.as_deref().map(strip_character_code)
}

fn stage_label(stage: ConnectionStage) -> &'static str {
    match stage {
        ConnectionStage::Disconnected => "Disconnected",
        ConnectionStage::WebAuthentication => "Authenticating Battle.net account",
        ConnectionStage::GameUtilities => "Resolving SC2 game account",
        ConnectionStage::NativeAuthentication => "Authenticating SC2 native session",
        ConnectionStage::ChatBootstrap => "Loading SC2 chat",
        ConnectionStage::Connected => "SC2 chat connected",
    }
}

fn friend_presence(presence: PresenceState) -> FriendPresence {
    match presence {
        PresenceState::Available => FriendPresence::Available,
        PresenceState::Away => FriendPresence::Away,
        PresenceState::Busy => FriendPresence::Busy,
        PresenceState::InGame => FriendPresence::InGame,
        PresenceState::Offline => FriendPresence::Offline,
        PresenceState::Unknown => FriendPresence::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn session() -> Session {
        let (commands, _command_receiver) = mpsc::channel();
        let (_event_sender, events) = mpsc::channel();
        let (authentication, _authentication_requests) = crate::web_auth::channel();
        let cancellation: Cancellation = std::sync::Arc::new(|| false);
        Session {
            commands,
            events,
            authentication,
            cancellation,
            login_timeout: Duration::from_secs(1),
            public_channels: Vec::new(),
            channels: BTreeMap::new(),
            active_channel_index: None,
            pending_channel: None,
            pending_announcement: None,
            toon_name: String::new(),
            friends: Vec::new(),
        }
    }

    fn chat_user(handle: u32, name: &str, presence: PresenceState) -> ChatUser {
        ChatUser {
            handle,
            presence_id: Some(handle),
            name: Some(name.into()),
            clan_tag: None,
            avatar: None,
            presence,
        }
    }

    #[test]
    fn joined_channel_is_announced_only_after_initial_roster_completes() {
        let mut session = session();

        let event = session
            .apply_joined(7, ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL), 42)
            .expect("joining the first channel does not send a leave command");
        assert_eq!(event, None);
        assert_eq!(session.pending_announcement, Some(7));

        assert_eq!(
            session.apply_roster(superiority_core::chat::RosterSnapshot {
                channel_index: 7,
                initial_complete: false,
                users: Vec::new(),
            }),
            None
        );
        assert_eq!(session.pending_announcement, Some(7));

        assert_eq!(
            session.apply_roster(superiority_core::chat::RosterSnapshot {
                channel_index: 7,
                initial_complete: true,
                users: Vec::new(),
            }),
            Some(Event::Joined { channel_index: 7 })
        );
        assert_eq!(session.pending_announcement, None);
        assert_eq!(
            session.apply_roster(superiority_core::chat::RosterSnapshot {
                channel_index: 7,
                initial_complete: true,
                users: Vec::new(),
            }),
            None
        );
    }

    #[test]
    fn messages_from_the_local_member_are_marked_as_outgoing() {
        let mut session = session();
        session
            .apply_joined(7, ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL), 42)
            .expect("joining the first channel");

        let local = session
            .apply_chat(ChatEvent::Message {
                channel_index: 7,
                sender: ChatUser {
                    handle: 42,
                    presence_id: None,
                    name: Some("Raynor".into()),
                    clan_tag: None,
                    avatar: None,
                    presence: PresenceState::Available,
                },
                body: "local echo".into(),
            })
            .expect("local message")
            .expect("active-channel message");
        assert!(matches!(local, Event::Message { outgoing: true, .. }));

        let remote = session
            .apply_chat(ChatEvent::Message {
                channel_index: 7,
                sender: ChatUser {
                    handle: 43,
                    presence_id: None,
                    name: Some("Kerrigan".into()),
                    clan_tag: None,
                    avatar: None,
                    presence: PresenceState::Available,
                },
                body: "remote message".into(),
            })
            .expect("remote message")
            .expect("active-channel message");
        assert!(matches!(
            remote,
            Event::Message {
                outgoing: false,
                ..
            }
        ));
    }

    #[test]
    fn presence_enrichment_does_not_emit_a_legacy_roster_change() {
        let mut session = session();
        session.active_channel_index = Some(7);
        session.channels.insert(
            7,
            ChannelState {
                channel: Some(ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL)),
                users: BTreeMap::from([(42, chat_user(42, "Raynor#123", PresenceState::Unknown))]),
                ..ChannelState::default()
            },
        );

        let event = session.apply_roster(superiority_core::chat::RosterSnapshot {
            channel_index: 7,
            initial_complete: true,
            users: vec![chat_user(42, "Raynor#123", PresenceState::Available)],
        });

        assert_eq!(event, None);
        assert_eq!(
            session.channels[&7].users[&42].presence,
            PresenceState::Available
        );
    }

    #[test]
    fn a_legacy_visible_name_change_emits_a_roster_change() {
        let mut session = session();
        session.active_channel_index = Some(7);
        session.channels.insert(
            7,
            ChannelState {
                channel: Some(ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL)),
                users: BTreeMap::from([(42, chat_user(42, "Raynor#123", PresenceState::Unknown))]),
                ..ChannelState::default()
            },
        );

        let event = session.apply_roster(superiority_core::chat::RosterSnapshot {
            channel_index: 7,
            initial_complete: true,
            users: vec![chat_user(42, "Kerrigan#456", PresenceState::Unknown)],
        });

        assert_eq!(event, Some(Event::RosterChanged { channel_index: 7 }));
    }

    #[test]
    fn channel_names_include_the_catalog_and_active_private_channel() {
        let mut session = session();
        session.public_channels = vec![PublicChannel {
            identifier: DEFAULT_PUBLIC_CHANNEL,
            name: "General".into(),
        }];
        session.active_channel_index = Some(7);
        session.channels.insert(
            7,
            ChannelState {
                channel: Some(ChatChannel::Private("Op Raynor".into())),
                ..ChannelState::default()
            },
        );

        assert_eq!(session.channel_names(), ["General", "Op Raynor"]);
    }

    #[test]
    fn leave_active_channel_updates_local_state_and_commands_sc2() {
        let (commands, command_receiver) = mpsc::channel();
        let (_event_sender, events) = mpsc::channel();
        let (authentication, _authentication_requests) = crate::web_auth::channel();
        let cancellation: Cancellation = std::sync::Arc::new(|| false);
        let mut session = Session {
            commands,
            events,
            authentication,
            cancellation,
            login_timeout: Duration::from_secs(1),
            public_channels: Vec::new(),
            channels: BTreeMap::from([(
                7,
                ChannelState {
                    channel: Some(ChatChannel::Private("Op Raynor".into())),
                    ..ChannelState::default()
                },
            )]),
            active_channel_index: Some(7),
            pending_channel: None,
            pending_announcement: None,
            toon_name: "Raynor".into(),
            friends: Vec::new(),
        };

        assert!(session.leave_active_channel().expect("leave"));
        assert!(session.active_channel().is_none());
        assert!(!session.channels.contains_key(&7));
        assert!(matches!(
            command_receiver.recv().expect("leave command"),
            ClientCommand::LeaveChannel { channel_index: 7 }
        ));
    }
}
