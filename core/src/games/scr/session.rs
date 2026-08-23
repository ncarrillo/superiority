//! bringing a Remastered session up on the classic channel.
//!
//! The order is the retail client's, recovered from a paired capture: the
//! websocket carries `AuthSession` first, then a toon session, then the legacy
//! chat gateway is switched on, and only then is a channel joined. Each step is
//! a plain request/response; what makes it a sequence is that the server will
//! not answer the next one until the previous has landed.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use crate::{
    Error, Result,
    games::scr::{
        CLIENT_IDENTITY, GAME_VERSION,
        aurora::{AuroraSession, ChallengeHandler},
        auth,
        catalog::{method, service},
        chat::{self, ChatState, CommandCatalog, EventKind},
        client::{ClassicClient, Ignore, Request, is_quiet_read, request_trace},
        gateway,
        handoff::ClassicHandoff,
        profile::{self, Avatar},
        user::{self, PlayerStatus},
    },
    platform::wire::raw::{self as protobuf, Message},
};

/// U.S. East's id in the gateway catalogue and in each `GetToons` record.
pub const DEFAULT_GATEWAY_CATALOG_ID: u64 = 11;
pub const DEFAULT_GATEWAY_NAME: &str = "U.S. East";

/// where the retail client puts you on U.S. East: Public Chat 1.
///
/// A fixed id, not a search of the advertised list. The gateway does not have
/// to have advertised anything by the time the session is up, so looking for
/// "the first public channel" can find nothing and silently join nowhere.
pub const DEFAULT_CHANNEL: u32 = 9;

/// a signed-in Remastered session: the socket, and the chat state its frames
/// are folded into.
pub struct ClassicSession {
    client: ClassicClient,
    state: ChatState,
    channel: u32,
    timeout: Duration,
    /// numeric Battle.net account identity from the Aurora logon. Unlike a
    /// BattleTag, this cannot be renamed and is therefore the cross-product
    /// account binding.
    account_id: u64,
    /// the region Aurora signed this session in through. Kept because the card
    /// says it, and nothing below the account layer knows it.
    connected_region: u64,
    /// the BattleTag the logon named, when it named one. Kept for the same
    /// reason as the region: the account surface says who you are, and only
    /// the logon knows it before a roster answers.
    battle_tag: Option<String>,
    /// the toon the gateway signed in as — the name the session's own talk is
    /// written under until the roster has a member carrying our BattleTag.
    toon_name: String,
    /// a successful empty lookup is cached too: it means this toon has no
    /// selected avatar and must not be queried on every 200 ms poll.
    avatar_cache: BTreeMap<String, Option<Avatar>>,
    commands: CommandCatalog,
    player_status: Option<PlayerStatus>,
    watch: Watch,
}

/// how long a line of our own talk is remembered, so that a gateway echo of
/// it — should LegacyChat ever send one — is absorbed rather than shown twice.
const ECHO_WINDOW: Duration = Duration::from_secs(10);

/// how long a named join that answered without naming the room is given for
/// the roster to arrive before the next request is tried. The startup roster
/// lands well inside this; a join that has not by then is not coming.
const JOIN_GRACE: Duration = Duration::from_millis(1500);

/// how long one wait-for-the-roster poll blocks before its deadline is
/// checked again.
const POLL_STEP: Duration = Duration::from_millis(250);

/// what one way of joining a named room did: the room it landed in, or a
/// note for the error that says every way was tried.
struct JoinAttempt {
    moved: Option<u32>,
    note: String,
}

/// how many inbound frames are kept for a join's post-mortem.
const FRAME_LOG: usize = 32;

/// what the session keeps an eye on beside the state proper: the lines of our
/// own talk written locally — LegacyChat acknowledges `SendMessage` and never
/// echoes the sender's line back, so the transcript and the Live tap hear it
/// from here — and a short log of every inbound frame, so a join that does
/// not land can say exactly what the gateway sent back.
#[derive(Default)]
struct Watch {
    pending: VecDeque<(String, std::time::Instant)>,
    frames: VecDeque<String>,
    /// frames seen since the session began; the log above keeps the tail.
    seen: usize,
}

impl Watch {
    /// notes one inbound frame: its name (or hashes), and its size.
    fn saw(&mut self, frame: &crate::games::scr::rpc::Frame) {
        let header = &frame.header;
        let name = crate::games::scr::catalog::rpc_name(header.service_id, header.method_id)
            .unwrap_or_else(|| format!("{:08x}.{:08x}", header.service_id, header.method_id));
        let kind = if header.is_response() {
            "answer"
        } else {
            "callback"
        };
        self.frames
            .push_back(format!("{name} {kind} {}b", frame.body.len()));
        while self.frames.len() > FRAME_LOG {
            self.frames.pop_front();
        }
        self.seen += 1;
    }

    /// the frames logged since `mark` (a value of `seen` taken earlier).
    fn frames_since(&self, mark: usize) -> Vec<String> {
        let kept = self.frames.len();
        let skip = self.seen.saturating_sub(mark);
        self.frames
            .iter()
            .skip(kept.saturating_sub(skip))
            .cloned()
            .collect()
    }

    fn remember(&mut self, text: &str) {
        self.prune();
        self.pending
            .push_back((text.to_owned(), std::time::Instant::now()));
    }

    /// whether `text`, arriving under our own name, is a line we wrote
    /// ourselves a moment ago. each remembered line absorbs one echo.
    fn absorb(&mut self, text: &str) -> bool {
        self.prune();
        match self.pending.iter().position(|(pending, _)| pending == text) {
            Some(index) => {
                self.pending.remove(index);
                true
            }
            None => false,
        }
    }

    fn prune(&mut self) {
        let now = std::time::Instant::now();
        while self
            .pending
            .front()
            .is_some_and(|(_, written)| now.saturating_duration_since(*written) > ECHO_WINDOW)
        {
            self.pending.pop_front();
        }
    }
}

impl ClassicSession {
    /// opens the classic channel the handoff names and signs in on it.
    ///
    /// `session` is the shared Battle.net logon — the same one `StarCraft II`
    /// uses. Nothing here re-authenticates; the ticket and the session key are
    /// replayed, and the server answers with a proof that it holds the same
    /// key.
    pub fn establish(
        handoff: &ClassicHandoff,
        session: &AuroraSession,
        timeout: Duration,
    ) -> Result<Self> {
        let mut client = ClassicClient::connect(handoff, timeout)?;

        // SecureTransport can remain blocked after a peer closes, so the TCP
        // read timeout is not sufficient here. Keep the whole classic startup
        // bounded by cutting the cloned socket if it does not finish.
        let watchdog = ClassicConnectWatchdog::arm(client.interrupt()?, timeout);

        let outcome = (|| {
            let trace = request_trace();
            let body = auth::request(handoff, session, CLIENT_IDENTITY)?;
            // Aurora pushes the initial account friend roster while
            // AuthSession is still waiting for its response. The old Ignore
            // callback acknowledged those records and discarded them before
            // ChatState existed, leaving the shared Social surface empty for
            // the lifetime of the connection.
            let mut state = ChatState::new();
            let response = client.call(
                &Request::new(service::AUTHENTICATION, method::AUTH_SESSION, &body)
                    .with_trace(&trace),
                timeout,
                &mut |frame: &crate::games::scr::rpc::Frame| {
                    state.apply(frame);
                },
            )?;
            // the proof is checked for shape only: it is the server showing it holds
            // the session key, and we have no capture of the derivation to verify
            // it against
            let _proof = auth::parse_response(&response.body)?;

            let mut session = Self {
                client,
                state,
                channel: 0,
                timeout,
                account_id: session.account_low,
                connected_region: session.connected_region,
                battle_tag: session.battle_tag.clone(),
                toon_name: String::new(),
                avatar_cache: BTreeMap::new(),
                commands: CommandCatalog::default(),
                player_status: None,
                watch: Watch::default(),
            };
            session.start_toon_session()?;
            session.start_chat()?;
            Ok(session)
        })();

        watchdog.disarm();
        if watchdog.fired() {
            return Err(Error::ClassicWire(
                "Battle.net did not finish the Remastered connection in time".into(),
            ));
        }
        outcome
    }

    /// names the account's toons and pins the build. The legacy gateway will
    /// not talk until both have happened.
    fn start_toon_session(&mut self) -> Result<()> {
        let response = self.call_body(service::GAME_ACCOUNT, method::GET_TOONS, &[])?;
        let toon = toon_for_gateway(&response, DEFAULT_GATEWAY_CATALOG_ID)?;
        self.toon_name.clone_from(&toon.name);
        let version = Message::new().bytes(1, GAME_VERSION.as_bytes()).into_vec();
        self.call(service::GAME_VERSION, method::SET_GAME_VERSION, &version)?;
        let connect = legacy_connect_request(toon.id)?;
        self.call(service::LEGACY, method::LEGACY_CONNECT, &connect)?;
        Ok(())
    }

    /// switches the legacy chat gateway on and waits for its channel catalog.
    ///
    /// `LegacyChat.Connect` answers before its `ChannelsUpdated` callback, so
    /// the callback—not the empty RPC response—is the readiness signal used by
    /// the working Remastered client.
    fn start_chat(&mut self) -> Result<()> {
        self.call(service::LEGACY_CHAT, method::SET_ONLINE, &[])?;
        let connect = Message::new().varint(1, 0).into_vec();
        self.call(service::LEGACY_CHAT, method::CHAT_CONNECT, &connect)?;
        self.await_roster_change(self.state.roster_revision())?;
        self.subscribe_player_status()?;
        self.set_player_status(PlayerStatus::Online)?;
        self.load_command_catalog()?;
        let stats = self.gateway_stats()?;
        self.state.push_information("Connected to chat service.");
        self.state
            .push_information("Welcome to StarCraft: Remastered!");
        self.state.push_information(format!(
            "There are {} games being played.",
            stats.games_being_played
        ));
        self.state.push_information(format!(
            "There are {} players online.",
            stats.players_online
        ));
        self.state
            .push_information("Type /help for a list of commands.");
        Ok(())
    }

    pub fn join(&mut self, channel_id: u32) -> Result<()> {
        if channel_id == self.channel {
            return Ok(());
        }
        let previous = self.channel;
        let join = chat::channel_request(channel_id)?;
        // the SDK leaves the room it is in before it joins another, and the
        // gateway was seen to acknowledge joins sent from inside a room and
        // do nothing. the transport that once made leaving fatal is fixed,
        // and a join that still does not land puts the session back where it
        // was. the roster is what says the join actually happened; returning
        // before it lands leaves a channel that looks joined and has nobody
        // in it
        self.leave_current_channel()?;
        let revision = self.state.roster_revision();
        self.call(service::LEGACY_CHAT, method::JOIN_CHANNEL, &join)?;
        let patience = if previous == 0 {
            self.timeout
        } else {
            JOIN_GRACE
        };
        match self.await_channel(channel_id, revision, patience) {
            Ok(()) => {
                self.channel = channel_id;
                Ok(())
            }
            Err(error) => {
                let note = self.rejoin(previous);
                Err(Error::ClassicWire(format!(
                    "{error}{}",
                    note.map(|note| format!(" ({note})")).unwrap_or_default()
                )))
            }
        }
    }

    fn leave_current_channel(&mut self) -> Result<()> {
        if self.channel == 0 {
            return Ok(());
        }
        let leave = chat::channel_request(self.channel)?;
        self.call(service::LEGACY_CHAT, method::LEAVE_CHANNEL, &leave)?;
        self.channel = 0;
        Ok(())
    }

    /// puts the session back in the room it left for a join that did not
    /// land; the note is for the error that reports the join.
    fn rejoin(&mut self, previous: u32) -> Option<String> {
        if previous == 0 || self.channel == previous {
            return None;
        }
        let join = chat::channel_request(previous).ok()?;
        let revision = self.state.roster_revision();
        let outcome = self
            .call(service::LEGACY_CHAT, method::JOIN_CHANNEL, &join)
            .and_then(|()| self.await_channel(previous, revision, Duration::from_secs(5)));
        match outcome {
            Ok(()) => {
                self.channel = previous;
                Some(format!("back in channel {previous}"))
            }
            Err(error) => Some(format!("could not rejoin channel {previous}: {error}")),
        }
    }

    /// joins a channel the reader named.
    ///
    /// a room the gateway has already listed — a public channel — is joined
    /// by its id, the request that lands the startup channel every time;
    /// "public chat N" is read as that id. Anything else is a custom channel:
    /// the SDK's `JoinCustomChannelByName` first and, when that does not move
    /// us, `CreateAndJoinCustomChannel` — classic Battle.net makes the room
    /// when it is not there, and the SDK splits that into its own request.
    pub fn join_named(&mut self, name: &str) -> Result<()> {
        let name = name.trim();
        if let Some(channel_id) = self
            .state
            .find_channel(name)
            .map(|channel| channel.channel_id)
        {
            return self.join(channel_id);
        }
        let mark = self.watch.seen;
        let mut notes = Vec::new();
        // classic Battle.net's /join is a server command, and the gateway
        // whitelists the classic commands it forwards; when join is among
        // them, that is the retail route and the server does the moving
        if let Some(verb) = ["join", "j", "channel"]
            .into_iter()
            .find(|verb| self.commands.allows(verb))
        {
            match self.join_by_command(verb, name)? {
                Some(channel_id) => {
                    self.channel = channel_id;
                    return Ok(());
                }
                None => notes.push(format!("/{verb} command sent, no move")),
            }
        } else {
            notes.push("no join command in the whitelist".to_owned());
        }
        // the SDK's own requests, sent the way the SDK sends them: from
        // outside any room. a session left outside every room by a join that
        // does not land goes back to the one it came from
        let previous = self.channel;
        self.leave_current_channel()?;
        notes.push(format!("left channel {previous}"));
        for method_id in [
            method::JOIN_CUSTOM_CHANNEL_BY_NAME,
            method::CREATE_AND_JOIN_CUSTOM_CHANNEL,
            method::JOIN_CUSTOM_CHANNEL,
        ] {
            let attempt = self.join_named_with(name, method_id)?;
            match attempt.moved {
                Some(channel_id) => {
                    self.channel = channel_id;
                    return Ok(());
                }
                None => notes.push(attempt.note),
            }
        }
        if let Some(note) = self.rejoin(previous) {
            notes.push(note);
        }
        let frames = self.watch.frames_since(mark);
        let frames = if frames.is_empty() {
            "none".to_owned()
        } else {
            frames.join(", ")
        };
        Err(Error::ClassicWire(format!(
            "Battle.net did not move you to {name:?} ({}; frames since the join: {frames})",
            notes.join("; ")
        )))
    }

    /// the server-side `/join`: the command the gateway whitelists, forwarded
    /// as `SendCommand`. the move comes back as ForceJoinChannel or a new
    /// roster, so the channel is watched rather than the answer.
    fn join_by_command(&mut self, verb: &str, name: &str) -> Result<Option<u32>> {
        let previous = self.channel;
        let revision = self.state.roster_revision();
        let body = chat::send_command_request(self.channel, verb, &[name])?;
        self.call(service::LEGACY_CHAT, method::SEND_COMMAND, &body)?;
        let deadline = std::time::Instant::now() + JOIN_GRACE;
        loop {
            if self.channel != previous && self.channel != 0 {
                return Ok(Some(self.channel));
            }
            if self.state.roster_revision() != revision
                && let Some(channel_id) = self
                    .state
                    .find_channel(name)
                    .map(|channel| channel.channel_id)
            {
                return Ok(Some(channel_id));
            }
            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }
            self.poll_quietly()?;
        }
    }

    fn join_named_with(&mut self, name: &str, method_id: u32) -> Result<JoinAttempt> {
        let name = name.trim();
        if self.state.channel(self.channel).is_some_and(|channel| {
            channel.name.eq_ignore_ascii_case(name)
                || channel
                    .display_name
                    .as_deref()
                    .is_some_and(|display| display.eq_ignore_ascii_case(name))
        }) {
            return Ok(JoinAttempt {
                moved: Some(self.channel),
                note: String::new(),
            });
        }

        let request = chat::named_channel_request(name)?;
        let revision = self.state.roster_revision();
        let response = self.call_body(service::LEGACY_CHAT, method_id, &request)?;
        let named = chat::parse_joined_channel_response(&response);
        if crate::trace_enabled() {
            eprintln!(
                "superiority: [S1] join {name:?} via {:#010x}: {} byte answer, room named: {}",
                method_id,
                response.len(),
                named.is_some()
            );
        }
        // the answer names the room: that is the join, roster and all. the
        // ChannelsUpdated that normally carries it is welcome when it comes
        if let Some(channel) = named {
            let channel_id = channel.channel_id;
            self.state.adopt_channel(channel);
            return Ok(JoinAttempt {
                moved: Some(channel_id),
                note: String::new(),
            });
        }
        // an answer that does not name the room may still be moving us: give
        // the roster a moment, then report — a session that waited the whole
        // startup patience here looked hung, and a failure looked like a
        // missing channel list
        let moved = self.await_named_channel(name, revision)?;
        Ok(JoinAttempt {
            moved,
            note: format!(
                "{} answered {} bytes, no move",
                crate::games::scr::catalog::method_name(service::LEGACY_CHAT, method_id)
                    .unwrap_or("join request"),
                response.len()
            ),
        })
    }

    /// the channel this session is in, or `0` for none.
    #[must_use]
    pub fn channel(&self) -> u32 {
        self.channel
    }

    #[must_use]
    pub fn battle_tag(&self) -> Option<&str> {
        self.battle_tag.as_deref()
    }

    #[must_use]
    pub const fn account_id(&self) -> u64 {
        self.account_id
    }

    #[must_use]
    pub fn connected_region(&self) -> u64 {
        self.connected_region
    }

    /// the latest account-wide status delivered by `PlayerStatusUpdated`.
    #[must_use]
    pub fn player_status(&self) -> Option<PlayerStatus> {
        self.player_status
    }

    /// enables the account-status callback. The generated service declares an
    /// empty request and response for this subscription.
    pub fn subscribe_player_status(&mut self) -> Result<()> {
        self.call(
            service::AURORA_USER,
            method::SUBSCRIBE_TO_PLAYER_STATUS,
            &[],
        )
    }

    /// updates SC:R's account-wide Online/Away/Busy value.
    pub fn set_player_status(&mut self, status: PlayerStatus) -> Result<()> {
        let request = user::update_player_status_request(status);
        self.call(service::AURORA_USER, method::UPDATE_PLAYER_STATUS, &request)?;
        self.player_status = Some(status);
        Ok(())
    }

    pub fn send_message(&mut self, text: &str) -> Result<()> {
        if self.channel == 0 {
            return Err(Error::ClassicWire(
                "cannot send a message before joining a channel".into(),
            ));
        }
        let body = crate::games::scr::chat::send_message_request(self.channel, text)?;
        self.call(service::LEGACY_CHAT, method::SEND_MESSAGE, &body)?;
        Ok(())
    }

    /// writes the session's own talk into the transcript. LegacyChat
    /// acknowledges the send and never echoes it, so this is the only copy the
    /// reader — and the Live tap — will see; a gateway echo, should one ever
    /// arrive, is absorbed against it.
    pub fn record_local_talk(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() || self.channel == 0 {
            return;
        }
        let sender = self.local_toon().to_owned();
        self.state.push_talk(self.channel, sender, text);
        self.watch.remember(text);
    }

    /// the name this account talks under: the roster member carrying our
    /// BattleTag, or the toon the gateway signed in as before the roster says.
    fn local_toon(&self) -> &str {
        self.battle_tag
            .as_deref()
            .and_then(|battle_tag| {
                self.state
                    .channel(self.channel)?
                    .users
                    .iter()
                    .find_map(|user| {
                        user.battle_tag()
                            .is_some_and(|tag| tag.eq_ignore_ascii_case(battle_tag))
                            .then_some(user.name.as_str())
                    })
            })
            .unwrap_or(&self.toon_name)
    }

    /// broadcasts one message to every classic-chat friend.
    pub fn send_message_to_all_friends(&mut self, text: &str) -> Result<()> {
        let body = chat::send_message_to_all_friends_request(text)?;
        self.call(
            service::LEGACY_CHAT,
            method::SEND_MESSAGE_TO_ALL_FRIENDS,
            &body,
        )
    }

    /// sends one private message through the server-advertised classic
    /// whisper command. The recipient and message remain separate protobuf
    /// arguments, matching the retail SDK's `SendCommandRequest` serializer.
    pub fn send_whisper(&mut self, recipient: &str, text: &str) -> Result<()> {
        if self.channel == 0 {
            return Err(Error::ClassicWire(
                "cannot send a whisper before joining a channel".into(),
            ));
        }
        let recipient = recipient.trim();
        if recipient.is_empty() {
            return Err(Error::ClassicWire(
                "whisper recipient cannot be empty".into(),
            ));
        }
        let message = chat::validated_message(text)?;
        let command = ["whisper", "w", "msg", "m"]
            .into_iter()
            .find(|command| self.commands.allows(command))
            .ok_or_else(|| {
                Error::ClassicWire("Battle.net did not advertise a whisper command".into())
            })?;
        let body = chat::send_command_request(self.channel, command, &[recipient, message])?;
        self.call(service::LEGACY_CHAT, method::SEND_COMMAND, &body)
    }

    /// sends an account-level whisper through the AuroraChat service registered
    /// by SC:R. Its echo callback is the authoritative outgoing transcript
    /// event, just as its receive callback is the inbound event.
    pub fn send_account_whisper(&mut self, account_id: u32, text: &str) -> Result<()> {
        let body = chat::send_account_whisper_request(account_id, text)?;
        self.call(service::AURORA_CHAT, method::SEND_WHISPER, &body)
    }

    /// executes a command typed into Remastered's chat composer.
    ///
    /// `/help` is client-owned in retail and is rendered from the two command
    /// lists fetched at connect time. Other advertised commands use
    /// `SendCommand`; presence commands have dedicated empty RPCs.
    pub fn execute_command(&mut self, command_line: &str) -> Result<()> {
        if self.channel == 0 {
            return Err(Error::ClassicWire(
                "cannot send a command before joining a channel".into(),
            ));
        }
        let command_line = command_line.trim();
        let command_line = command_line.strip_prefix('/').unwrap_or(command_line);
        let split = command_line.find(char::is_whitespace);
        let (command, remainder) = split.map_or((command_line, ""), |index| {
            (&command_line[..index], command_line[index..].trim())
        });
        let command = command.to_ascii_lowercase();
        if command.is_empty() {
            return Err(Error::ClassicWire("chat command cannot be empty".into()));
        }
        if command == "help" {
            if !remainder.is_empty() {
                return Err(Error::ClassicWire(
                    "command-specific /help is not implemented yet".into(),
                ));
            }
            self.state.push_information(self.commands.help_text());
            return Ok(());
        }
        match command.as_str() {
            "away" if remainder.is_empty() => {
                self.set_classic_status(method::SET_AWAY, PlayerStatus::Away, "Away")
            }
            "dnd" if remainder.is_empty() => {
                self.set_classic_status(method::SET_DND, PlayerStatus::Busy, "Do Not Disturb")
            }
            "online" if remainder.is_empty() => {
                self.set_classic_status(method::SET_ONLINE, PlayerStatus::Online, "Online")
            }
            "away" | "dnd" | "online" => Err(Error::ClassicWire(format!(
                "/{command} does not accept arguments"
            ))),
            _ if self.commands.allows(&command) => {
                let arguments = command_arguments(&command, remainder);
                let body = chat::send_command_request(self.channel, &command, &arguments)?;
                self.call(service::LEGACY_CHAT, method::SEND_COMMAND, &body)
            }
            _ => Err(Error::ClassicWire(format!(
                "Unknown Remastered command: /{command}"
            ))),
        }
    }

    /// resolves one not-yet-cached roster avatar. Calling this once per worker
    /// turn progressively fills a large roster without delaying chat traffic
    /// behind a synchronous batch of profile requests.
    pub fn resolve_next_avatar(&mut self) -> Result<bool> {
        let Some(toon) = self.state.channel(self.channel).and_then(|channel| {
            channel
                .users
                .iter()
                .map(|user| user.name.as_str())
                .find(|toon| !self.avatar_cache.contains_key(&toon.to_ascii_lowercase()))
                .map(str::to_owned)
        }) else {
            return Ok(false);
        };
        let cache_key = toon.to_ascii_lowercase();
        let body = profile::get_avatar_request(
            crate::product::Product::Remastered.fourcc(),
            DEFAULT_GATEWAY_CATALOG_ID,
            &toon,
        )?;
        let local_toon = self.local_toon().to_owned();
        let state = &mut self.state;
        let channel = &mut self.channel;
        let commands = &mut self.commands;
        let player_status = &mut self.player_status;
        let watch = &mut self.watch;
        let response = match self.client.call(
            &Request::new(service::TOON_PROFILE, method::GET_AVATAR, &body),
            self.timeout,
            &mut |frame: &crate::games::scr::rpc::Frame| {
                apply_classic_frame(
                    state,
                    channel,
                    commands,
                    player_status,
                    watch,
                    &local_toon,
                    frame,
                );
            },
        ) {
            Ok(response) => response,
            Err(error) => {
                self.avatar_cache.insert(cache_key, None);
                return Err(error);
            }
        };
        let lookup = match profile::parse_avatar_response(&response.body) {
            Ok(lookup) => lookup,
            Err(error) => {
                // a malformed profile response is a valid terminal miss for
                // this roster pass. Retrying it every worker turn would pin
                // chat behind the same bad profile indefinitely.
                self.avatar_cache.insert(cache_key, None);
                return Err(error);
            }
        };
        if lookup.program_id != crate::product::Product::Remastered.fourcc()
            || lookup.gateway != DEFAULT_GATEWAY_CATALOG_ID
            || !lookup.toon.eq_ignore_ascii_case(&toon)
        {
            self.avatar_cache.insert(cache_key, None);
            return Err(Error::ClassicWire(
                "ToonProfile.GetAvatar answered for a different profile".into(),
            ));
        }
        let avatar = lookup.avatar;
        self.avatar_cache.insert(cache_key, avatar.clone());
        Ok(self.state.set_avatar(self.channel, &toon, avatar))
    }

    /// reads one frame and folds it into the chat state. Returns whether the
    /// frame changed anything worth redrawing.
    pub fn poll(&mut self, timeout: Duration) -> Result<bool> {
        let frame = self.client.pump(timeout, &mut Ignore)?;
        let local_toon = self.local_toon().to_owned();
        let changed = apply_classic_frame(
            &mut self.state,
            &mut self.channel,
            &mut self.commands,
            &mut self.player_status,
            &mut self.watch,
            &local_toon,
            &frame,
        );
        self.apply_cached_avatars();
        Ok(changed)
    }

    #[must_use]
    pub fn state(&self) -> &ChatState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut ChatState {
        &mut self.state
    }

    pub fn set_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.client.set_timeout(timeout)
    }

    /// keeps the authenticated classic session alive while its channel is
    /// otherwise quiet. Retail declares this RPC as Empty -> Empty.
    pub fn keep_alive(&mut self) -> Result<()> {
        self.call(service::AUTHENTICATION, method::PING, &[])
    }

    pub fn close(&mut self) -> Result<()> {
        // retail sends the empty LegacyChat.Disconnect request before closing
        // the transport. It is deliberately fire-and-close here: waiting for
        // its response can stall shutdown when the peer is already gone.
        let disconnected = self
            .client
            .send(&Request::new(
                service::LEGACY_CHAT,
                method::CHAT_DISCONNECT,
                &[],
            ))
            .map(drop);
        let closed = self.client.close();
        disconnected.and(closed)
    }

    fn call(&mut self, service_id: u32, method_id: u32, body: &[u8]) -> Result<()> {
        self.call_body(service_id, method_id, body).map(drop)
    }

    fn call_body(&mut self, service_id: u32, method_id: u32, body: &[u8]) -> Result<Vec<u8>> {
        // Battle.net can put chat callbacks in front of the response this call
        // is waiting for. They still belong to the session: acknowledging them
        // with Ignore makes join-time information notices and concurrent talk
        // disappear permanently.
        let timeout = self.timeout;
        let local_toon = self.local_toon().to_owned();
        let Self {
            client,
            state,
            channel,
            commands,
            player_status,
            watch,
            ..
        } = self;
        let frame = client.call(
            &Request::new(service_id, method_id, body),
            timeout,
            &mut |frame: &crate::games::scr::rpc::Frame| {
                apply_classic_frame(
                    state,
                    channel,
                    commands,
                    player_status,
                    watch,
                    &local_toon,
                    frame,
                );
            },
        )?;
        apply_classic_frame(
            state,
            channel,
            commands,
            player_status,
            watch,
            &local_toon,
            &frame,
        );
        Ok(frame.body)
    }

    fn load_command_catalog(&mut self) -> Result<()> {
        let whitelist = self.call_body(service::LEGACY_CHAT, method::GET_COMMAND_WHITELIST, &[])?;
        let blacklist = self.call_body(service::LEGACY_CHAT, method::GET_COMMAND_BLACKLIST, &[])?;
        self.commands = CommandCatalog::from_responses(&whitelist, &blacklist);
        Ok(())
    }

    fn set_classic_status(
        &mut self,
        method_id: u32,
        player_status: PlayerStatus,
        label: &str,
    ) -> Result<()> {
        self.call(service::LEGACY_CHAT, method_id, &[])?;
        self.set_player_status(player_status)?;
        self.state
            .push_information(format!("Status set to {label}."));
        Ok(())
    }

    fn gateway_stats(&mut self) -> Result<gateway::GatewayStats> {
        let body = gateway::request(DEFAULT_GATEWAY_CATALOG_ID)?;
        let timeout = self.timeout;
        let local_toon = self.local_toon().to_owned();
        let Self {
            client,
            state,
            channel,
            commands,
            player_status,
            watch,
            ..
        } = self;
        let response = client.call(
            &Request::new(service::GATEWAY, method::GET_GATEWAY_STATS, &body),
            timeout,
            &mut |frame: &crate::games::scr::rpc::Frame| {
                apply_classic_frame(
                    state,
                    channel,
                    commands,
                    player_status,
                    watch,
                    &local_toon,
                    frame,
                );
            },
        )?;
        gateway::parse_response(&response.body)
    }

    fn apply_cached_avatars(&mut self) {
        let Some(channel) = self.state.channel(self.channel) else {
            return;
        };
        let cached = channel
            .users
            .iter()
            .filter_map(|user| {
                self.avatar_cache
                    .get(&user.name.to_ascii_lowercase())
                    .map(|avatar| (user.name.clone(), avatar.clone()))
            })
            .collect::<Vec<_>>();
        for (toon, avatar) in cached {
            self.state.set_avatar(self.channel, &toon, avatar);
        }
    }

    /// blocks until the roster revision moves past `from`, which is how the
    /// gateway signals it has finished pushing the channel list.
    fn await_roster_change(&mut self, from: u64) -> Result<()> {
        let deadline = std::time::Instant::now() + self.timeout;
        while self.state.roster_revision() == from {
            if std::time::Instant::now() >= deadline {
                return Err(Error::ClassicWire(
                    "LegacyChat came online but never sent a channel list".into(),
                ));
            }
            self.poll_quietly()?;
        }
        Ok(())
    }

    /// blocks until `channel_id`'s roster has landed since `from`. the
    /// LeftChannel for the room being left can arrive first, and a roster
    /// revision alone would mistake it for the join.
    fn await_channel(&mut self, channel_id: u32, from: u64, patience: Duration) -> Result<()> {
        let deadline = std::time::Instant::now() + patience;
        while self.state.roster_revision() == from || self.state.channel(channel_id).is_none() {
            if std::time::Instant::now() >= deadline {
                return Err(Error::ClassicWire(format!(
                    "Battle.net did not move you to channel {channel_id}"
                )));
            }
            self.poll_quietly()?;
        }
        Ok(())
    }

    /// waits [`JOIN_GRACE`] for a room called `name` to appear in a roster
    /// newer than `from`; `None` when it does not.
    fn await_named_channel(&mut self, name: &str, from: u64) -> Result<Option<u32>> {
        let deadline = std::time::Instant::now() + JOIN_GRACE;
        loop {
            if self.state.roster_revision() != from
                && let Some(channel_id) = self
                    .state
                    .find_channel(name)
                    .map(|channel| channel.channel_id)
            {
                return Ok(Some(channel_id));
            }
            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }
            self.poll_quietly()?;
        }
    }

    /// one short poll where silence is not an error — the caller keeps its
    /// own deadline.
    fn poll_quietly(&mut self) -> Result<()> {
        match self.poll(POLL_STEP.min(self.timeout)) {
            Ok(_) => Ok(()),
            Err(error) if is_quiet_read(&error) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

/// reproduces the argument grouping used by the retail command composer.
/// Whisper aliases carry a recipient plus the untouched message, user lookup
/// commands carry the whole lookup string, and any newer server-advertised
/// command falls back to one argument per word.
fn command_arguments<'a>(command: &str, remainder: &'a str) -> Vec<&'a str> {
    if remainder.is_empty() {
        return Vec::new();
    }
    match command {
        "whisper" | "w" | "msg" | "m" => remainder.find(char::is_whitespace).map_or_else(
            || vec![remainder],
            |index| {
                let message = remainder[index..].trim();
                if message.is_empty() {
                    vec![&remainder[..index]]
                } else {
                    vec![&remainder[..index], message]
                }
            },
        ),
        "squelch" | "ignore" | "unsquelch" | "unignore" | "whois" | "join" | "j" | "channel" => {
            vec![remainder]
        }
        _ => remainder.split_whitespace().collect(),
    }
}

fn apply_classic_frame(
    state: &mut ChatState,
    channel: &mut u32,
    commands: &mut CommandCatalog,
    player_status: &mut Option<PlayerStatus>,
    watch: &mut Watch,
    local_toon: &str,
    frame: &crate::games::scr::rpc::Frame,
) -> bool {
    watch.saw(frame);
    if !frame.header.is_response() && frame.header.service_id == service::LEGACY_CHAT {
        match frame.header.method_id {
            method::COMMAND_WHITELIST_UPDATE => {
                return commands.replace_whitelist(&frame.body);
            }
            method::COMMAND_BLACKLIST_UPDATE => {
                return commands.replace_blacklist(&frame.body);
            }
            _ => {}
        }
    }
    if !frame.header.is_response()
        && frame.header.service_id == service::AURORA_USER
        && frame.header.method_id == method::PLAYER_STATUS_UPDATED
    {
        let Some(status) = user::parse_player_status(&frame.body) else {
            return false;
        };
        let changed = *player_status != Some(status);
        *player_status = Some(status);
        return changed;
    }
    let forced_channel = (!frame.header.is_response()
        && frame.header.service_id == service::LEGACY_CHAT
        && frame.header.method_id == method::FORCE_JOIN_CHANNEL)
        .then(|| chat::parse_force_join_channel(&frame.body))
        .flatten()
        .map(|forced| forced.channel_id);
    let changed = state.apply(frame);
    // our own talk comes back only if the gateway echoes it, and the local
    // copy is already in the transcript
    if changed {
        state.retract_last_event_if(|event| {
            event.kind == EventKind::Talk
                && event
                    .sender
                    .as_deref()
                    .is_some_and(|sender| sender.eq_ignore_ascii_case(local_toon))
                && event.text.as_deref().is_some_and(|text| watch.absorb(text))
        });
    }
    if let Some(forced_channel) = forced_channel {
        let channel_changed = *channel != forced_channel;
        *channel = forced_channel;
        return changed || channel_changed;
    }
    changed
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Toon {
    id: u64,
    name: String,
    gateway_id: u64,
}

/// selects the toon Battle.net actually returned for the configured gateway.
///
/// `Legacy.Connect` field 1 is the toon record's field 1, not the gateway id.
/// Treating it as a realm enum happens to work when a toon has id 1 and then
/// silently stalls chat startup for accounts whose selected toon has another
/// id.
fn toon_for_gateway(response: &[u8], gateway_id: u64) -> Result<Toon> {
    let mut toons = Vec::new();
    for field in protobuf::fields(response) {
        let field = field?;
        if field.number != 1 {
            continue;
        }
        let record = field.bytes().ok_or_else(|| {
            Error::ClassicWire("GameAccount.GetToons returned a non-message toon".into())
        })?;
        let mut id = None;
        let mut name = None;
        let mut record_gateway = None;
        for field in protobuf::fields(record) {
            let field = field?;
            match field.number {
                1 => id = field.varint(),
                2 => {
                    name = field
                        .bytes()
                        .and_then(|value| std::str::from_utf8(value).ok())
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned);
                }
                3 => record_gateway = field.varint(),
                _ => {}
            }
        }
        toons.push(Toon {
            id: id.ok_or_else(|| {
                Error::ClassicWire("GameAccount.GetToons omitted a toon id".into())
            })?,
            name: name.ok_or_else(|| {
                Error::ClassicWire("GameAccount.GetToons omitted a toon name".into())
            })?,
            gateway_id: record_gateway.ok_or_else(|| {
                Error::ClassicWire("GameAccount.GetToons omitted a toon gateway".into())
            })?,
        });
    }
    toons
        .into_iter()
        .find(|toon| toon.gateway_id == gateway_id)
        .ok_or_else(|| {
            Error::ClassicWire(format!(
                "this account has no StarCraft: Remastered toon on {DEFAULT_GATEWAY_NAME} \
                 (gateway {gateway_id})"
            ))
        })
}

fn legacy_connect_request(toon_id: u64) -> Result<Vec<u8>> {
    if toon_id == 0 {
        return Err(Error::ClassicWire(
            "Legacy.Connect requires a positive toon id".into(),
        ));
    }
    Ok(Message::new().varint(1, toon_id).into_vec())
}

/// Bounds classic startup even when SecureTransport ignores the socket's read
/// timeout. The helper thread exits on the next short beat after disarming.
struct ClassicConnectWatchdog {
    waiting: Arc<AtomicBool>,
    fired: Arc<AtomicBool>,
}

impl ClassicConnectWatchdog {
    const BEAT: Duration = Duration::from_millis(250);

    fn arm(
        interrupt: crate::platform::wire::websocket::SocketInterrupt,
        patience: Duration,
    ) -> Self {
        let waiting = Arc::new(AtomicBool::new(true));
        let fired = Arc::new(AtomicBool::new(false));
        let watchdog = Self {
            waiting: Arc::clone(&waiting),
            fired: Arc::clone(&fired),
        };
        let started = thread::Builder::new()
            .name("scr-connect-watchdog".into())
            .spawn(move || {
                let mut spent = Duration::ZERO;
                while waiting.load(Ordering::Relaxed) {
                    thread::sleep(Self::BEAT);
                    spent += Self::BEAT;
                    if spent >= patience {
                        if waiting.load(Ordering::Relaxed) {
                            fired.store(true, Ordering::Relaxed);
                            interrupt.cut();
                        }
                        return;
                    }
                }
            });
        if started.is_err() {
            watchdog.waiting.store(false, Ordering::Relaxed);
        }
        watchdog
    }

    fn disarm(&self) {
        self.waiting.store(false, Ordering::Relaxed);
    }

    fn fired(&self) -> bool {
        self.fired.load(Ordering::Relaxed)
    }
}

impl Drop for ClassicConnectWatchdog {
    fn drop(&mut self) {
        self.disarm();
    }
}

/// how far a Remastered sign-in has got.
///
/// Typed rather than a log line, because the caller drives a progress bar off
/// it and matching on prose would be a way to break that silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// signing in to Battle.net through Aurora.
    SigningIn,
    /// asking the game service where the classic server is.
    AskingForServer,
    /// authenticating to the classic server with the ticket.
    Authenticating,
    /// bringing chat online.
    StartingChat,
}

/// signs in to Remastered and brings its chat up, end to end.
///
/// Both halves are Remastered's own: [`aurora`] for the account layer and the
/// classic channel for everything after it.
///
/// [`aurora`]: crate::games::scr::aurora
pub fn connect(
    credential: &crate::platform::bgs::SecretBytes,
    challenge: &mut impl ChallengeHandler,
    timeout: Duration,
    mut validate_account: impl FnMut(u64, Option<&str>) -> Result<()>,
    mut progress: impl FnMut(&str),
    mut step: impl FnMut(Step),
) -> Result<ClassicSession> {
    use crate::games::scr::aurora::AuroraClient;

    step(Step::SigningIn);
    let mut aurora = AuroraClient::connect_default(timeout)?;
    progress("Connected to Aurora; logging on…");
    // the same call signs in and asks where the server is; the second half is
    // where the wait actually is, so the step moves before it
    let (handoff, session) = aurora.bootstrap(credential, challenge, &mut validate_account)?;
    step(Step::AskingForServer);
    progress(&format!(
        "Aurora named {}:{} ({} byte ticket)\n\
         [S1]   url:   {}\n\
         [S1]   path:  {}\n\
         [S1]   shape: {}",
        handoff.host,
        handoff.port,
        handoff.ticket.expose().len(),
        handoff.url,
        handoff.path,
        handoff.shape,
    ));

    if crate::games::scr::probe::run_is_wanted() {
        progress(&crate::games::scr::probe::run(&handoff, timeout));
    }
    step(Step::Authenticating);
    let classic = ClassicSession::establish(&handoff, &session, timeout)?;
    // aurora has done its job; the classic channel carries the rest
    let _ = aurora.close();
    progress("Classic channel is up.");
    step(Step::StartingChat);
    Ok(classic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        games::scr::rpc::{Frame, Header},
        platform::wire::raw as protobuf,
    };

    #[test]
    fn get_toons_selects_the_returned_us_east_toon() {
        // captured from the account that exposed the hard-coded selector bug:
        // toon 1, `ncarrillo1`, gateway catalogue id 11 (U.S. East).
        let response = hex::decode("0a100801120a6e63617272696c6c6f31180b").expect("capture");
        assert_eq!(
            toon_for_gateway(&response, DEFAULT_GATEWAY_CATALOG_ID).expect("U.S. East toon"),
            Toon {
                id: 1,
                name: "ncarrillo1".into(),
                gateway_id: 11,
            }
        );
    }

    #[test]
    fn legacy_connect_uses_the_returned_toon_id_not_the_gateway_id() {
        let request = legacy_connect_request(7).expect("toon id");
        assert_eq!(protobuf::first_varint(&request, 1), Some(7));
        assert_eq!(DEFAULT_GATEWAY_CATALOG_ID, 11);
        assert_ne!(protobuf::first_varint(&request, 1), Some(11));
        assert_eq!(DEFAULT_GATEWAY_NAME, "U.S. East");
    }

    #[test]
    fn a_toon_on_another_gateway_is_not_silently_substituted() {
        let korea = Message::new()
            .varint(1, 2)
            .bytes(2, b"KoreanToon")
            .varint(3, 30)
            .into_vec();
        let response = Message::new().bytes(1, &korea).into_vec();
        let error = toon_for_gateway(&response, DEFAULT_GATEWAY_CATALOG_ID)
            .expect_err("there is no U.S. East toon");
        assert!(error.to_string().contains("no StarCraft: Remastered toon"));
        assert!(legacy_connect_request(0).is_err());
    }

    #[test]
    fn command_arguments_keep_whisper_messages_and_lookup_names_together() {
        assert_eq!(
            command_arguments("whisper", "somebody hello there"),
            ["somebody", "hello there"]
        );
        assert_eq!(command_arguments("w", "somebody"), ["somebody"]);
        assert_eq!(
            command_arguments("whois", "a name with spaces"),
            ["a name with spaces"]
        );
        assert_eq!(command_arguments("future", "one two"), ["one", "two"]);
        assert_eq!(command_arguments("join", "Op BnetCC"), ["Op BnetCC"]);
    }

    #[test]
    fn a_force_join_callback_moves_the_sessions_active_channel() {
        let channel_info = Message::new()
            .varint(1, 77)
            .bytes(2, b"Op Superiority")
            .into_vec();
        let frame = Frame {
            header: Header {
                service_id: service::LEGACY_CHAT,
                method_id: method::FORCE_JOIN_CHANNEL,
                token: 1,
                is_response: Some(false),
                ..Header::default()
            },
            body: Message::new().bytes(2, &channel_info).into_vec(),
        };
        let mut state = ChatState::new();
        let mut channel = 9;
        let mut commands = CommandCatalog::default();
        let mut status = None;

        assert!(apply_classic_frame(
            &mut state,
            &mut channel,
            &mut commands,
            &mut status,
            &mut Watch::default(),
            "",
            &frame,
        ));
        assert_eq!(channel, 77);
        assert_eq!(
            state.channel(77).map(|channel| channel.name.as_str()),
            Some("Op Superiority")
        );
    }

    fn talk_frame(channel: u64, sender: &str, text: &str) -> Frame {
        Frame {
            header: Header {
                service_id: service::LEGACY_CHAT,
                method_id: method::CHAT_TALK_MESSAGE,
                token: 2,
                is_response: Some(false),
                ..Header::default()
            },
            body: Message::new()
                .varint(1, channel)
                .bytes(2, sender.as_bytes())
                .bytes(3, text.as_bytes())
                .into_vec(),
        }
    }

    #[test]
    fn our_own_talk_is_written_once_even_if_the_gateway_echoes_it() {
        // LegacyChat never echoes the sender's line, so the session writes it;
        // should an echo ever come, it is the same line and must not show twice
        let mut state = ChatState::new();
        let mut watch = Watch::default();
        state.push_talk(9, "Me".to_owned(), "hello there");
        watch.remember("hello there");
        let mut channel = 9;
        let mut commands = CommandCatalog::default();
        let mut status = None;

        apply_classic_frame(
            &mut state,
            &mut channel,
            &mut commands,
            &mut status,
            &mut watch,
            "me",
            &talk_frame(9, "Me", "hello there"),
        );
        let events = state.take_events();
        assert_eq!(events.len(), 1, "the echo was absorbed: {events:?}");
        assert_eq!(events[0].kind, EventKind::Talk);
        assert_eq!(events[0].sender.as_deref(), Some("Me"));
        assert_eq!(events[0].text.as_deref(), Some("hello there"));

        // a second, different line from us, and someone else saying the same
        // words, are both real
        apply_classic_frame(
            &mut state,
            &mut channel,
            &mut commands,
            &mut status,
            &mut watch,
            "me",
            &talk_frame(9, "Me", "hello there"),
        );
        apply_classic_frame(
            &mut state,
            &mut channel,
            &mut commands,
            &mut status,
            &mut watch,
            "me",
            &talk_frame(9, "Somebody", "hello there"),
        );
        assert_eq!(state.take_events().len(), 2);
    }
}
