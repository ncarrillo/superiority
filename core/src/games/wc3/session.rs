use std::{
    collections::VecDeque,
    fs::{self, DirBuilder, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};

use url::Url;
use zeroize::Zeroize as _;

use crate::{
    Error, Result,
    platform::{auth::CredentialStore, bgs::SecretBytes},
};

use super::{
    account::{AccountClient, ChallengeHandler},
    classic::{ChatChannel, ChatEvent, ChatFriend, ClanSnapshot, ClassicSession},
    identity::ClientIdentity,
};

const COOKIE_FILENAME: &str = "wc3-offline-cookie.bin";
const MAX_COOKIE_BYTES: usize = 16 * 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    SigningIn,
    AskingForServer,
    Authenticating,
    StartingChat,
}

pub struct WarcraftSession {
    classic: ClassicSession,
    initial_events: VecDeque<ChatEvent>,
    /// Stable numeric Battle.net identity from the JSON-BGS logon.
    account_id: u64,
    battle_tag: Option<String>,
    connected_region: Option<u32>,
}

impl WarcraftSession {
    pub fn connect(
        credentials: &(impl CredentialStore + ?Sized),
        challenge: &mut impl ChallengeHandler,
        force_interactive: bool,
        timeout: Duration,
        mut validate_account: impl FnMut(u64, Option<&str>) -> Result<()>,
        mut on_step: impl FnMut(Step),
        mut on_account: impl FnMut(Option<&str>, Option<u32>),
    ) -> Result<Self> {
        on_step(Step::SigningIn);
        let cached = if force_interactive {
            None
        } else {
            credentials.load()?
        };
        let mut account = AccountClient::connect(timeout)?;
        let connection = account.establish()?;
        let mut stale_deleted = false;
        let mut browser = |url: &Url| {
            if cached.is_some() && !stale_deleted {
                credentials.delete()?;
                stale_deleted = true;
            }
            challenge.resolve(url)
        };
        let authentication = account.authenticate(cached.as_ref(), &mut browser)?;
        // surface the identity as soon as BGS names it. If the account-bound
        // Classic handoff is refused, the UI and diagnostics must still say
        // which Battle.net account was actually attempted.
        on_account(
            authentication.session.battle_tag.as_deref(),
            connection.connected_region,
        );
        // Account identity is known before ProcessTask consumes a product
        // handoff. Reject a stale token here so it can be rotated through the
        // authoritative shared SSO session without touching the wrong edge.
        validate_account(
            authentication.session.account_id,
            authentication.session.battle_tag.as_deref(),
        )?;
        // reading these keeps the protocol diagnostics exercised even though
        // the desktop currently renders the shared four-stage progress model.
        let _logon_diagnostics = (
            authentication.browser_used,
            authentication
                .queue_updates
                .iter()
                .filter_map(|state| state.position)
                .next_back(),
        );

        on_step(Step::AskingForServer);
        // retail performs ProcessTask before rotating the generated token.
        let endpoint = account.request_classic_endpoint(&authentication.session)?;
        let successor = account.generate_auth_token()?;
        credentials.store(&successor)?;
        // the WC3 SDK tears down its JSON-BGS channel before it opens the
        // Classic route. The handoff is a connection transfer, not two live
        // sessions for the same account. Keeping BGS open here makes the
        // Classic edge answer AuthSession with an empty rejection.
        let _ = account.close();
        drop(account);

        on_step(Step::Authenticating);
        let identity = ClientIdentity::for_current_host()?;
        let cached_cookie = load_cookie()?;
        let mut classic = match ClassicSession::establish(
            &endpoint,
            &connection,
            &authentication.session,
            &identity,
            cached_cookie.as_ref(),
            timeout,
        ) {
            Ok(classic) => classic,
            Err(_cached_error) if cached_cookie.is_some() => {
                // OfflineCookie is an opaque server lease, not durable account
                // identity. Battle.net answers an expired/revoked cookie with
                // an AuthSession body that has no encrypted OnlineStats. A
                // ProcessTask ticket is one-time, and the original BGS channel
                // has already transferred the session, so retry the complete
                // handoff on a new BGS connection without the cookie.
                delete_offline_cookie()?;
                return Self::connect(
                    credentials,
                    challenge,
                    false,
                    timeout,
                    validate_account,
                    on_step,
                    on_account,
                );
            }
            Err(error) => return Err(error),
        };
        if let Some(cookie) = classic.take_cookie() {
            store_cookie(&cookie)?;
        }

        on_step(Step::StartingChat);
        let mut initial_events = VecDeque::from(classic.dispatch_queued()?);
        if classic.public_channels().is_empty() {
            initial_events.push_back(ChatEvent::Notice {
                text: "Battle.net advertised no public Warcraft III channels.".into(),
            });
        } else {
            initial_events.extend(classic.join(0)?);
            let deadline = Instant::now() + timeout;
            while classic.channel().is_none() && Instant::now() < deadline {
                match classic.poll(Duration::from_millis(500)) {
                    Ok(events) => initial_events.extend(events),
                    Err(Error::Io(error))
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) => {}
                    Err(error) => return Err(error),
                }
            }
            if classic.channel().is_none() {
                return Err(Error::ClassicWire(
                    "WC3 joined a public channel but received no roster".into(),
                ));
            }
        }
        if let Some(cookie) = classic.take_cookie() {
            store_cookie(&cookie)?;
        }

        Ok(Self {
            classic,
            initial_events,
            account_id: authentication.session.account_id,
            battle_tag: authentication.session.battle_tag,
            connected_region: connection.connected_region,
        })
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
    pub const fn connected_region(&self) -> Option<u32> {
        self.connected_region
    }

    #[must_use]
    pub fn channel(&self) -> Option<&ChatChannel> {
        self.classic.channel()
    }

    pub fn channels(&self) -> impl Iterator<Item = &ChatChannel> {
        self.classic.channels()
    }

    #[must_use]
    pub fn public_channels(&self) -> Vec<String> {
        self.classic
            .public_channels()
            .iter()
            .enumerate()
            .map(|(index, channel)| {
                channel
                    .display_name()
                    .map_or_else(|| format!("Public Channel {}", index + 1), str::to_owned)
            })
            .collect()
    }

    pub fn friends(&self) -> impl Iterator<Item = &ChatFriend> {
        self.classic.friends()
    }

    #[must_use]
    pub const fn friends_revision(&self) -> u64 {
        self.classic.friends_revision()
    }

    #[must_use]
    pub const fn clan(&self) -> &ClanSnapshot {
        self.classic.clan()
    }

    #[must_use]
    pub const fn clan_revision(&self) -> u64 {
        self.classic.clan_revision()
    }

    pub fn take_initial_events(&mut self) -> Vec<ChatEvent> {
        self.initial_events.drain(..).collect()
    }

    pub fn join(&mut self, name: &str) -> Result<Vec<ChatEvent>> {
        self.classic.join_named(name)
    }

    pub fn send_message(&mut self, body: &str) -> Result<Vec<ChatEvent>> {
        self.classic.send_message(body)
    }

    pub fn send_message_to(&mut self, channel_id: u8, body: &str) -> Result<Vec<ChatEvent>> {
        self.classic.send_message_to(channel_id, body)
    }

    pub fn leave(&mut self, channel_id: u8) -> Result<Vec<ChatEvent>> {
        self.classic.leave(channel_id)
    }

    pub fn send_whisper(&mut self, account_id: u64, body: &str) -> Result<Vec<ChatEvent>> {
        self.classic.send_whisper(account_id, body)
    }

    pub fn poll(&mut self, timeout: Duration) -> Result<Vec<ChatEvent>> {
        let events = self.classic.poll(timeout)?;
        if let Some(cookie) = self.classic.take_cookie() {
            store_cookie(&cookie)?;
        }
        Ok(events)
    }

    pub fn close(&mut self) -> Result<()> {
        self.classic.close()
    }
}

fn cookie_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let root = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let root = std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
    });
    root.map(|root| root.join("Superiority").join("WC3").join(COOKIE_FILENAME))
}

/// Removes WC3's opaque classic-session cookie during an explicit sign-out.
pub fn delete_offline_cookie() -> Result<()> {
    let Some(path) = cookie_path() else {
        return Ok(());
    };
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(cookie_error("delete", &error)),
    }
}

fn load_cookie() -> Result<Option<SecretBytes>> {
    let Some(path) = cookie_path() else {
        return Ok(None);
    };
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(cookie_error("inspect", &error)),
    };
    if !metadata.file_type().is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_COOKIE_BYTES as u64
    {
        return Err(Error::Authentication(
            "WC3 offline cookie has an invalid file shape".into(),
        ));
    }
    let mut bytes = fs::read(path).map_err(|error| cookie_error("read", &error))?;
    if bytes.is_empty() || bytes.len() > MAX_COOKIE_BYTES {
        bytes.zeroize();
        return Err(Error::Authentication(
            "WC3 offline cookie has an invalid length".into(),
        ));
    }
    SecretBytes::new(bytes).map(Some)
}

fn store_cookie(cookie: &SecretBytes) -> Result<()> {
    if cookie.is_empty() || cookie.len() > MAX_COOKIE_BYTES {
        return Err(Error::Authentication(
            "WC3 offline cookie has an invalid length".into(),
        ));
    }
    let path = cookie_path().ok_or_else(|| {
        Error::Authentication("could not locate a WC3 offline-cookie directory".into())
    })?;
    let parent = path
        .parent()
        .ok_or_else(|| Error::Authentication("WC3 cookie path has no parent".into()))?;
    create_private_directory(parent)?;
    let temporary = parent.join(format!(
        ".{COOKIE_FILENAME}.{}-{}.tmp",
        std::process::id(),
        rand::random::<u64>()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .map_err(|error| cookie_error("create", &error))?;
    if let Err(error) = file
        .write_all(cookie.expose())
        .and_then(|()| file.sync_all())
    {
        let _ = fs::remove_file(&temporary);
        return Err(cookie_error("write", &error));
    }
    if let Err(error) = fs::rename(&temporary, &path) {
        let _ = fs::remove_file(&temporary);
        return Err(cookie_error("replace", &error));
    }
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| cookie_error("secure", &error))?;
    Ok(())
}

fn create_private_directory(path: &Path) -> Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder
        .create(path)
        .map_err(|error| cookie_error("create directory", &error))?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| cookie_error("secure directory", &error))?;
    Ok(())
}

fn cookie_error(operation: &str, error: &std::io::Error) -> Error {
    Error::Authentication(format!("WC3 offline cookie could not {operation}: {error}"))
}
