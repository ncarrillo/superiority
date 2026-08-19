use crate::{
    Result,
    bgs::{ChallengeHandler, Client, LogonSession, SecretBytes},
};

pub trait CredentialStore {
    fn load(&self) -> Result<Option<SecretBytes>>;
    fn store(&self, credential: &SecretBytes) -> Result<()>;
    fn delete(&self) -> Result<()>;
}

#[derive(Clone, Debug)]
pub struct AuthenticationOutcome {
    pub session: LogonSession,
    pub cache_hit: bool,
    pub browser_used: bool,
}

pub fn authenticate_cached(
    client: &mut Client,
    store: &(impl CredentialStore + ?Sized),
    browser: &mut impl ChallengeHandler,
    force_interactive: bool,
) -> Result<AuthenticationOutcome> {
    let cached = if force_interactive {
        None
    } else {
        store.load()?
    };
    let mut browser_used = false;
    let mut stale_deleted = false;
    let mut challenge = |url: &url::Url| {
        browser_used = true;
        if cached.is_some() && !stale_deleted {
            store.delete()?;
            stale_deleted = true;
        }
        browser.complete(url)
    };

    let session = client.authenticate(cached.as_ref().map(SecretBytes::expose), &mut challenge)?;
    let successor = match client.generate_web_credentials() {
        Ok(value) => value,
        Err(error) => {
            if cached.is_some() && !stale_deleted {
                store.delete()?;
            }
            return Err(error);
        }
    };
    store.store(&successor)?;
    Ok(AuthenticationOutcome {
        session,
        cache_hit: cached.is_some() && !browser_used,
        browser_used,
    })
}
