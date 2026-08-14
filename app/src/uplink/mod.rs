//! Live sharing: a tap on the network thread projects selected chat events
//! into wire DTOs and hands them to a dedicated `sc2-uplink` thread, which
//! batches and POSTs them to the Cloudflare backend. The module is named
//! `uplink` because `LiveChat` already means "the connected Battle.net
//! session"; user-facing copy calls the feature Live.
//!
//! Invariants:
//! - the tap never blocks and never errors into the chat loop — a full
//!   channel counts a drop and moves on, and sequence numbers are assigned
//!   before the send so losses leave visible gaps server-side;
//! - nothing is sent while the master switch is off, and the first passing
//!   event lazily announces the session, so a disabled uplink is silent;
//! - a 401/403 latches the uplink off until restart (or a new link).

pub mod config;
pub mod http;
pub mod model;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, RwLock,
        atomic::Ordering,
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;

use crate::chat::{ChatChannel, ChatEvent};
use config::{UplinkConfig, UplinkStats};
use http::{LiveHttp, PostError, validate_endpoint};
use model::{
    Backoff, Batcher, Envelope, EventDto, EventKind, Projector, SessionMeta, channel_identity,
};

const CHANNEL_CAPACITY: usize = 1024;
const IDLE_POLL: Duration = Duration::from_millis(500);
const SHUTDOWN_FLUSH_DEADLINE: Duration = Duration::from_secs(5);
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Shared handles between the UI (config writer, stats reader) and the
/// uplink machinery. Cheap to clone.
#[derive(Clone, Default)]
pub struct UplinkControl {
    pub config: Arc<RwLock<UplinkConfig>>,
    pub stats: Arc<UplinkStats>,
}

impl UplinkControl {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Main-thread convenience: mutate the config under the lock.
    pub fn update_config(&self, apply: impl FnOnce(&mut UplinkConfig)) {
        if let Ok(mut config) = self.config.write() {
            apply(&mut config);
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> UplinkConfig {
        self.config
            .read()
            .map(|config| config.clone())
            .unwrap_or_default()
    }
}

enum TapMessage {
    /// A new Battle.net connection began; subsequent events belong to it.
    Session(SessionMeta),
    Event(EventDto),
}

/// The network thread's handle: mint one [`SessionTap`] per connection.
#[derive(Clone)]
pub struct Publisher {
    sender: SyncSender<TapMessage>,
    control: UplinkControl,
    club_names: Arc<RwLock<BTreeMap<u32, String>>>,
}

impl Publisher {
    #[must_use]
    pub fn begin_session(&self, channels: &[ChatChannel]) -> SessionTap {
        let expected_channels: BTreeSet<String> = channels.iter().map(channel_identity).collect();
        let club_names = self.club_names.read().map_or_else(
            |_| BTreeMap::new(),
            |names| {
                channels
                    .iter()
                    .filter_map(|channel| {
                        let ChatChannel::Club(club_id) = channel else {
                            return None;
                        };
                        names.get(club_id).cloned().map(|name| (*club_id, name))
                    })
                    .collect()
            },
        );
        let mut tap = SessionTap {
            sender: self.sender.clone(),
            control: self.control.clone(),
            meta: SessionMeta {
                id: format!("{:032x}", rand::random::<u128>()),
                client_version: CLIENT_VERSION,
                started_at: now_ms(),
            },
            projector: Projector::with_club_names(club_names),
            club_names: self.club_names.clone(),
            announced: false,
            next_seq: 1,
            pending_dropped: 0,
            pending_channels: expected_channels.clone(),
            expected_channels,
            resolved_channels: BTreeSet::new(),
            initial_sync_complete: false,
            ended: false,
        };
        if matches!(tap.control.config.read(), Ok(config) if config.enabled) {
            tap.announce();
        }
        tap
    }
}

/// Lives on the network thread for the duration of one connection.
pub struct SessionTap {
    sender: SyncSender<TapMessage>,
    control: UplinkControl,
    meta: SessionMeta,
    projector: Projector,
    club_names: Arc<RwLock<BTreeMap<u32, String>>>,
    announced: bool,
    next_seq: u64,
    pending_dropped: u64,
    expected_channels: BTreeSet<String>,
    pending_channels: BTreeSet<String>,
    resolved_channels: BTreeSet<String>,
    initial_sync_complete: bool,
    ended: bool,
}

impl SessionTap {
    /// Observes one chat event. Non-blocking and infallible by design.
    pub fn observe(&mut self, event: &ChatEvent) {
        if let ChatEvent::GroupSummary {
            club_id,
            name: Some(name),
            ..
        } = event
            && let Ok(mut names) = self.club_names.write()
        {
            names.insert(*club_id, name.clone());
        }
        let enabled = match self.control.config.read() {
            Ok(config) => config.enabled,
            Err(_) => return,
        };
        // The global switch shares every open channel; per-channel selection
        // (`config.shared_channels`) is plumbed but deliberately has no UI
        // yet, so no allow-list is passed.
        let gates = model::ProjectionGates {
            enabled,
            shared_channels: None,
        };
        let Some(kind) = self.projector.project(event, gates) else {
            return;
        };
        if let EventKind::Roster {
            channel,
            complete: true,
            ..
        } = &kind
        {
            self.pending_channels.remove(&channel.key);
            self.resolved_channels.insert(channel.key.clone());
            if self.initial_sync_complete
                || self.expected_channels.is_subset(&self.resolved_channels)
            {
                self.reconcile();
                self.initial_sync_complete = true;
            }
        } else {
            self.announce_and_emit(kind);
        }
    }

    /// Called on the connection keepalive tick so the feed stays live through
    /// idle stretches. No-op while sharing is off or the session hasn't begun
    /// (nothing to keep alive yet).
    pub fn heartbeat(&mut self) {
        let enabled = matches!(self.control.config.read(), Ok(config) if config.enabled);
        if !enabled {
            self.end_session();
        } else if self.ended {
            self.ended = false;
            self.emit(EventKind::SessionStarted);
            self.reconcile();
        } else if self.announced {
            self.emit(EventKind::Heartbeat);
        }
    }

    /// The local user left a channel. Leaving is a command rather than a chat
    /// event, so the command loop reports it here directly.
    pub fn observe_left(&mut self, channel_index: u8) {
        let enabled = match self.control.config.read() {
            Ok(config) => config.enabled,
            Err(_) => return,
        };
        let Some(channel) = self.projector.leave(channel_index, enabled, None) else {
            return;
        };
        self.announce_and_emit(EventKind::Left { channel });
    }

    pub fn reject_channel(&mut self, channel: &ChatChannel) {
        let identity = channel_identity(channel);
        self.pending_channels.remove(&identity);
        self.resolved_channels.insert(identity);
        if !self.initial_sync_complete && self.expected_channels.is_subset(&self.resolved_channels)
        {
            self.reconcile();
            self.initial_sync_complete = true;
        }
    }

    pub fn end_session(&mut self) {
        if self.announced && !self.ended {
            self.emit(EventKind::SessionEnded);
            self.ended = true;
        }
    }

    pub fn resolve_pending_channels(&mut self) {
        if self.initial_sync_complete || self.pending_channels.is_empty() {
            return;
        }
        self.resolved_channels.append(&mut self.pending_channels);
        self.reconcile();
        self.initial_sync_complete = true;
    }

    fn announce_and_emit(&mut self, kind: EventKind) {
        if self.announce() {
            self.emit(kind);
        }
    }

    fn announce(&mut self) -> bool {
        if self.announced {
            return true;
        }
        if self
            .sender
            .try_send(TapMessage::Session(self.meta.clone()))
            .is_err()
        {
            self.control.stats.note_dropped(1);
            return false;
        }
        self.announced = true;
        self.emit(EventKind::SessionStarted);
        true
    }

    fn reconcile(&mut self) {
        let events = self.projector.reconciliation_events(None);
        if events.is_empty() {
            return;
        }
        self.announce_and_emit(EventKind::SyncStarted);
        for event in events {
            self.emit(event);
        }
        self.emit(EventKind::SessionSynced);
    }

    fn emit(&mut self, kind: EventKind) {
        if self.pending_dropped > 0 {
            let marker = EventDto {
                seq: self.next_seq,
                ts: now_ms(),
                kind: EventKind::Dropped {
                    count: self.pending_dropped,
                },
            };
            self.next_seq += 1;
            match self.sender.try_send(TapMessage::Event(marker)) {
                Ok(()) => self.pending_dropped = 0,
                Err(TrySendError::Full(_)) => {
                    self.pending_dropped += 1;
                    self.control.stats.note_dropped(1);
                }
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
        let dto = EventDto {
            seq: self.next_seq,
            ts: now_ms(),
            kind,
        };
        self.next_seq += 1;
        if let Err(TrySendError::Full(_)) = self.sender.try_send(TapMessage::Event(dto)) {
            self.pending_dropped += 1;
            self.control.stats.note_dropped(1);
        }
    }
}

impl Drop for SessionTap {
    fn drop(&mut self) {
        self.end_session();
    }
}

// The core connects through these; the inherent methods above stay the uplink's
// own API. `self.method(..)` resolves to the inherent, not the trait, so the
// delegation is a call, not a loop.
impl crate::observer::SessionObserverFactory for Publisher {
    fn begin_session(&self, channels: &[ChatChannel]) -> Box<dyn crate::observer::SessionObserver> {
        Box::new(self.begin_session(channels))
    }
}

impl crate::observer::SessionObserver for SessionTap {
    fn observe(&mut self, event: &ChatEvent) {
        self.observe(event);
    }

    fn observe_left(&mut self, channel_index: u8) {
        self.observe_left(channel_index);
    }

    fn heartbeat(&mut self) {
        self.heartbeat();
    }

    fn reconcile(&mut self, snapshots: &[ChatEvent]) {
        let enabled = matches!(self.control.config.read(), Ok(config) if config.enabled);
        if !enabled || snapshots.is_empty() {
            return;
        }
        for snapshot in snapshots {
            let _ = self.projector.project(
                snapshot,
                model::ProjectionGates {
                    enabled: true,
                    shared_channels: None,
                },
            );
        }
        self.reconcile();
    }

    fn resolve_pending_channels(&mut self) {
        self.resolve_pending_channels();
    }

    fn reject_channel(&mut self, channel: &ChatChannel) {
        self.reject_channel(channel);
    }

    fn end_session(&mut self) {
        self.end_session();
    }
}

/// Starts the `sc2-uplink` worker thread and returns the publisher the
/// network thread will feed. Call once, at app startup.
#[must_use]
pub fn spawn(control: UplinkControl, club_names: BTreeMap<u32, String>) -> Publisher {
    let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
    let worker_control = control.clone();
    thread::Builder::new()
        .name("sc2-uplink".into())
        .spawn(move || run_worker(&receiver, &worker_control))
        .expect("uplink thread must start");
    Publisher {
        sender,
        control,
        club_names: Arc::new(RwLock::new(club_names)),
    }
}

enum FlushOutcome {
    NothingDue,
    Sent,
    Deferred,
    Dropped,
}

fn run_worker(receiver: &Receiver<TapMessage>, control: &UplinkControl) {
    let http = LiveHttp::new();
    let mut batcher = Batcher::default();
    let mut backoff = Backoff::default();
    let mut next_attempt = Instant::now();
    let mut registration_backoff = Backoff::default();
    let mut next_registration = Instant::now();
    let mut session: Option<SessionMeta> = None;

    loop {
        match receiver.recv_timeout(wait_duration(&batcher, next_attempt)) {
            Ok(TapMessage::Session(meta)) => {
                // Never mix two connections in one envelope: push the old
                // session's remainder out (or drop it) before switching.
                if !batcher.is_empty() {
                    let _ = flush(
                        &http,
                        control,
                        &mut batcher,
                        &mut backoff,
                        &mut next_attempt,
                        session.as_ref(),
                        true,
                    );
                    drain_as_dropped(&mut batcher, control);
                }
                session = Some(meta);
            }
            Ok(TapMessage::Event(dto)) => {
                let dropped = batcher.push(dto, Instant::now());
                if dropped > 0 {
                    control.stats.note_dropped(dropped);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                shutdown_flush(control, &mut batcher, session.as_ref());
                return;
            }
        }

        maybe_register(
            &http,
            control,
            &mut registration_backoff,
            &mut next_registration,
        );
        let _ = flush(
            &http,
            control,
            &mut batcher,
            &mut backoff,
            &mut next_attempt,
            session.as_ref(),
            false,
        );
    }
}

fn wait_duration(batcher: &Batcher, next_attempt: Instant) -> Duration {
    let Some(deadline) = batcher.flush_deadline() else {
        return IDLE_POLL;
    };
    deadline
        .max(next_attempt)
        .saturating_duration_since(Instant::now())
        .clamp(Duration::from_millis(50), Duration::from_secs(1))
}

fn flush(
    http: &LiveHttp,
    control: &UplinkControl,
    batcher: &mut Batcher,
    backoff: &mut Backoff,
    next_attempt: &mut Instant,
    session: Option<&SessionMeta>,
    force: bool,
) -> FlushOutcome {
    let now = Instant::now();
    if !force && now < *next_attempt {
        return FlushOutcome::NothingDue;
    }
    let batch = if force {
        let batch = batcher.take_now();
        if batch.is_empty() {
            return FlushOutcome::NothingDue;
        }
        batch
    } else {
        match batcher.take_batch(now) {
            Some(batch) => batch,
            None => return FlushOutcome::NothingDue,
        }
    };

    let config = control.snapshot();
    let Some(session) = session else {
        note_batch_dropped(control, &batch);
        return FlushOutcome::Dropped;
    };
    // Turned off mid-flight, or latched out: the data stops here.
    if !config.enabled || control.stats.auth_failed.load(Ordering::Relaxed) {
        note_batch_dropped(control, &batch);
        return FlushOutcome::Dropped;
    }
    let Some(token) = config.effective_token() else {
        // Registration hasn't finished; keep the batch buffered.
        batcher.restore(batch);
        *next_attempt = now + Duration::from_secs(1);
        return FlushOutcome::Deferred;
    };
    let base = config.endpoint_base();
    if let Err(error) = validate_endpoint(&base) {
        control.stats.set_last_error(Some(error));
        note_batch_dropped(control, &batch);
        return FlushOutcome::Dropped;
    }
    let envelope = Envelope {
        v: 1,
        session,
        events: &batch,
    };
    let Ok(body) = serde_json::to_vec(&envelope) else {
        note_batch_dropped(control, &batch);
        return FlushOutcome::Dropped;
    };

    match http.post_json(&format!("{base}/v1/events"), Some(token.as_str()), &body) {
        Ok(_) => {
            control
                .stats
                .note_sent(u64::try_from(batch.len()).unwrap_or(u64::MAX));
            control.stats.set_last_error(None);
            backoff.reset();
            *next_attempt = now;
            FlushOutcome::Sent
        }
        Err(PostError::Retryable(message)) => {
            batcher.restore(batch);
            backoff.note_failure();
            *next_attempt = now + jittered(backoff.delay());
            control.stats.set_last_error(Some(message));
            FlushOutcome::Deferred
        }
        Err(PostError::Rejected(message) | PostError::Fatal(message)) => {
            control.stats.set_last_error(Some(message));
            note_batch_dropped(control, &batch);
            FlushOutcome::Dropped
        }
        Err(PostError::Unauthorized) => {
            control.stats.auth_failed.store(true, Ordering::Relaxed);
            control.stats.set_last_error(Some(
                "authentication failed; Live is off until restart".into(),
            ));
            note_batch_dropped(control, &batch);
            FlushOutcome::Dropped
        }
    }
}

fn note_batch_dropped(control: &UplinkControl, batch: &[EventDto]) {
    control
        .stats
        .note_dropped(u64::try_from(batch.len()).unwrap_or(u64::MAX));
}

fn drain_as_dropped(batcher: &mut Batcher, control: &UplinkControl) {
    loop {
        let remainder = batcher.discard_batch();
        if remainder.is_empty() {
            break;
        }
        note_batch_dropped(control, &remainder);
    }
}

fn shutdown_flush(control: &UplinkControl, batcher: &mut Batcher, session: Option<&SessionMeta>) {
    let deadline = Instant::now() + SHUTDOWN_FLUSH_DEADLINE;
    let http = LiveHttp::brief();
    let mut backoff = Backoff::default();
    let mut next_attempt = Instant::now();
    while !batcher.is_empty() && Instant::now() < deadline {
        match flush(
            &http,
            control,
            batcher,
            &mut backoff,
            &mut next_attempt,
            session,
            true,
        ) {
            FlushOutcome::Sent => {}
            // At shutdown there is no later; whatever could not go, goes down.
            FlushOutcome::NothingDue | FlushOutcome::Deferred | FlushOutcome::Dropped => break,
        }
    }
    drain_as_dropped(batcher, control);
}

#[derive(Deserialize)]
struct RegisterResponse {
    feed: RegisteredFeed,
}

#[derive(Deserialize)]
struct RegisteredFeed {
    id: String,
    token: String,
    url: String,
}

/// One registration attempt against the backend. Pure network; persistence
/// happens in the caller.
fn register_once(http: &LiveHttp, base: &str) -> Result<RegisteredFeed, String> {
    validate_endpoint(base)?;
    let body = format!("{{\"client_version\":\"{CLIENT_VERSION}\"}}");
    let response = http
        .post_json(&format!("{base}/v1/feeds"), None, body.as_bytes())
        .map_err(|error| match error {
            PostError::Retryable(message)
            | PostError::Rejected(message)
            | PostError::Fatal(message) => message,
            PostError::Unauthorized => "registration refused".into(),
        })?;
    let parsed: RegisterResponse = serde_json::from_slice(&response.body)
        .map_err(|error| format!("bad registration response: {error}"))?;
    Ok(parsed.feed)
}

fn maybe_register(
    http: &LiveHttp,
    control: &UplinkControl,
    backoff: &mut Backoff,
    next_attempt: &mut Instant,
) {
    let config = control.snapshot();
    if !config.enabled {
        return;
    }
    if config.effective_token().is_some() {
        // Already registered: make sure the UI can show the link.
        if control.stats.feed_url().is_none() {
            if let Some(feed_id) = &config.feed_id {
                control
                    .stats
                    .set_feed_url(Some(format!("{}/{feed_id}", config.endpoint_base())));
            }
        }
        return;
    }
    if Instant::now() < *next_attempt {
        return;
    }

    match register_once(http, &config.endpoint_base()) {
        Ok(feed) => {
            if let Err(error) = config::store_identity(&config::StoredIdentity {
                token: feed.token.clone(),
                feed_id: feed.id.clone(),
                url: feed.url.clone(),
            }) {
                control
                    .stats
                    .set_last_error(Some(format!("could not save the feed identity: {error}")));
            }
            control.update_config(|config| {
                config.token = Some(zeroize::Zeroizing::new(feed.token.clone()));
                config.feed_id = Some(feed.id.clone());
            });
            control.stats.set_feed_url(Some(feed.url));
            control.stats.set_last_error(None);
            backoff.reset();
        }
        Err(message) => {
            backoff.note_failure();
            *next_attempt = Instant::now() + jittered(backoff.delay()).max(Duration::from_secs(5));
            control.stats.set_last_error(Some(message));
        }
    }
}

fn jittered(delay: Duration) -> Duration {
    delay.mul_f64(0.5 + 0.5 * rand::random::<f64>())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{ChatChannel, ChatUser};
    use crate::native::presence::PresenceState;

    fn test_publisher(capacity: usize) -> (Publisher, Receiver<TapMessage>, UplinkControl) {
        let control = UplinkControl::new();
        let (sender, receiver) = mpsc::sync_channel(capacity);
        (
            Publisher {
                sender,
                control: control.clone(),
                club_names: Arc::new(RwLock::new(BTreeMap::new())),
            },
            receiver,
            control,
        )
    }

    fn enable_sharing(control: &UplinkControl, channels: &[&str]) {
        control.update_config(|config| {
            config.enabled = true;
            config.shared_channels = channels.iter().map(|key| (*key).to_owned()).collect();
        });
    }

    fn message(index: u8, body: &str) -> ChatEvent {
        ChatEvent::Message {
            channel_index: index,
            sender: ChatUser {
                handle: 7,
                presence_id: None,
                name: Some("Overmind".into()),
                clan_tag: None,
                avatar: None,
                presence: PresenceState::Available,
            },
            body: body.into(),
        }
    }

    fn joined(index: u8) -> ChatEvent {
        ChatEvent::Joined {
            channel_index: index,
            channel: ChatChannel::Public(1033),
            local_member_handle: 1,
            shard_index: None,
        }
    }

    #[test]
    fn tap_announces_at_connection_start_and_assigns_sequences() {
        let (publisher, receiver, control) = test_publisher(16);
        enable_sharing(&control, &["public:1033"]);
        let mut tap = publisher.begin_session(&[ChatChannel::Public(1033)]);

        tap.observe(&joined(0));
        tap.observe(&message(0, "hello"));

        let TapMessage::Session(meta) = receiver.try_recv().expect("session") else {
            panic!("expected the session announcement first");
        };
        assert_eq!(meta.id.len(), 32);
        let seqs: Vec<u64> = std::iter::from_fn(|| receiver.try_recv().ok())
            .map(|message| match message {
                TapMessage::Event(dto) => dto.seq,
                TapMessage::Session(_) => panic!("only one session"),
            })
            .collect();
        // session_started, joined, message
        assert_eq!(seqs, vec![1, 2, 3]);
    }

    #[test]
    fn disabled_tap_is_completely_silent() {
        let (publisher, receiver, _control) = test_publisher(16);
        let mut tap = publisher.begin_session(&[ChatChannel::Public(1033)]);
        tap.observe(&joined(0));
        tap.observe(&message(0, "hello"));
        tap.heartbeat();
        drop(tap);
        assert!(
            receiver.try_recv().is_err(),
            "nothing may be sent while disabled"
        );
    }

    #[test]
    fn heartbeat_keeps_an_idle_session_alive() {
        let (publisher, receiver, control) = test_publisher(16);
        enable_sharing(&control, &["public:1033"]);
        let mut tap = publisher.begin_session(&[ChatChannel::Public(1033)]);

        // an enabled session is announced immediately, so idle connections
        // still have accurate liveness.
        tap.heartbeat();
        let messages = std::iter::from_fn(|| receiver.try_recv().ok()).collect::<Vec<_>>();
        assert!(matches!(messages.first(), Some(TapMessage::Session(_))));
        assert!(messages.iter().any(|message| matches!(
            message,
            TapMessage::Event(EventDto {
                kind: EventKind::Heartbeat,
                ..
            })
        )));
    }

    #[test]
    fn full_channel_burns_sequences_and_reports_drops() {
        let (publisher, receiver, control) = test_publisher(2);
        enable_sharing(&control, &["public:1033"]);
        let mut tap = publisher.begin_session(&[ChatChannel::Public(1033)]);

        tap.observe(&joined(0)); // Session + session_started fill capacity 2
        tap.observe(&message(0, "lost")); // joined(seq2) dropped... then message(seq3) dropped
        assert!(control.stats.dropped.load(Ordering::Relaxed) > 0);

        // Drain, then the next event must be preceded by a Dropped marker
        // and the sequence numbers must show the gap.
        while receiver.try_recv().is_ok() {}
        tap.observe(&message(0, "arrives"));
        let mut got_dropped_marker = false;
        let mut last_seq = 0;
        while let Ok(TapMessage::Event(dto)) = receiver.try_recv() {
            if matches!(dto.kind, EventKind::Dropped { .. }) {
                got_dropped_marker = true;
            }
            last_seq = dto.seq;
        }
        assert!(got_dropped_marker);
        assert!(last_seq > 3, "sequences must reflect the losses");
    }

    #[test]
    fn registration_parses_and_reports_the_feed() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer);
            let body = "{\"feed\":{\"id\":\"slugslugslugs\",\"token\":\"aa\",\"url\":\"http://x/f/slugslugslugs\"}}";
            let response = format!(
                "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len(),
            );
            stream.write_all(response.as_bytes()).expect("write");
        });

        let feed = register_once(&LiveHttp::new(), &format!("http://127.0.0.1:{port}"))
            .expect("registration");
        assert_eq!(feed.id, "slugslugslugs");
        assert_eq!(feed.token, "aa");
        server.join().expect("server");
    }
}
