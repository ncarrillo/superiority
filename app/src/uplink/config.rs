//! Runtime configuration and counters for Live sharing.
//!
//! The main thread writes [`UplinkConfig`] when the user flips a toggle; the
//! network-thread tap and the uplink worker read snapshots of it. Counters in
//! [`UplinkStats`] travel the other way, main-thread-read for the status line.

use std::{
    collections::BTreeSet,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use zeroize::Zeroizing;

/// Environment override for the backend base URL (scheme + host, no path).
pub const ENDPOINT_ENV: &str = "SUPERIORITY_LIVE_ENDPOINT";
/// Environment override for the feed token, for development runs.
pub const TOKEN_ENV: &str = "SC2_LIVE_TOKEN";
/// Where releases point when no override is set: the deployed Live backend.
pub const DEFAULT_ENDPOINT_BASE: &str = "https://live.superioritybot.com";

/// Filename of the stored feed registration, kept beside the Battle.net
/// credential in Application Support.
pub const IDENTITY_FILENAME: &str = "live-feed.json";

#[derive(Clone, Default)]
pub struct UplinkConfig {
    /// Master switch. Off means the tap projects nothing and sends nothing.
    pub enabled: bool,
    /// Channel identity strings (`public:1028`, `private:Op Test`,
    /// `club:5322`) the user chose to share.
    pub shared_channels: BTreeSet<String>,
    /// Backend origin override; the env var and then the default fill in.
    pub endpoint_base: Option<String>,
    /// The secret feed token minted at registration.
    pub token: Option<Zeroizing<String>>,
    /// The public feed slug the token writes into.
    pub feed_id: Option<String>,
}

impl UplinkConfig {
    /// The backend origin to talk to: env override, then stored value, then
    /// the compiled-in default.
    #[must_use]
    pub fn endpoint_base(&self) -> String {
        if let Some(value) = std::env::var_os(ENDPOINT_ENV) {
            if let Some(value) = value.to_str() {
                return value.trim_end_matches('/').to_owned();
            }
        }
        self.endpoint_base
            .as_deref()
            .unwrap_or(DEFAULT_ENDPOINT_BASE)
            .trim_end_matches('/')
            .to_owned()
    }

    /// Installs a registered identity (secret token + public slug).
    pub fn adopt_identity(&mut self, token: String, feed_id: String) {
        self.token = Some(Zeroizing::new(token));
        self.feed_id = Some(feed_id);
    }

    /// Forgets the identity so the worker mints a fresh link on next use.
    pub fn forget_identity(&mut self) {
        self.token = None;
        self.feed_id = None;
    }

    /// The token to present: env override first so development runs never
    /// touch the Keychain.
    #[must_use]
    pub fn effective_token(&self) -> Option<Zeroizing<String>> {
        if let Some(value) = std::env::var_os(TOKEN_ENV) {
            if let Some(value) = value.to_str() {
                if !value.is_empty() {
                    return Some(Zeroizing::new(value.to_owned()));
                }
            }
        }
        self.token.clone()
    }
}

/// Counters shared between the uplink worker and the UI. All monotonic or
/// latching; the UI only ever reads.
#[derive(Default)]
pub struct UplinkStats {
    pub sent: AtomicU64,
    pub dropped: AtomicU64,
    /// Latched on a 401/403 until the app restarts or a new link is minted.
    pub auth_failed: AtomicBool,
    last_error: Mutex<Option<String>>,
    feed_url: Mutex<Option<String>>,
}

impl UplinkStats {
    pub fn note_sent(&self, count: u64) {
        self.sent.fetch_add(count, Ordering::Relaxed);
    }

    pub fn note_dropped(&self, count: u64) {
        self.dropped.fetch_add(count, Ordering::Relaxed);
    }

    pub fn set_last_error(&self, error: Option<String>) {
        if let Ok(mut slot) = self.last_error.lock() {
            *slot = error;
        }
    }

    #[must_use]
    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|slot| slot.clone())
    }

    pub fn set_feed_url(&self, url: Option<String>) {
        if let Ok(mut slot) = self.feed_url.lock() {
            *slot = url;
        }
    }

    /// The shareable live link, once registration has happened.
    #[must_use]
    pub fn feed_url(&self) -> Option<String> {
        self.feed_url.lock().ok().and_then(|slot| slot.clone())
    }
}

/// The feed registration, persisted as one JSON blob so the token, slug, and
/// link never drift apart. It lives in a private file — not the Keychain —
/// exactly like the Battle.net credential, so it never prompts for a password.
/// The token is only a feed write-credential, less sensitive than the account
/// credential that already sits in a file next to it.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredIdentity {
    pub token: String,
    pub feed_id: String,
    pub url: String,
}

mod identity_store {
    use std::{
        fs::{self, DirBuilder, OpenOptions},
        io::Write,
        path::PathBuf,
    };

    #[cfg(unix)]
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

    use super::{IDENTITY_FILENAME, StoredIdentity};
    use crate::{Error, Result};

    #[cfg(not(windows))]
    fn identity_path() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library/Application Support/Superiority")
                .join(IDENTITY_FILENAME)
        })
    }

    #[cfg(windows)]
    fn identity_path() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|app_data| {
            PathBuf::from(app_data)
                .join("Superiority")
                .join(IDENTITY_FILENAME)
        })
    }

    /// Loads the stored feed identity, if any. A missing or corrupt file reads
    /// as absent, so a bad write is always recoverable by re-registering.
    #[must_use]
    pub fn load_identity() -> Option<StoredIdentity> {
        let bytes = fs::read(identity_path()?).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn store_identity(identity: &StoredIdentity) -> Result<()> {
        let path = identity_path()
            .ok_or_else(|| Error::Transport("no home directory for the feed identity".into()))?;
        if let Some(directory) = path.parent() {
            let mut builder = DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            builder.mode(0o700);
            builder
                .create(directory)
                .map_err(|error| Error::Transport(format!("create feed identity dir: {error}")))?;
        }
        let encoded = serde_json::to_vec(identity)
            .map_err(|error| Error::Transport(format!("encode feed identity: {error}")))?;
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&path)
            .map_err(|error| Error::Transport(format!("write feed identity: {error}")))?;
        file.write_all(&encoded)
            .map_err(|error| Error::Transport(format!("write feed identity: {error}")))
    }

    pub fn clear_identity() -> Result<()> {
        let Some(path) = identity_path() else {
            return Ok(());
        };
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::Transport(format!("clear feed identity: {error}"))),
        }
    }
}

pub use identity_store::{clear_identity, load_identity, store_identity};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_base_strips_trailing_slashes() {
        let config = UplinkConfig {
            endpoint_base: Some("http://127.0.0.1:8787/".into()),
            ..UplinkConfig::default()
        };
        // the env override wins when present, so only assert the fallback
        // when the variable is absent from the test environment.
        if std::env::var_os(ENDPOINT_ENV).is_none() {
            assert_eq!(config.endpoint_base(), "http://127.0.0.1:8787");
        }
    }

    #[test]
    fn stats_latch_and_report() {
        let stats = UplinkStats::default();
        stats.note_sent(3);
        stats.note_dropped(2);
        stats.set_last_error(Some("boom".into()));
        stats.set_feed_url(Some("https://x/f/abc".into()));
        assert_eq!(stats.sent.load(Ordering::Relaxed), 3);
        assert_eq!(stats.dropped.load(Ordering::Relaxed), 2);
        assert_eq!(stats.last_error().as_deref(), Some("boom"));
        assert_eq!(stats.feed_url().as_deref(), Some("https://x/f/abc"));
    }

    #[test]
    fn stored_identity_round_trips_as_json() {
        let identity = StoredIdentity {
            token: "aa".into(),
            feed_id: "slug".into(),
            url: "https://x/f/slug".into(),
        };
        let encoded = serde_json::to_string(&identity).unwrap();
        let decoded: StoredIdentity = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.feed_id, "slug");
        assert_eq!(decoded.token, "aa");
    }
}
