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

mod classic;
pub mod config;
pub mod http;
pub mod model;
mod scr;
mod wc3;

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
const CLIENT_VERSION: &str = env!("SUPERIORITY_EFFECTIVE_VERSION");

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
    /// One event, tagged with the session it belongs to. Several products'
    /// sessions share this channel now, so an untagged event could not be put
    /// in the right envelope.
    Event { session: String, dto: EventDto },
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
                product: "sc2",
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
        announce_session(
            &self.sender,
            &self.control,
            &self.meta,
            &mut self.announced,
            &mut self.next_seq,
            &mut self.pending_dropped,
        )
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
        emit_event(
            &self.sender,
            &self.control,
            &self.meta,
            &mut self.next_seq,
            &mut self.pending_dropped,
            kind,
        );
    }
}

/// Sends the one-time session announcement, returning whether the tap may go on
/// to emit. Shared by the `StarCraft II` tap and the classic one so the
/// announce-once rule has a single home.
fn announce_session(
    sender: &SyncSender<TapMessage>,
    control: &UplinkControl,
    meta: &SessionMeta,
    announced: &mut bool,
    next_seq: &mut u64,
    pending_dropped: &mut u64,
) -> bool {
    if *announced {
        return true;
    }
    if sender.try_send(TapMessage::Session(meta.clone())).is_err() {
        control.stats.note_dropped(1);
        return false;
    }
    *announced = true;
    emit_event(
        sender,
        control,
        meta,
        next_seq,
        pending_dropped,
        EventKind::SessionStarted,
    );
    true
}

/// Emits one event onto the channel, tagged with its session, assigning the
/// sequence number and preceding it with a `Dropped` marker if the last send
/// could not fit. Shared by every tap so sequence gaps and drop accounting stay
/// identical across products.
fn emit_event(
    sender: &SyncSender<TapMessage>,
    control: &UplinkControl,
    meta: &SessionMeta,
    next_seq: &mut u64,
    pending_dropped: &mut u64,
    kind: EventKind,
) {
    if *pending_dropped > 0 {
        let marker = EventDto {
            seq: *next_seq,
            ts: now_ms(),
            kind: EventKind::Dropped {
                count: *pending_dropped,
            },
        };
        *next_seq += 1;
        match sender.try_send(TapMessage::Event {
            session: meta.id.clone(),
            dto: marker,
        }) {
            Ok(()) => *pending_dropped = 0,
            Err(TrySendError::Full(_)) => {
                *pending_dropped += 1;
                control.stats.note_dropped(1);
            }
            Err(TrySendError::Disconnected(_)) => return,
        }
    }
    let dto = EventDto {
        seq: *next_seq,
        ts: now_ms(),
        kind,
    };
    *next_seq += 1;
    if let Err(TrySendError::Full(_)) = sender.try_send(TapMessage::Event {
        session: meta.id.clone(),
        dto,
    }) {
        *pending_dropped += 1;
        control.stats.note_dropped(1);
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
impl superiority_core::observer::SessionObserverFactory for Publisher {
    fn begin_session(
        &self,
        channels: &[ChatChannel],
    ) -> Box<dyn superiority_core::observer::SessionObserver> {
        Box::new(self.begin_session(channels))
    }

    fn begin_classic_session(
        &self,
        product: superiority_core::product::Product,
        local_identity: Option<String>,
    ) -> Box<dyn superiority_core::observer::SessionObserver> {
        Box::new(classic::ClassicSessionTap::new(
            self.sender.clone(),
            self.control.clone(),
            product,
            local_identity,
        ))
    }
}

impl superiority_core::observer::SessionObserver for SessionTap {
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

/// One product's session buffered on the uplink thread. A feed can carry
/// several at once — one per game — so each keeps its own envelope and its own
/// batch; they never share a POST. `ended` marks a session that sent
/// `SessionEnded`, so it can be pruned once its last batch is away.
struct SessionSink {
    meta: SessionMeta,
    batcher: Batcher,
    ended: bool,
}

fn run_worker(receiver: &Receiver<TapMessage>, control: &UplinkControl) {
    let http = LiveHttp::new();
    let mut sessions: BTreeMap<String, SessionSink> = BTreeMap::new();
    let mut backoff = Backoff::default();
    let mut next_attempt = Instant::now();
    let mut registration_backoff = Backoff::default();
    let mut next_registration = Instant::now();

    loop {
        match receiver.recv_timeout(wait_duration(&sessions, next_attempt)) {
            Ok(TapMessage::Session(meta)) => {
                // Each connection gets its own sink; a second product announcing
                // no longer forces the first's batch out. Re-announcing keeps
                // the sink and refreshes the meta.
                sessions
                    .entry(meta.id.clone())
                    .and_modify(|sink| sink.meta = meta.clone())
                    .or_insert_with(|| SessionSink {
                        meta,
                        batcher: Batcher::default(),
                        ended: false,
                    });
            }
            Ok(TapMessage::Event { session, dto }) => {
                // An event always follows its session's announcement; one for an
                // unknown (or already-pruned) session has nowhere to go.
                if let Some(sink) = sessions.get_mut(&session) {
                    if matches!(dto.kind, EventKind::SessionEnded) {
                        sink.ended = true;
                    }
                    let dropped = sink.batcher.push(dto, Instant::now());
                    if dropped > 0 {
                        control.stats.note_dropped(dropped);
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                shutdown_flush_all(control, &mut sessions);
                return;
            }
        }

        maybe_register(
            &http,
            control,
            &mut registration_backoff,
            &mut next_registration,
        );
        flush_all(
            &http,
            control,
            &mut sessions,
            &mut backoff,
            &mut next_attempt,
        );
        // A session that has ended and drained holds nothing worth keeping;
        // dropping it keeps the map bounded across a long run of reconnects.
        sessions.retain(|_, sink| !(sink.ended && sink.batcher.is_empty()));
    }
}

fn wait_duration(sessions: &BTreeMap<String, SessionSink>, next_attempt: Instant) -> Duration {
    let Some(deadline) = sessions
        .values()
        .filter_map(|sink| sink.batcher.flush_deadline())
        .min()
    else {
        return IDLE_POLL;
    };
    deadline
        .max(next_attempt)
        .saturating_duration_since(Instant::now())
        .clamp(Duration::from_millis(50), Duration::from_secs(1))
}

/// Flushes every session whose batch is due, each as its own envelope. The
/// backoff and next-attempt clock are shared because they describe the one
/// endpoint, not any one session.
fn flush_all(
    http: &LiveHttp,
    control: &UplinkControl,
    sessions: &mut BTreeMap<String, SessionSink>,
    backoff: &mut Backoff,
    next_attempt: &mut Instant,
) {
    for sink in sessions.values_mut() {
        let _ = flush(http, control, sink, backoff, next_attempt, false);
    }
}

fn flush(
    http: &LiveHttp,
    control: &UplinkControl,
    sink: &mut SessionSink,
    backoff: &mut Backoff,
    next_attempt: &mut Instant,
    force: bool,
) -> FlushOutcome {
    let now = Instant::now();
    if !force && now < *next_attempt {
        return FlushOutcome::NothingDue;
    }
    let batcher = &mut sink.batcher;
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
    let session = &sink.meta;
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

fn shutdown_flush_all(control: &UplinkControl, sessions: &mut BTreeMap<String, SessionSink>) {
    let deadline = Instant::now() + SHUTDOWN_FLUSH_DEADLINE;
    let http = LiveHttp::brief();
    let mut backoff = Backoff::default();
    let mut next_attempt = Instant::now();
    for sink in sessions.values_mut() {
        while !sink.batcher.is_empty() && Instant::now() < deadline {
            match flush(&http, control, sink, &mut backoff, &mut next_attempt, true) {
                FlushOutcome::Sent => {}
                // At shutdown there is no later; whatever could not go, goes down.
                FlushOutcome::NothingDue | FlushOutcome::Deferred | FlushOutcome::Dropped => break,
            }
        }
        drain_as_dropped(&mut sink.batcher, control);
    }
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
    let body = format!("{{\"product\":\"sc2\",\"client_version\":\"{CLIENT_VERSION}\"}}");
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
    use superiority_core::native::presence::PresenceState;

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
                TapMessage::Event { dto, .. } => dto.seq,
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
            TapMessage::Event {
                dto: EventDto {
                    kind: EventKind::Heartbeat,
                    ..
                },
                ..
            }
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
        while let Ok(TapMessage::Event { dto, .. }) = receiver.try_recv() {
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
