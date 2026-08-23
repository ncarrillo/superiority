use std::{
    cell::{Cell, RefCell},
    fs::OpenOptions,
    io::{ErrorKind, Write as _},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

use url::Url;

use crate::{
    Error, Result,
    auth::{CredentialStore, FileCredentialStore, authenticate_cached},
    bgs::{Client, Endpoint, SecretBytes},
    chat::{ChatChannel, ChatEvent, GENERAL_PUBLIC_CHANNEL, LiveChat},
    games::scr::{
        self,
        chat::{ChatEvent as ClassicChatEvent, ChatFriend as ClassicChatFriend},
        session::ClassicSession,
    },
    games::wc3::{
        ChatChannel as WarcraftChatChannel, ChatEvent as WarcraftChatEvent,
        ChatFriend as WarcraftChatFriend, ClanSnapshot as WarcraftClanSnapshot,
        session::{Step as WarcraftStep, WarcraftSession},
    },
    native::{Connector, Protocol, WhisperTarget, protocol::MAX_JOINED_CHANNELS},
    observer::{SessionObserver, SessionObserverFactory},
    product::Product,
    wire::websocket::SocketInterrupt,
};

pub const DEFAULT_PUBLIC_CHANNEL: u16 = GENERAL_PUBLIC_CHANNEL;
const LIVE_POLL_INTERVAL: Duration = Duration::from_millis(200);
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(60);
const OBSERVER_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const TRANSPORT_MAINTENANCE_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone, Debug)]
pub enum ClientCommand {
    Connect {
        force_interactive: bool,
        /// stable numeric id of the authoritative Battle.net account. Every
        /// product protocol returns this id, so it is the identity boundary;
        /// the display BattleTag is retained only for useful diagnostics.
        expected_account_id: Option<u64>,
        /// the BattleTag established by the app's authoritative account
        /// session, used to name that account in errors.
        expected_battle_tag: Option<String>,
        channels: Vec<ChatChannel>,
    },
    /// installs a product-scoped credential minted by the authoritative
    /// Battle.net session before this worker begins its queued connection.
    InstallCredential(SecretBytes),
    Disconnect,
    SignOut,
    JoinChannel(ChatChannel),
    /// Remastered's join, which names a channel rather than describing one.
    ///
    /// `StarCraft II`'s `JoinChannel` carries a `ChatChannel` — its own catalogue
    /// type, with none of which Remastered has an equivalent. The classic
    /// channel is named or numbered, and the worker resolves which.
    JoinClassic(String),
    /// a slash command entered in Remastered's classic chat composer.
    SendClassicCommand(String),
    /// selects one of the public channels advertised by WC3's AuroraChat.
    JoinWarcraft(String),
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
        /// which product is minting a credential. The browser session itself
        /// is shared so all products authorize the same Battle.net account.
        product: Product,
        /// clears the shared Battle.net browser session before opening. Used
        /// only for an explicit account-wide switch. A mismatched product
        /// credential is rotated through the existing authoritative SSO store.
        fresh_account: bool,
    },
    Chat(ChatEvent),
    /// the classic channel this session is in, and who is in it. Re-sent
    /// whenever the roster moves.
    ClassicChannel(scr::chat::ChatChannel),
    /// Remastered's chat, which does not fit `ChatEvent`: that enum is
    /// `StarCraft II`'s, down to conferences, shards, and a `u8` channel slot.
    /// The classic channel has numeric channel ids and six message kinds, so it
    /// travels as itself rather than being bent into a shape it is not.
    Classic(ClassicChatEvent),
    /// Remastered's Battle.net friends as announced by the classic chat edge.
    /// The snapshot is retained across offline transitions so the Social panel
    /// does not make a friend disappear merely because they left the game.
    ClassicFriends(Vec<ClassicChatFriend>),
    /// confirmation that Remastered's LegacyChat edge accepted an outgoing
    /// whisper command. Unlike SC2, that edge does not reliably echo the
    /// accepted message back to the sender, so the worker supplies the single
    /// authoritative UI event after the RPC succeeds.
    ClassicWhisperSent {
        peer: String,
        body: String,
    },
    /// one of WC3's joined AuroraChat rosters. It is product-owned for the same
    /// reason the SC:R roster is: neither has SC2 conferences or channel slots.
    WarcraftChannel(WarcraftChatChannel),
    /// WC3 AuroraChat transcript and membership activity.
    Warcraft(WarcraftChatEvent),
    /// WC3's account-level AuroraFriends roster.
    WarcraftFriends(Vec<WarcraftChatFriend>),
    /// WC3's read-only clan descriptor and roster, exactly as reported by the
    /// retail Clan service. `Pending` remains distinct from an authoritative
    /// empty `ReceivedMyClanOnLogin` callback.
    WarcraftClan(WarcraftClanSnapshot),
    /// the public-channel catalogue returned during WC3 startup.
    WarcraftChannels(Vec<String>),
    /// a product-scoped ticket minted by the authoritative account session.
    /// The host routes it to that product's worker before queuing Connect.
    ProductCredential {
        product: Product,
        credential: SecretBytes,
    },
    /// who signed in and what they own. Sent once, on connect: none of it
    /// changes while you are signed in.
    Account(AccountSummary),
    CommandError(String),
    Error(String),
}

/// what the service says about the account behind this session.
#[derive(Clone, Debug, Default)]
pub struct AccountSummary {
    /// stable numeric Battle.net account identity shared by every product
    /// protocol. This, not the renameable BattleTag, binds sessions together.
    pub account_id: Option<u64>,
    pub battle_tag: Option<String>,
    /// the Battle.net region this session is connected through.
    pub region: Option<u32>,
    /// retail products selected by Battle.net Desktop's signed catalog rules,
    /// by FourCC. `None` means account state did not answer.
    pub games: Option<Vec<String>>,
}

pub struct ClientHandle {
    pub commands: Sender<ClientCommand>,
    pub events: Receiver<ClientEvent>,
    /// closed when the session thread has finished. nothing is ever sent on it;
    /// the worker owns the sender, so a receive failing is the signal that the
    /// thread is gone. lets an embedder wait for teardown — with a timeout,
    /// which a `JoinHandle` could not offer — instead of guessing.
    pub finished: Receiver<Finished>,
}

/// uninhabited: `finished` only ever reports that the sender was dropped.
pub enum Finished {}

/// signs in against the Superiority app's own credential cache. embedders that
/// are not the app should use [`spawn_client_with`] and name their own store.
#[must_use]
pub fn spawn_client(product: Product, observer: Box<dyn SessionObserverFactory>) -> ClientHandle {
    spawn_client_with(
        product,
        observer,
        Box::new(FileCredentialStore::for_product(product)),
    )
}

/// signs in against `credentials`, so an embedder decides where the cached
/// session lives rather than inheriting the app's.
#[must_use]
pub fn spawn_client_with(
    product: Product,
    observer: Box<dyn SessionObserverFactory>,
    credentials: Box<dyn CredentialStore + Send>,
) -> ClientHandle {
    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel::<Finished>();
    thread::Builder::new()
        .name("sc2-network".into())
        .spawn(move || {
            run_worker(
                product,
                &command_rx,
                &event_tx,
                observer.as_ref(),
                credentials.as_ref(),
            );
            drop(finished_tx);
        })
        .expect("network thread must start");
    ClientHandle {
        commands: command_tx,
        events: event_rx,
        finished: finished_rx,
    }
}

fn run_worker(
    product: Product,
    commands: &Receiver<ClientCommand>,
    events: &Sender<ClientEvent>,
    observer: &dyn SessionObserverFactory,
    credentials: &dyn CredentialStore,
) {
    while let Ok(command) = commands.recv() {
        match command {
            ClientCommand::Connect {
                force_interactive,
                expected_account_id,
                expected_battle_tag,
                channels,
            } => {
                if let Err(error) = connect_once(
                    product,
                    commands,
                    events,
                    observer,
                    credentials,
                    force_interactive,
                    expected_account_id,
                    expected_battle_tag.as_deref(),
                    channels,
                ) {
                    // named: more than one product's worker traces to the same
                    // place now, and an untagged failure could be either
                    trace_connection(format_args!(
                        "[{}] connection ended: {error:?}",
                        product.code()
                    ));
                    emit(events, ClientEvent::Error(error.to_string()));
                    emit(events, ClientEvent::Stage(ConnectionStage::Disconnected));
                }
            }
            ClientCommand::InstallCredential(credential) => {
                trace_connection(format_args!(
                    "[{}] installing credential minted by authoritative account",
                    product.code()
                ));
                if let Err(error) = credentials.store(&credential) {
                    emit(events, ClientEvent::Error(error.to_string()));
                }
            }
            ClientCommand::SignOut => {
                if let Err(error) = credentials.delete() {
                    emit(events, ClientEvent::Error(error.to_string()));
                }
                if product == Product::Warcraft3
                    && let Err(error) = super::super::games::wc3::session::delete_offline_cookie()
                {
                    emit(events, ClientEvent::Error(error.to_string()));
                }
                emit(events, ClientEvent::Stage(ConnectionStage::Disconnected));
            }
            ClientCommand::Quit => break,
            // Remastered's join only means something inside its own session
            ClientCommand::JoinClassic(_)
            | ClientCommand::SendClassicCommand(_)
            | ClientCommand::JoinWarcraft(_)
            | ClientCommand::Disconnect
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
    product: Product,
    commands: &Receiver<ClientCommand>,
    events: &Sender<ClientEvent>,
    observer: &dyn SessionObserverFactory,
    credentials: &dyn CredentialStore,
    force_interactive: bool,
    expected_account_id: Option<u64>,
    expected_battle_tag: Option<&str>,
    channels: Vec<ChatChannel>,
) -> Result<()> {
    // both classic products branch before Front is opened. Each has a JSON
    // account service and only that service issues a ticket its classic route
    // accepts.
    if product == Product::Remastered {
        return run_remastered(
            commands,
            events,
            observer,
            credentials,
            force_interactive,
            expected_account_id,
            expected_battle_tag,
        );
    }
    if product == Product::Warcraft3 {
        return run_warcraft(
            commands,
            events,
            observer,
            credentials,
            force_interactive,
            expected_account_id,
            expected_battle_tag,
        );
    }
    let channels = normalized_channels(channels);
    let protocol = load_protocol()?;
    emit(
        events,
        ClientEvent::Stage(ConnectionStage::WebAuthentication),
    );
    let mut bgs = Client::open(&Endpoint::default(), product)?;
    bgs.establish()?;

    // a sign-in that is going nowhere has to end by itself. the read blocks
    // below the TLS layer, where no timeout reaches it, so the only way to give
    // up is to cut the connection from another thread and let the read fail.
    let waiting = Arc::new(AtomicBool::new(true));
    let at_login_page = Arc::new(AtomicBool::new(false));
    let watchdog = LogonWatchdog::arm(
        bgs.interrupt()?,
        &waiting,
        &at_login_page,
        Endpoint::default().timeout,
    );

    let mut browser = |url: &Url| {
        let (reply, response) = mpsc::channel();
        events
            .send(ClientEvent::Authentication {
                url: url.clone(),
                reply,
                product,
                fresh_account: force_interactive && expected_account_id.is_none(),
            })
            .map_err(|_| application_closed())?;
        // somebody is reading a login page now, which takes as long as it takes
        at_login_page.store(true, Ordering::Relaxed);
        let credential = response.recv().map_err(|_| application_closed())?;
        at_login_page.store(false, Ordering::Relaxed);
        credential
    };
    let authentication =
        authenticate_cached(&mut bgs, credentials, &mut browser, force_interactive);
    watchdog.disarm();
    let authentication = authentication.map_err(|error| {
        if watchdog.fired() {
            Error::Server(
                "Battle.net accepted the connection but never answered the sign-in.".into(),
            )
        } else {
            error
        }
    })?;
    let account_id = authentication
        .session
        .account_id
        .map(|id| id.low)
        .ok_or_else(|| {
            Error::Authentication(
                "Battle.net authenticated the session without a numeric account id".into(),
            )
        })?;
    ensure_authoritative_account(
        product,
        expected_account_id,
        expected_battle_tag,
        Some(account_id),
        authentication.session.battle_tag.as_deref(),
    )?;
    // evaluate exactly the same inputs and retail rules as Battle.net Desktop.
    // in particular, W3 beta id 50676 is intentionally absent from the retail
    // predicate even though it creates a W3 game-account record.
    let retail_games = match bgs.account_catalog(&authentication.session) {
        Ok(catalog) => {
            for game in &catalog.games {
                trace_connection(format_args!(
                    "account product record: {} ({}){}{}",
                    game.code(),
                    game.name.as_deref().unwrap_or("unnamed"),
                    if game.is_trial { " trial" } else { "" },
                    if game.accounts > 0 {
                        format!(" · {} account(s)", game.accounts)
                    } else {
                        String::new()
                    },
                ));
            }
            if catalog.games.is_empty() {
                trace_connection("account state named no product records");
            }
            let license_ids = catalog
                .licenses
                .iter()
                .map(|license| license.id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            trace_connection(format_args!("account license ids: [{license_ids}]"));
            Some(
                catalog
                    .retail_products(false)
                    .into_iter()
                    .map(|product| product.code().to_owned())
                    .collect::<Vec<_>>(),
            )
        }
        Err(error) => {
            trace_connection(format_args!("account state unavailable: {error}"));
            None
        }
    };
    // one interactive account authorization is enough. Battle.net's
    // authenticated Front session can mint a product-scoped credential for
    // every provisioned protocol; route those tickets to the queued workers
    // before publishing Account (which is what starts that queue).
    if product == Product::StarCraft2
        && let Some(games) = retail_games.as_ref()
    {
        for secondary in [Product::Remastered, Product::Warcraft3] {
            if !games.iter().any(|code| code == secondary.code()) {
                continue;
            }
            match bgs.generate_web_credentials_for(secondary) {
                Ok(credential) => {
                    trace_connection(format_args!(
                        "[{}] credential minted by authoritative account",
                        secondary.code()
                    ));
                    emit(
                        events,
                        ClientEvent::ProductCredential {
                            product: secondary,
                            credential,
                        },
                    );
                }
                Err(error) => trace_connection(format_args!(
                    "[{}] authoritative credential mint failed: {error}",
                    secondary.code()
                )),
            }
        }
    }
    trace_connection(format_args!(
        "account battletag={:?}",
        authentication.session.battle_tag
    ));
    emit(
        events,
        ClientEvent::Account(AccountSummary {
            account_id: Some(account_id),
            battle_tag: authentication.session.battle_tag.clone(),
            region: authentication.session.connected_region,
            games: retail_games,
        }),
    );
    // Remastered never reaches here — it branched before the Front channel was
    // opened, because its account layer is a different protocol
    if product != Product::StarCraft2 {
        return Err(Error::Server(format!(
            "Signed in to {}, but this client does not speak its chat protocol yet.",
            product.name()
        )));
    }
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
    let mut tap = observer.begin_session(&channels);
    for event in initial_events {
        tap.observe(&event);
        emit(events, ClientEvent::Chat(event));
    }
    emit(events, ClientEvent::Stage(ConnectionStage::Connected));
    run_live(commands, events, chat, tap.as_mut(), credentials)
}

fn normalized_channels(channels: Vec<ChatChannel>) -> Vec<ChatChannel> {
    let mut normalized = Vec::new();
    for channel in channels {
        if channel != ChatChannel::Party && !normalized.contains(&channel) {
            normalized.push(channel);
        }
    }
    if normalized.is_empty() {
        normalized.push(ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL));
    }
    normalized.truncate(MAX_JOINED_CHANNELS);
    normalized
}

fn ensure_authoritative_account(
    product: Product,
    expected_account_id: Option<u64>,
    expected_battle_tag: Option<&str>,
    actual_account_id: Option<u64>,
    actual_battle_tag: Option<&str>,
) -> Result<()> {
    let Some(expected_id) = expected_account_id else {
        return Ok(());
    };
    let expected_name = expected_battle_tag.unwrap_or("the authoritative account");
    let Some(actual_id) = actual_account_id else {
        return Err(Error::Authentication(format!(
            "{} did not return a numeric Battle.net account id; expected {expected_name} ({expected_id})",
            product.name()
        )));
    };
    if actual_id == expected_id {
        return Ok(());
    }
    let actual_name = actual_battle_tag.unwrap_or("an unknown account");
    Err(Error::Authentication(format!(
        "{} authenticated {actual_name} ({actual_id}), but Superiority is signed in as {expected_name} ({expected_id})",
        product.name(),
    )))
}

/// Remastered's session, which is its own protocol from the account layer down.
///
/// Nothing here touches [`crate::platform::bgs`]: Aurora is a different
/// protocol at the same host, and the ticket it issues is the only one the
/// classic edge accepts. The stages are shared because they describe the same
/// shape of bring-up — sign in, be told where the server is, start chat — even
/// though every message inside them differs.
fn run_remastered(
    commands: &Receiver<ClientCommand>,
    events: &Sender<ClientEvent>,
    observer: &dyn SessionObserverFactory,
    credentials: &dyn CredentialStore,
    force_interactive: bool,
    expected_account_id: Option<u64>,
    expected_battle_tag: Option<&str>,
) -> Result<()> {
    emit(
        events,
        ClientEvent::Stage(ConnectionStage::WebAuthentication),
    );
    let timeout = Endpoint::default().timeout;

    let fresh_account = Cell::new(force_interactive && expected_account_id.is_none());
    // a browser credential is only durable after the classic account layer
    // proves that it belongs to the authoritative Battle.net account. Keeping
    // it in the callback used to persist a previous account's still-valid SSO
    // ticket before that check ran, trapping every reconnect in the same
    // mismatch.
    let pending_credential = RefCell::new(None);
    let mut browser = |url: &Url| {
        let (reply, response) = mpsc::channel();
        events
            .send(ClientEvent::Authentication {
                url: url.clone(),
                reply,
                product: Product::Remastered,
                fresh_account: fresh_account.get(),
            })
            .map_err(|_| application_closed())?;
        let credential: SecretBytes = response.recv().map_err(|_| application_closed())??;
        pending_credential.replace(Some(credential.clone()));
        Ok(credential)
    };

    // a cached credential is replayed; without one the bootstrap marker asks
    // Battle.net for a fresh challenge, which is what opens the browser
    let cached = if force_interactive {
        None
    } else {
        credentials.load()?
    };
    let had_cached_credential = cached.is_some();
    let credential = match cached {
        Some(credential) => credential,
        None => SecretBytes::new(scr::aurora::CHALLENGE_BOOTSTRAP_CREDENTIAL.to_vec())?,
    };

    // the same four stages StarCraft II reports, at Remastered's equivalents:
    // without them the card jumped from step 1 to step 4 with nothing between
    let stage_of = |step| match step {
        scr::session::Step::SigningIn => ConnectionStage::WebAuthentication,
        scr::session::Step::AskingForServer => ConnectionStage::GameUtilities,
        scr::session::Step::Authenticating => ConnectionStage::NativeAuthentication,
        scr::session::Step::StartingChat => ConnectionStage::ChatBootstrap,
    };
    let identity_mismatch = Cell::new(false);
    let mut validate_account = |account_id: u64, battle_tag: Option<&str>| {
        let result = ensure_authoritative_account(
            Product::Remastered,
            expected_account_id,
            expected_battle_tag,
            Some(account_id),
            battle_tag,
        );
        identity_mismatch.set(result.is_err());
        result
    };
    let mut connected = scr::session::connect(
        &credential,
        &mut browser,
        timeout,
        &mut validate_account,
        |line| trace_connection(format_args!("[S1] {line}")),
        |step| emit(events, ClientEvent::Stage(stage_of(step))),
    );
    // a valid ticket for a different account is not a toon-selection problem:
    // it means WebKit inherited a previous Battle.net SSO identity. Discard
    // the ticket and clear that browser identity before one explicit retry.
    // this is the only safe way to prevent two product sessions from silently
    // representing different accounts.
    let mismatched_identity = identity_mismatch.replace(false);
    if mismatched_identity {
        trace_connection("[S1] authorization belongs to another account; clearing the web session");
        let _ = credentials.delete();
        pending_credential.borrow_mut().take();
        fresh_account.set(true);
        let bootstrap = SecretBytes::new(scr::aurora::CHALLENGE_BOOTSTRAP_CREDENTIAL.to_vec())?;
        connected = scr::session::connect(
            &bootstrap,
            &mut browser,
            timeout,
            &mut validate_account,
            |line| trace_connection(format_args!("[S1] {line}")),
            |step| emit(events, ClientEvent::Stage(stage_of(step))),
        );
    } else if had_cached_credential
        && !force_interactive
        && matches!(connected, Err(Error::Authentication(_) | Error::Server(_)))
    {
        // a cached credential that the service rejected is worth exactly one
        // product-token rotation. Network errors do not enter this branch.
        trace_connection(format_args!(
            "[S1] cached credential did not work; asking for a fresh sign-in"
        ));
        let _ = credentials.delete();
        let bootstrap = SecretBytes::new(scr::aurora::CHALLENGE_BOOTSTRAP_CREDENTIAL.to_vec())?;
        connected = scr::session::connect(
            &bootstrap,
            &mut browser,
            timeout,
            &mut validate_account,
            |line| trace_connection(format_args!("[S1] {line}")),
            |step| emit(events, ClientEvent::Stage(stage_of(step))),
        );
    }
    let mut classic = connected?;
    ensure_authoritative_account(
        Product::Remastered,
        expected_account_id,
        expected_battle_tag,
        Some(classic.account_id()),
        classic.battle_tag(),
    )?;
    if let Some(credential) = pending_credential.borrow_mut().take() {
        // kept, so a validated account can reconnect without flashing the
        // browser challenge on every launch.
        if let Err(error) = credentials.store(&credential) {
            trace_connection(format_args!("[S1] could not keep the credential: {error}"));
        }
    }

    // the card says which region you came in through, and only the account
    // layer knows it. `games` is left empty: ownership came from the account
    // service and this is not it. The BattleTag is how the account surface
    // finds you in the roster — without it the flyout can only say
    // "YOUR BATTLE.NET ACCOUNT" at an unknown presence.
    trace_connection(format_args!(
        "[S1] account battletag={:?}",
        classic.battle_tag()
    ));
    emit(
        events,
        ClientEvent::Account(AccountSummary {
            account_id: Some(classic.account_id()),
            battle_tag: classic.battle_tag().map(str::to_owned),
            region: u32::try_from(classic.connected_region()).ok(),
            games: None,
        }),
    );

    classic.join(scr::session::DEFAULT_CHANNEL)?;
    trace_connection(format_args!(
        "[S1] joined channel {} ({} known)",
        classic.channel(),
        classic.state().channels().count()
    ));
    // the tap begins once the session is in a channel, like StarCraft II's;
    // it is told the BattleTag because the classic roster never says which
    // member is you
    let mut tap = observer
        .begin_classic_session(Product::Remastered, classic.battle_tag().map(str::to_owned));
    publish_classic_channel(events, tap.as_mut(), &classic);
    // joining produces the welcome, population, and help notices. Publish
    // them now, after the UI knows which channel owns the transcript; waiting
    // for another inbound frame leaves them stuck on an otherwise quiet room.
    emit_classic_events(events, tap.as_mut(), &mut classic);

    classic.set_timeout(Some(LIVE_POLL_INTERVAL))?;
    emit(events, ClientEvent::Stage(ConnectionStage::Connected));
    let outcome = run_live_classic(commands, events, tap.as_mut(), classic, credentials);
    tap.end_session();
    outcome
}

/// the classic channel, whole, to the UI and to the tap — it is re-sent rather
/// than diffed, because the classic edge describes membership, not changes.
fn publish_classic_channel(
    events: &Sender<ClientEvent>,
    tap: &mut dyn SessionObserver,
    classic: &ClassicSession,
) {
    if let Some(channel) = classic.state().channel(classic.channel()) {
        tap.observe_classic_channel(channel);
        emit(events, ClientEvent::ClassicChannel(channel.clone()));
    }
}

/// Warcraft III's complete JSON-BGS to Classic/AuroraChat session.
fn run_warcraft(
    commands: &Receiver<ClientCommand>,
    events: &Sender<ClientEvent>,
    observer: &dyn SessionObserverFactory,
    credentials: &dyn CredentialStore,
    force_interactive: bool,
    expected_account_id: Option<u64>,
    expected_battle_tag: Option<&str>,
) -> Result<()> {
    let timeout = Endpoint::default().timeout;
    let had_cached_credential = !force_interactive && credentials.load()?.is_some();
    let fresh_account = Cell::new(force_interactive && expected_account_id.is_none());
    emit(
        events,
        ClientEvent::Stage(ConnectionStage::WebAuthentication),
    );
    let mut browser = |url: &Url| {
        let (reply, response) = mpsc::channel();
        events
            .send(ClientEvent::Authentication {
                url: url.clone(),
                reply,
                product: Product::Warcraft3,
                fresh_account: fresh_account.get(),
            })
            .map_err(|_| application_closed())?;
        response.recv().map_err(|_| application_closed())?
    };
    let stage_of = |step| match step {
        WarcraftStep::SigningIn => ConnectionStage::WebAuthentication,
        WarcraftStep::AskingForServer => ConnectionStage::GameUtilities,
        WarcraftStep::Authenticating => ConnectionStage::NativeAuthentication,
        WarcraftStep::StartingChat => ConnectionStage::ChatBootstrap,
    };
    let mut on_step = |step| emit(events, ClientEvent::Stage(stage_of(step)));
    let mut on_account = |battle_tag: Option<&str>, region| {
        trace_connection(format_args!(
            "[W3] account battletag={battle_tag:?} region={region:?}"
        ));
    };
    let identity_mismatch = Cell::new(false);
    let mut validate_account = |account_id: u64, battle_tag: Option<&str>| {
        let result = ensure_authoritative_account(
            Product::Warcraft3,
            expected_account_id,
            expected_battle_tag,
            Some(account_id),
            battle_tag,
        );
        identity_mismatch.set(result.is_err());
        result
    };
    let mut connected = WarcraftSession::connect(
        credentials,
        &mut browser,
        force_interactive,
        timeout,
        &mut validate_account,
        &mut on_step,
        &mut on_account,
    );
    // a generated token can remain valid while belonging to an identity an
    // older release selected only for WC3. Replace it once. Classic rejection
    // also gets one product-token rotation. Neither case clears the shared SSO
    // store: that store is the already-established authoritative identity.
    if identity_mismatch.get()
        || (had_cached_credential && connected.as_ref().is_err_and(warcraft_entitlement_rejected))
    {
        if let Ok(warcraft) = connected.as_mut() {
            let _ = warcraft.close();
        }
        trace_connection("[W3] cached authorization did not match; minting a replacement");
        let _ = credentials.delete();
        let _ = super::super::games::wc3::session::delete_offline_cookie();
        fresh_account.set(false);
        connected = WarcraftSession::connect(
            credentials,
            &mut browser,
            true,
            timeout,
            &mut validate_account,
            &mut on_step,
            &mut on_account,
        );
    }
    let mut warcraft = connected?;
    ensure_authoritative_account(
        Product::Warcraft3,
        expected_account_id,
        expected_battle_tag,
        Some(warcraft.account_id()),
        warcraft.battle_tag(),
    )?;
    emit(
        events,
        ClientEvent::Account(AccountSummary {
            account_id: Some(warcraft.account_id()),
            battle_tag: warcraft.battle_tag().map(str::to_owned),
            region: warcraft.connected_region(),
            games: None,
        }),
    );
    emit(
        events,
        ClientEvent::WarcraftChannels(warcraft.public_channels()),
    );
    let mut tap = observer
        .begin_classic_session(Product::Warcraft3, warcraft.battle_tag().map(str::to_owned));
    emit_warcraft_events(events, tap.as_mut(), warcraft.take_initial_events());
    let mut last_channels = Vec::new();
    publish_warcraft_channels(events, tap.as_mut(), &warcraft, &mut last_channels);
    emit(events, ClientEvent::Stage(ConnectionStage::Connected));
    let outcome = run_live_warcraft(
        commands,
        events,
        tap.as_mut(),
        warcraft,
        last_channels,
        credentials,
    );
    tap.end_session();
    outcome
}

fn warcraft_entitlement_rejected(error: &Error) -> bool {
    matches!(
        error,
        Error::Authentication(message) if message.contains("Classic AuthSession rejected")
    )
}

fn run_live_warcraft(
    commands: &Receiver<ClientCommand>,
    events: &Sender<ClientEvent>,
    tap: &mut dyn SessionObserver,
    mut warcraft: WarcraftSession,
    mut last_channels: Vec<WarcraftChatChannel>,
    credentials: &dyn CredentialStore,
) -> Result<()> {
    let mut friends_revision = u64::MAX;
    let mut clan_revision = u64::MAX;
    let mut next_observer_heartbeat = Instant::now() + OBSERVER_HEARTBEAT_INTERVAL;
    loop {
        loop {
            match commands.try_recv() {
                Ok(ClientCommand::Disconnect) => {
                    let _ = warcraft.close();
                    emit(events, ClientEvent::Stage(ConnectionStage::Disconnected));
                    return Ok(());
                }
                Ok(ClientCommand::SignOut) => {
                    let _ = warcraft.close();
                    credentials.delete()?;
                    super::super::games::wc3::session::delete_offline_cookie()?;
                    emit(events, ClientEvent::Stage(ConnectionStage::Disconnected));
                    return Ok(());
                }
                Ok(ClientCommand::InstallCredential(credential)) => {
                    credentials.store(&credential)?;
                }
                Ok(ClientCommand::Quit) | Err(TryRecvError::Disconnected) => {
                    let _ = warcraft.close();
                    return Ok(());
                }
                Ok(ClientCommand::JoinWarcraft(channel)) => match warcraft.join(&channel) {
                    Ok(activity) => emit_warcraft_events(events, tap, activity),
                    Err(error) => emit(events, ClientEvent::CommandError(error.to_string())),
                },
                Ok(ClientCommand::SendMessage {
                    channel_index,
                    body,
                }) => match warcraft.send_message_to(channel_index, &body) {
                    Ok(activity) => emit_warcraft_events(events, tap, activity),
                    Err(error) => emit(events, ClientEvent::CommandError(error.to_string())),
                },
                Ok(ClientCommand::LeaveChannel { channel_index }) => {
                    match warcraft.leave(channel_index) {
                        Ok(activity) => emit_warcraft_events(events, tap, activity),
                        Err(error) => {
                            // the desktop follows SC2 and removes a tab as soon
                            // as its close button is accepted. Force the still-
                            // joined authoritative snapshot back through if
                            // Battle.net rejects the leave so the room cannot
                            // disappear only in local UI state.
                            last_channels.retain(|channel| channel.id != channel_index);
                            emit(events, ClientEvent::CommandError(error.to_string()));
                        }
                    }
                }
                Ok(ClientCommand::SendWhisper {
                    target,
                    display_name: _,
                    body,
                }) => match target {
                    WhisperTarget::WarcraftAccount(account_id) => {
                        match warcraft.send_whisper(account_id, &body) {
                            Ok(activity) => emit_warcraft_events(events, tap, activity),
                            Err(error) => {
                                emit(events, ClientEvent::CommandError(error.to_string()));
                            }
                        }
                    }
                    _ => emit(
                        events,
                        ClientEvent::CommandError(
                            "WC3 whisper target has no AuroraFriends account identity".into(),
                        ),
                    ),
                },
                Ok(
                    ClientCommand::Connect { .. }
                    | ClientCommand::JoinClassic(_)
                    | ClientCommand::SendClassicCommand(_)
                    | ClientCommand::JoinChannel(_)
                    | ClientCommand::AnswerGroupInvitation { .. }
                    | ClientCommand::AnswerPartyInvitation { .. }
                    | ClientCommand::SearchGroups { .. },
                )
                | Err(TryRecvError::Empty) => break,
            }
        }
        publish_warcraft_channels(events, tap, &warcraft, &mut last_channels);
        emit_warcraft_friends(events, &warcraft, &mut friends_revision);
        emit_warcraft_clan(events, &warcraft, &mut clan_revision);
        let now = Instant::now();
        if now >= next_observer_heartbeat {
            tap.heartbeat();
            next_observer_heartbeat = now + OBSERVER_HEARTBEAT_INTERVAL;
        }
        match warcraft.poll(LIVE_POLL_INTERVAL) {
            Ok(activity) => {
                emit_warcraft_events(events, tap, activity);
                publish_warcraft_channels(events, tap, &warcraft, &mut last_channels);
                emit_warcraft_friends(events, &warcraft, &mut friends_revision);
                emit_warcraft_clan(events, &warcraft, &mut clan_revision);
            }
            Err(Error::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(error) => {
                let _ = warcraft.close();
                return Err(error);
            }
        }
    }
}

fn emit_warcraft_clan(
    events: &Sender<ClientEvent>,
    warcraft: &WarcraftSession,
    emitted_revision: &mut u64,
) {
    let revision = warcraft.clan_revision();
    if revision == *emitted_revision {
        return;
    }
    *emitted_revision = revision;
    trace_connection(format_args!(
        "[W3] clan snapshot={:?}",
        warcraft.clan().membership
    ));
    emit(events, ClientEvent::WarcraftClan(warcraft.clan().clone()));
}

fn emit_warcraft_friends(
    events: &Sender<ClientEvent>,
    warcraft: &WarcraftSession,
    emitted_revision: &mut u64,
) {
    let revision = warcraft.friends_revision();
    if revision == *emitted_revision {
        return;
    }
    *emitted_revision = revision;
    let friends = warcraft.friends().cloned().collect::<Vec<_>>();
    trace_connection(format_args!("[W3] friends snapshot={}", friends.len()));
    emit(events, ClientEvent::WarcraftFriends(friends));
}

fn emit_warcraft_events(
    events: &Sender<ClientEvent>,
    tap: &mut dyn SessionObserver,
    activity: Vec<WarcraftChatEvent>,
) {
    for event in activity {
        tap.observe_warcraft(&event);
        emit(events, ClientEvent::Warcraft(event));
    }
}

fn publish_warcraft_channels(
    events: &Sender<ClientEvent>,
    tap: &mut dyn SessionObserver,
    warcraft: &WarcraftSession,
    previous: &mut Vec<WarcraftChatChannel>,
) {
    let current = warcraft.channels().cloned().collect::<Vec<_>>();
    for channel in &current {
        if previous
            .iter()
            .any(|prior| prior.id == channel.id && prior == channel)
        {
            continue;
        }
        tap.observe_warcraft_channel(channel);
        emit(events, ClientEvent::WarcraftChannel(channel.clone()));
    }
    *previous = current;
}

/// pumps the classic channel until told to stop. Deliberately narrower than
/// [`run_live`]: Remastered has no parties or groups, but it does keep the
/// classic friend and whisper state consumed by the Social surface.
fn run_live_classic(
    commands: &Receiver<ClientCommand>,
    events: &Sender<ClientEvent>,
    tap: &mut dyn SessionObserver,
    mut classic: ClassicSession,
    credentials: &dyn CredentialStore,
) -> Result<()> {
    let mut friends_revision = u64::MAX;
    let mut next_keep_alive = Instant::now() + KEEP_ALIVE_INTERVAL;
    let mut next_observer_heartbeat = Instant::now() + OBSERVER_HEARTBEAT_INTERVAL;
    loop {
        match commands.try_recv() {
            Ok(ClientCommand::Disconnect) | Err(TryRecvError::Disconnected) => {
                let _ = classic.close();
                return Ok(());
            }
            Ok(ClientCommand::SignOut) => {
                let _ = classic.close();
                credentials.delete()?;
                emit(events, ClientEvent::Stage(ConnectionStage::Disconnected));
                return Ok(());
            }
            Ok(ClientCommand::InstallCredential(credential)) => {
                credentials.store(&credential)?;
            }
            Ok(ClientCommand::SendMessage { body, .. }) => match classic.send_message(&body) {
                // LegacyChat acknowledges the send and never echoes it: the
                // line the reader sees is written here, and the loop below
                // publishes it with the rest of the channel's events
                Ok(()) => classic.record_local_talk(&body),
                Err(error) => {
                    trace_connection(format_args!("[S1] classic send failed: {error}"));
                    emit(events, ClientEvent::CommandError(error.to_string()));
                }
            },
            Ok(ClientCommand::SendClassicCommand(command)) => {
                if let Err(error) = classic.execute_command(&command) {
                    trace_connection(format_args!("[S1] classic command failed: {error}"));
                    emit(events, ClientEvent::CommandError(error.to_string()));
                }
            }
            Ok(ClientCommand::SendWhisper {
                target,
                display_name,
                body,
            }) => {
                let (result, legacy_peer) = match target {
                    WhisperTarget::Account(account_id) => {
                        (classic.send_account_whisper(account_id, &body), None)
                    }
                    WhisperTarget::Name(name) => {
                        let result = classic.send_whisper(&name, &body);
                        (result, Some(name))
                    }
                    // only account and name identities belong to SC:R. Keep a
                    // guarded name fallback for conversations opened by text.
                    _ => {
                        let result = classic.send_whisper(&display_name, &body);
                        (result, Some(display_name))
                    }
                };
                if let Err(error) = result {
                    trace_connection(format_args!("[S1] classic whisper failed: {error}"));
                    emit(events, ClientEvent::CommandError(error.to_string()));
                } else if let Some(peer) = legacy_peer {
                    // LegacyChat does not reliably echo accepted commands.
                    // AuroraChat does, so account sends are recorded only when
                    // WhisperEchoReceived arrives and cannot be duplicated.
                    emit(events, ClientEvent::ClassicWhisperSent { peer, body });
                }
            }
            Ok(ClientCommand::JoinClassic(target)) => {
                // public channels have stable numeric ids. Names use SC:R's
                // dedicated custom-channel request; they do not have to be in
                // the current roster before they can be joined.
                let target = target.trim();
                let joined = match target.parse::<u32>() {
                    Ok(channel_id) => classic.join(channel_id),
                    Err(_) => classic.join_named(target),
                };
                if let Err(error) = joined {
                    trace_connection(format_args!("[S1] join failed: {error}"));
                    emit(events, ClientEvent::CommandError(error.to_string()));
                }
                // the room after the attempt, landed or rejoined: the roster
                // it came back with is newer than the one on screen
                publish_classic_channel(events, tap, &classic);
            }
            // every other command is StarCraft II's, and nothing here can serve
            // it; a quiet queue is the same non-event
            Ok(_) | Err(TryRecvError::Empty) => {}
        }
        // a synchronous send or join can receive unrelated callbacks before
        // its own response. ClassicSession retains them, so do not make their
        // delivery depend on a later successful poll.
        emit_classic_events(events, tap, &mut classic);
        emit_classic_friends(events, &classic, &mut friends_revision);
        let now = Instant::now();
        if now >= next_keep_alive {
            classic.keep_alive()?;
            next_keep_alive = now + KEEP_ALIVE_INTERVAL;
            // ping can receive and acknowledge unrelated callbacks before its
            // own empty response, so publish any state retained by the call.
            emit_classic_events(events, tap, &mut classic);
            emit_classic_friends(events, &classic, &mut friends_revision);
        }
        if now >= next_observer_heartbeat {
            tap.heartbeat();
            next_observer_heartbeat = now + OBSERVER_HEARTBEAT_INTERVAL;
        }
        match classic.resolve_next_avatar() {
            Ok(true) => publish_classic_channel(events, tap, &classic),
            Ok(false) => {}
            Err(error) => {
                trace_connection(format_args!("[S1] profile avatar lookup failed: {error}"));
            }
        }
        emit_classic_events(events, tap, &mut classic);
        emit_classic_friends(events, &classic, &mut friends_revision);
        let revision = classic.state().roster_revision();
        match classic.poll(LIVE_POLL_INTERVAL) {
            Ok(_) => {
                emit_classic_events(events, tap, &mut classic);
                emit_classic_friends(events, &classic, &mut friends_revision);
                // the roster is the whole membership, so it is re-sent rather
                // than diffed: the classic channel does not describe changes
                if classic.state().roster_revision() != revision {
                    publish_classic_channel(events, tap, &classic);
                }
            }
            // a quiet channel is the normal case, not a failure
            Err(Error::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                emit_classic_events(events, tap, &mut classic);
                emit_classic_friends(events, &classic, &mut friends_revision);
            }
            Err(error) => {
                let _ = classic.close();
                return Err(error);
            }
        }
    }
}

fn emit_classic_events(
    events: &Sender<ClientEvent>,
    tap: &mut dyn SessionObserver,
    classic: &mut ClassicSession,
) {
    for event in classic.state_mut().take_events() {
        tap.observe_classic(&event);
        emit(events, ClientEvent::Classic(event));
    }
}

fn emit_classic_friends(
    events: &Sender<ClientEvent>,
    classic: &ClassicSession,
    emitted_revision: &mut u64,
) {
    let revision = classic.state().friends_revision();
    if revision == *emitted_revision {
        return;
    }
    *emitted_revision = revision;
    let friends = classic.state().friends().cloned().collect::<Vec<_>>();
    trace_connection(format_args!("[S1] friends snapshot={}", friends.len()));
    emit(events, ClientEvent::ClassicFriends(friends));
}

fn run_live(
    commands: &Receiver<ClientCommand>,
    events: &Sender<ClientEvent>,
    mut chat: LiveChat,
    tap: &mut dyn SessionObserver,
    credentials: &dyn CredentialStore,
) -> Result<()> {
    let mut next_keep_alive = Instant::now() + KEEP_ALIVE_INTERVAL;
    let mut next_observer_heartbeat = Instant::now() + OBSERVER_HEARTBEAT_INTERVAL;
    let mut pending_channels_resolved = false;
    let mut next_transport_maintenance = Instant::now();
    loop {
        loop {
            match commands.try_recv() {
                Ok(ClientCommand::Disconnect) => {
                    tap.end_session();
                    chat.close()?;
                    emit(events, ClientEvent::Stage(ConnectionStage::Disconnected));
                    return Ok(());
                }
                Ok(ClientCommand::SignOut) => {
                    tap.end_session();
                    let _ = chat.close();
                    credentials.delete()?;
                    emit(events, ClientEvent::Stage(ConnectionStage::Disconnected));
                    return Ok(());
                }
                Ok(ClientCommand::InstallCredential(credential)) => {
                    credentials.store(&credential)?;
                }
                Ok(ClientCommand::Quit) | Err(TryRecvError::Disconnected) => {
                    tap.end_session();
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
                    if let Err(error) = chat.search_groups(&query) {
                        emit(events, ClientEvent::CommandError(error.to_string()));
                    }
                }
                // Remastered's own join; this session is StarCraft II's
                Ok(
                    ClientCommand::Connect { .. }
                    | ClientCommand::JoinClassic(_)
                    | ClientCommand::SendClassicCommand(_)
                    | ClientCommand::JoinWarcraft(_),
                ) => {}
                Err(TryRecvError::Empty) => break,
            }
        }

        maintain_live_state(
            &mut chat,
            tap,
            &mut next_keep_alive,
            &mut next_observer_heartbeat,
            &mut pending_channels_resolved,
        )?;
        if Instant::now() >= next_transport_maintenance {
            chat.maintain_transport()?;
            next_transport_maintenance = Instant::now() + TRANSPORT_MAINTENANCE_INTERVAL;
        }

        match chat.receive() {
            Ok(event) => {
                tap.observe(&event);
                if let ChatEvent::JoinRejected {
                    channel: Some(channel),
                    ..
                } = &event
                {
                    tap.reject_channel(channel);
                }
                emit(events, ClientEvent::Chat(event));
            }
            Err(Error::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(error) => return Err(error),
        }
    }
}

fn maintain_live_state(
    chat: &mut LiveChat,
    tap: &mut dyn SessionObserver,
    next_keep_alive: &mut Instant,
    next_observer_heartbeat: &mut Instant,
    pending_channels_resolved: &mut bool,
) -> Result<()> {
    let now = Instant::now();
    if now >= *next_keep_alive {
        chat.keep_alive()?;
        *next_keep_alive = now + KEEP_ALIVE_INTERVAL;
    }
    if now < *next_observer_heartbeat {
        return Ok(());
    }
    if !*pending_channels_resolved {
        tap.resolve_pending_channels();
        *pending_channels_resolved = true;
    }
    tap.reconcile(
        &chat
            .rosters()
            .into_iter()
            .map(ChatEvent::Roster)
            .collect::<Vec<_>>(),
    );
    tap.heartbeat();
    *next_observer_heartbeat = now + OBSERVER_HEARTBEAT_INTERVAL;
    Ok(())
}

fn load_protocol() -> Result<Protocol> {
    Protocol::current()
}

fn emit(events: &Sender<ClientEvent>, event: ClientEvent) {
    let _ = events.send(event);
}

fn trace_connection(message: impl std::fmt::Display) {
    let message = message.to_string();
    if crate::trace_enabled() || std::env::var_os("SUPERIORITY_PARTY_TRACE").is_some() {
        eprintln!("superiority: {message}");
    }
    if let Some(path) = std::env::var_os("SUPERIORITY_TRACE_FILE")
        && let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path)
    {
        let _ = writeln!(file, "superiority: {message}");
    }
}

fn application_closed() -> Error {
    Error::Transport("application closed".into())
}

/// gives a sign-in a deadline it cannot ignore. The clock is held while a
/// person is at the login page — that wait is theirs, not the service's — and
/// when it runs out the connection is cut, which is what turns a read that
/// would never return into an error the application can show.
struct LogonWatchdog {
    waiting: Arc<AtomicBool>,
    fired: Arc<AtomicBool>,
}

impl LogonWatchdog {
    const BEAT: Duration = Duration::from_millis(250);

    fn arm(
        interrupt: SocketInterrupt,
        waiting: &Arc<AtomicBool>,
        at_login_page: &Arc<AtomicBool>,
        patience: Duration,
    ) -> Self {
        let watchdog = Self {
            waiting: Arc::clone(waiting),
            fired: Arc::new(AtomicBool::new(false)),
        };
        let waiting = Arc::clone(waiting);
        let at_login_page = Arc::clone(at_login_page);
        let fired = Arc::clone(&watchdog.fired);
        let started = thread::Builder::new()
            .name("sc2-logon-watchdog".into())
            .spawn(move || {
                let mut spent = Duration::ZERO;
                while waiting.load(Ordering::Relaxed) {
                    thread::sleep(Self::BEAT);
                    if at_login_page.load(Ordering::Relaxed) {
                        spent = Duration::ZERO;
                        continue;
                    }
                    spent += Self::BEAT;
                    if spent >= patience {
                        if waiting.load(Ordering::Relaxed) {
                            fired.store(true, Ordering::Relaxed);
                            trace_connection("sign-in went unanswered; cutting the connection");
                            interrupt.cut();
                        }
                        return;
                    }
                }
            });
        if started.is_err() {
            // without the thread the sign-in is only as bounded as it was
            // before; that is worse, but it is not a reason to refuse to try
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

    #[test]
    fn every_product_credential_must_resolve_to_the_authoritative_account() {
        assert!(
            ensure_authoritative_account(
                Product::Warcraft3,
                Some(42),
                Some("Commander#1234"),
                Some(42),
                Some("Renamed#9000")
            )
            .is_ok(),
            "numeric identity survives a BattleTag rename"
        );
        assert!(
            ensure_authoritative_account(
                Product::Warcraft3,
                Some(42),
                Some("Commander#1234"),
                Some(84),
                Some("Other#5678")
            )
            .is_err()
        );
        assert!(
            ensure_authoritative_account(
                Product::Remastered,
                Some(42),
                Some("Commander#1234"),
                None,
                Some("Commander#1234")
            )
            .is_err()
        );
        assert!(
            ensure_authoritative_account(
                Product::StarCraft2,
                None,
                None,
                Some(42),
                Some("First#1000")
            )
            .is_ok(),
            "the initial authority has no predecessor to compare against"
        );
    }
}
