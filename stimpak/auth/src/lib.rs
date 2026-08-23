//! Optional in-process Battle.net authentication UI for Stimpak.
//!
//! Presentation must begin on the host UI thread. The native window returns
//! immediately and is then driven by the `AppKit` or `Win32` loop the host already
//! owns. Results cross the FFI callback as borrowed UTF-8 valid only for the
//! duration of that callback.

use std::{
    ffi::{CStr, CString, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};

use url::Url;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as platform;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported {
    use url::Url;

    use crate::{AuthOutcome, Completion};

    pub(crate) struct Authenticator;

    impl Authenticator {
        pub(crate) fn empty() -> Self {
            Self
        }

        pub(crate) fn present(_url: &Url, _fresh_account: bool, completion: Completion) -> Self {
            completion(AuthOutcome::Error(
                "embedded authentication is supported only on macOS and Windows".into(),
            ));
            Self
        }

        pub(crate) fn cancel(&self) {}
    }
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use unsupported as platform;

pub const STIMPAK_AUTH_ABI_VERSION: u32 = 1;

pub(crate) enum AuthOutcome {
    Token(String),
    Cancelled,
    Error(String),
}

pub(crate) type Completion = Box<dyn FnOnce(AuthOutcome)>;

type AuthCallback = unsafe extern "C" fn(*mut c_void, i32, *const c_char);

pub struct AuthSession {
    authenticator: platform::Authenticator,
}

unsafe fn text(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .ok()
        .map(ToOwned::to_owned)
}

fn callback_token(location: &str) -> Option<String> {
    let url = Url::parse(location).ok()?;
    if url.scheme() != "http" || url.host_str() != Some("localhost") || url.port() != Some(0) {
        return None;
    }
    let token = url
        .query_pairs()
        .find_map(|(name, value)| (name == "ST").then(|| value.into_owned()))?;
    let bytes = token.as_bytes();
    if bytes.is_empty() || bytes.len() > 1024 || !bytes.iter().all(u8::is_ascii_graphic) {
        return None;
    }
    Some(token)
}

fn invoke(callback: AuthCallback, context: *mut c_void, outcome: AuthOutcome) {
    let (status, detail) = match outcome {
        AuthOutcome::Token(token) => (0, Some(token)),
        AuthOutcome::Cancelled => (1, None),
        AuthOutcome::Error(error) => (2, Some(error)),
    };
    let detail = detail.map(|value| {
        CString::new(value).unwrap_or_else(|_| CString::new("authentication failed").unwrap())
    });
    unsafe {
        callback(
            context,
            status,
            detail.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
        );
    }
}

/// Starts an in-process authentication window and returns immediately.
///
/// Must be called on the host UI thread. `callback` is invoked exactly once on
/// that thread with status 0/token, 1/cancelled, or 2/error. `fresh_account`
/// clears this provider's persistent browser data before navigation.
///
/// # Safety
/// `url` must be nul-terminated and `callback`/`context` remain valid until the
/// callback fires or the returned session is closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_auth_present(
    url: *const c_char,
    fresh_account: bool,
    callback: Option<AuthCallback>,
    context: *mut c_void,
) -> *mut AuthSession {
    catch_unwind(AssertUnwindSafe(|| {
        let (Some(url), Some(callback)) = (unsafe { text(url) }, callback) else {
            return ptr::null_mut();
        };
        let completion: Completion = Box::new(move |outcome| invoke(callback, context, outcome));
        let authenticator = match Url::parse(&url) {
            Ok(url) => platform::Authenticator::present(&url, fresh_account, completion),
            Err(error) => {
                completion(AuthOutcome::Error(format!(
                    "invalid authentication URL: {error}"
                )));
                platform::Authenticator::empty()
            }
        };
        Box::into_raw(Box::new(AuthSession { authenticator }))
    }))
    .unwrap_or(ptr::null_mut())
}

/// Cancels an outstanding window. Call on the same UI thread as `present`.
///
/// # Safety
/// `session` must be a live pointer returned by `stimpak_auth_present`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_auth_cancel(session: *mut AuthSession) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(session) = unsafe { session.as_ref() } {
            session.authenticator.cancel();
        }
    }));
}

/// Releases the window after completion or cancellation. Call on the same UI
/// thread as `present`, exactly once.
///
/// # Safety
/// `session` must be null or a live pointer returned by `stimpak_auth_present`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_auth_close(session: *mut AuthSession) {
    if session.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(session) })));
}

#[unsafe(no_mangle)]
pub extern "C" fn stimpak_auth_abi_version() -> u32 {
    STIMPAK_AUTH_ABI_VERSION
}

/// Static UTF-8 storage owned by the library.
#[unsafe(no_mangle)]
pub extern "C" fn stimpak_auth_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_only_the_battle_net_port_zero_callback() {
        assert_eq!(
            callback_token("http://localhost:0/?ST=US-test-token").as_deref(),
            Some("US-test-token")
        );
        assert!(callback_token("https://localhost:0/?ST=US-test-token").is_none());
        assert!(callback_token("http://localhost:8080/?ST=US-test-token").is_none());
        assert!(callback_token("http://example.com:0/?ST=US-test-token").is_none());
        assert!(callback_token("http://localhost:0/?ST=").is_none());
    }
}
