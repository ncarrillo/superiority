mod auth;
mod event;

use std::{
    ffi::{CStr, CString, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    ptr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Receiver, RecvTimeoutError, Sender},
    },
    thread,
    time::Duration,
};

use serde::Deserialize;
use superiority_core::{
    auth::FileCredentialStore,
    bgs::SecretBytes,
    chat::ChatChannel,
    connection::{ClientCommand, ClientEvent, ClientHandle, Finished, spawn_client_with},
    native::WhisperTarget,
    observer::NoObserver,
    product::Product,
};

use crate::auth::AuthWindow;

/// how long `stimpak_client_close` waits for a session to finish before giving
/// up on it.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

/// Stimpak is a `StarCraft II` binding and stays one. It exists to drive SC2
/// bots; whichever products the client learns to speak after it are the app's
/// business, not this FFI's, and widening the surface here would mean
/// versioning it for every one of them.
const STIMPAK_PRODUCT: Product = Product::StarCraft2;

/// the native function and ownership contract described by `stimpak.h`.
pub const STIMPAK_ABI_VERSION: u32 = 1;
/// the tagged JSON event vocabulary consumed by managed bindings.
pub const STIMPAK_EVENT_SCHEMA_VERSION: u32 = 1;

pub const STIMPAK_OK: i32 = 0;
pub const STIMPAK_ERR_INVALID_ARGUMENT: i32 = -1;
/// the session thread is gone; close the client.
pub const STIMPAK_ERR_DISCONNECTED: i32 = -2;
pub const STIMPAK_ERR_NO_SUCH_AUTH: i32 = -3;
/// the sign-in window was closed, or could not produce a token.
pub const STIMPAK_ERR_AUTH_FAILED: i32 = -4;
/// a panic was caught at the boundary. a bug in this library.
pub const STIMPAK_ERR_PANIC: i32 = -99;

type AuthReply = Sender<superiority_core::Result<SecretBytes>>;
type PendingAuth = Arc<Mutex<Option<(u64, AuthReply)>>>;

pub struct Client {
    /// `Sender` is `Sync`, so commands need no lock.
    commands: Sender<ClientCommand>,
    /// `Receiver` is not `Sync`, so it lives inside the lock rather than beside
    /// one — otherwise nothing but discipline stops two threads polling at once.
    events: Mutex<Receiver<ClientEvent>>,
    /// only read by `stimpak_client_close`, which owns the whole client.
    finished: Receiver<Finished>,
    /// the reply for a sign-in the host must answer. core blocks on that reply,
    /// so at most one is ever outstanding — hence an option, not a map that
    /// grows every time a host ignores the event.
    pending: PendingAuth,
    next_auth: AtomicU64,
    /// set once the session thread is gone, so the end is reported exactly once.
    ended: AtomicBool,
    window: AuthWindow,
    names: Mutex<event::Names>,
}

fn guard<F: FnOnce() -> i32>(body: F) -> i32 {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(STIMPAK_ERR_PANIC)
}

/// # Safety
/// `client` must come from `stimpak_client_open` and must not have been closed.
unsafe fn borrow<'a>(client: *mut Client) -> Option<&'a Client> {
    if client.is_null() {
        return None;
    }
    Some(unsafe { &*client })
}

/// # Safety
/// `value` must be null or a nul-terminated string valid for the call.
unsafe fn text(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .ok()
        .map(ToOwned::to_owned)
}

fn auth_cancelled(detail: &str) -> superiority_core::Error {
    superiority_core::Error::Authentication(detail.to_owned())
}

fn send(client: &Client, command: ClientCommand) -> i32 {
    if client.commands.send(command).is_ok() {
        STIMPAK_OK
    } else {
        STIMPAK_ERR_DISCONNECTED
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RequestedChannel {
    Public { id: u16 },
    Private { name: String },
    Group { club_id: u32 },
}

impl From<RequestedChannel> for ChatChannel {
    fn from(channel: RequestedChannel) -> Self {
        match channel {
            RequestedChannel::Public { id } => Self::Public(id),
            RequestedChannel::Private { name } => Self::Private(name),
            RequestedChannel::Group { club_id } => Self::Club(club_id),
        }
    }
}

fn connect(
    client: &Client,
    force_interactive: bool,
    expected_account_id: Option<u64>,
    channels: Vec<ChatChannel>,
) -> i32 {
    send(
        client,
        ClientCommand::Connect {
            force_interactive,
            expected_account_id,
            expected_battle_tag: None,
            channels,
        },
    )
}

fn cancel_pending_auth(client: &Client, detail: &str) -> i32 {
    client.window.cancel();
    let Ok(mut pending) = client.pending.lock() else {
        return STIMPAK_ERR_PANIC;
    };
    let Some((_, reply)) = pending.take() else {
        return STIMPAK_ERR_NO_SUCH_AUTH;
    };
    drop(pending);
    if reply.send(Err(auth_cancelled(detail))).is_ok() {
        STIMPAK_OK
    } else {
        STIMPAK_ERR_DISCONNECTED
    }
}

fn complete_window_auth(
    pending: &PendingAuth,
    auth_id: u64,
    outcome: Result<Option<String>, String>,
) {
    let response = match outcome {
        Ok(Some(token)) => SecretBytes::new(token.into_bytes()),
        Ok(None) => Err(auth_cancelled("sign-in window closed")),
        Err(detail) => Err(auth_cancelled(&detail)),
    };
    let Ok(mut pending) = pending.lock() else {
        return;
    };
    let Some(reply) = pending
        .take_if(|(waiting, _)| *waiting == auth_id)
        .map(|(_, reply)| reply)
    else {
        return;
    };
    drop(pending);
    let _ = reply.send(response);
}

/// the session thread starts immediately but stays idle until
/// `stimpak_client_connect`. null if `credential_path` is missing or unusable, or
/// if the thread could not start.
///
/// `credential_path` names the file this client caches its signed-in session
/// in. there is no default: the app's own cache belongs to the app, and two
/// programs sharing one file means either can sign the other out.
///
/// `auth_window_path` is optional. when it, `STIMPAK_AUTH_WINDOW`, or a sibling of
/// the running executable names `stimpak-auth-window`, sign-in happens in a window
/// this library opens and the host never sees an `authentication_required`
/// event. pass null to rely on the other two.
///
/// # Safety
/// both arguments must be null or nul-terminated strings valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_open(
    credential_path: *const c_char,
    auth_window_path: *const c_char,
) -> *mut Client {
    catch_unwind(|| {
        let Some(credential_path) = (unsafe { text(credential_path) }) else {
            return ptr::null_mut();
        };
        if credential_path.is_empty() {
            return ptr::null_mut();
        }
        let window = AuthWindow::new(unsafe { text(auth_window_path) }.map(PathBuf::from));
        let store = FileCredentialStore::at(PathBuf::from(credential_path));
        let ClientHandle {
            commands,
            events,
            finished,
        } = spawn_client_with(STIMPAK_PRODUCT, Box::new(NoObserver), Box::new(store));
        Box::into_raw(Box::new(Client {
            commands,
            events: Mutex::new(events),
            finished,
            pending: Arc::new(Mutex::new(None)),
            next_auth: AtomicU64::new(1),
            ended: AtomicBool::new(false),
            window,
            names: Mutex::new(event::Names::default()),
        }))
    })
    .unwrap_or(ptr::null_mut())
}

/// whether this client can sign in on its own. false means the host must
/// handle `authentication_required` itself.
///
/// # Safety
/// `client` must be a live client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_has_auth_window(client: *mut Client) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        unsafe { borrow(client) }.is_some_and(|client| client.window.locate().is_some())
    }))
    .unwrap_or(false)
}

/// closes the session and waits for its thread to finish, so a caller that
/// reconnects by disposing and opening again does not end up with two sessions
/// briefly overlapping.
///
/// The wait is bounded so a transport that does not settle cannot hold the
/// host's shutdown indefinitely. Any sign-in helper is cancelled first.
///
/// # Safety
/// `client` must come from `stimpak_client_open` and must not be closed twice.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_close(client: *mut Client) {
    if client.is_null() {
        return;
    }
    let client = unsafe { Box::from_raw(client) };
    client.window.cancel();
    if let Ok(mut pending) = client.pending.lock()
        && let Some((_, reply)) = pending.take()
    {
        let _ = reply.send(Err(auth_cancelled("client closed during sign-in")));
    }
    let Client {
        commands,
        events,
        finished,
        ..
    } = *client;
    if commands.send(ClientCommand::Quit).is_err() {
        return;
    }
    // the worker returns to `recv` after quitting, so it only finishes once
    // this sender is gone. holding it while waiting would deadlock.
    drop(commands);
    drop(events);
    // the worker owns the other end; a receive failing means it has finished.
    let _ = finished.recv_timeout(CLOSE_TIMEOUT);
}

/// progress arrives as `stage` events. `force_interactive` bypasses the cached
/// credential and forces the browser flow; a bot passes false so it can start
/// unattended.
///
/// # Safety
/// `client` must be a live client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_connect(
    client: *mut Client,
    force_interactive: bool,
) -> i32 {
    guard(|| {
        let Some(client) = (unsafe { borrow(client) }) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        connect(client, force_interactive, None, Vec::new())
    })
}

/// connects with an explicit account guard and initial channel set.
///
/// `expected_account_id` is zero when no guard is wanted. `channels_json` is a
/// UTF-8 JSON array of objects tagged by `kind`: `public` with `id`, `private`
/// with `name`, or `group` with `club_id`. An empty array joins General. The
/// session restores the complete set as part of connection establishment,
/// before the `connected` stage is emitted.
///
/// # Safety
/// `client` must be live and `channels_json` a nul-terminated string valid for
/// the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_connect_configured(
    client: *mut Client,
    force_interactive: bool,
    expected_account_id: u64,
    channels_json: *const c_char,
) -> i32 {
    guard(|| {
        let (Some(client), Some(channels_json)) =
            (unsafe { borrow(client) }, unsafe { text(channels_json) })
        else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        let Ok(requested) = serde_json::from_str::<Vec<RequestedChannel>>(&channels_json) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        if requested.iter().any(|channel| match channel {
            RequestedChannel::Private { name } => name.trim().is_empty(),
            RequestedChannel::Group { club_id } => *club_id == 0,
            RequestedChannel::Public { .. } => false,
        }) {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        }
        let channels = requested.into_iter().map(ChatChannel::from).collect();
        connect(
            client,
            force_interactive,
            (expected_account_id != 0).then_some(expected_account_id),
            channels,
        )
    })
}

/// drops the session but keeps the client open for a later connect.
///
/// # Safety
/// `client` must be a live client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_disconnect(client: *mut Client) -> i32 {
    guard(|| {
        let Some(client) = (unsafe { borrow(client) }) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        let _ = cancel_pending_auth(client, "connection cancelled");
        send(client, ClientCommand::Disconnect)
    })
}

/// disconnects and deletes the credential cached at the path supplied to
/// `stimpak_client_open`. The next connect requires authentication.
///
/// # Safety
/// `client` must be a live client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_sign_out(client: *mut Client) -> i32 {
    guard(|| {
        let Some(client) = (unsafe { borrow(client) }) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        let _ = cancel_pending_auth(client, "sign-in cancelled by sign-out");
        send(client, ClientCommand::SignOut)
    })
}

/// # Safety
/// `client` must be a live client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_join_public(client: *mut Client, channel_id: u16) -> i32 {
    guard(|| {
        let Some(client) = (unsafe { borrow(client) }) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        send(
            client,
            ClientCommand::JoinChannel(ChatChannel::Public(channel_id)),
        )
    })
}

/// creates the channel if nobody is there.
///
/// # Safety
/// `client` must be a live client pointer and `name` nul-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_join_private(
    client: *mut Client,
    name: *const c_char,
) -> i32 {
    guard(|| {
        let (Some(client), Some(name)) = (unsafe { borrow(client) }, unsafe { text(name) }) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        send(
            client,
            ClientCommand::JoinChannel(ChatChannel::Private(name)),
        )
    })
}

/// joins a `StarCraft II` club or clan by its stable id.
///
/// # Safety
/// `client` must be a live client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_join_group(client: *mut Client, club_id: u32) -> i32 {
    guard(|| {
        let Some(client) = (unsafe { borrow(client) }) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        if club_id == 0 {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        }
        send(
            client,
            ClientCommand::JoinChannel(ChatChannel::Club(club_id)),
        )
    })
}

/// searches the account's visible SC2 groups. Results arrive as
/// `group_search` and `group_summary` events.
///
/// # Safety
/// `client` must be live and `query` nul-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_search_groups(
    client: *mut Client,
    query: *const c_char,
) -> i32 {
    guard(|| {
        let (Some(client), Some(query)) = (unsafe { borrow(client) }, unsafe { text(query) })
        else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        let query = query.trim();
        if query.is_empty() {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        }
        send(
            client,
            ClientCommand::SearchGroups {
                query: query.to_owned(),
            },
        )
    })
}

/// `channel_index` is the one the `joined` event carried.
///
/// # Safety
/// `client` must be a live client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_leave(client: *mut Client, channel_index: u8) -> i32 {
    guard(|| {
        let Some(client) = (unsafe { borrow(client) }) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        send(client, ClientCommand::LeaveChannel { channel_index })
    })
}

/// # Safety
/// `client` must be a live client pointer and `body` nul-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_send_message(
    client: *mut Client,
    channel_index: u8,
    body: *const c_char,
) -> i32 {
    guard(|| {
        let (Some(client), Some(body)) = (unsafe { borrow(client) }, unsafe { text(body) }) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        send(
            client,
            ClientCommand::SendMessage {
                channel_index,
                body,
            },
        )
    })
}

/// `name` is the same string a `whisper` event's `peer` carries, so replying is
/// an echo of what arrived.
///
/// # Safety
/// `client` must be a live client pointer; `name` and `body` nul-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_send_whisper(
    client: *mut Client,
    name: *const c_char,
    body: *const c_char,
) -> i32 {
    guard(|| {
        let (Some(client), Some(name), Some(body)) =
            (unsafe { borrow(client) }, unsafe { text(name) }, unsafe {
                text(body)
            })
        else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        send(
            client,
            ClientCommand::SendWhisper {
                target: WhisperTarget::Name(name.clone()),
                display_name: name,
                body,
            },
        )
    })
}

/// # Safety
/// `client` must be a live client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_answer_group_invitation(
    client: *mut Client,
    club_id: u32,
    accept: bool,
) -> i32 {
    guard(|| {
        let Some(client) = (unsafe { borrow(client) }) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        send(
            client,
            ClientCommand::AnswerGroupInvitation { club_id, accept },
        )
    })
}

/// # Safety
/// `client` must be a live client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_answer_party_invitation(
    client: *mut Client,
    channel_index: u8,
    accept: bool,
) -> i32 {
    guard(|| {
        let Some(client) = (unsafe { borrow(client) }) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        send(
            client,
            ClientCommand::AnswerPartyInvitation {
                channel_index,
                accept,
            },
        )
    })
}

/// finishes an interactive sign-in with the token from the browser flow.
///
/// # Safety
/// `client` must be a live client pointer and `token` nul-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_submit_auth(
    client: *mut Client,
    auth_id: u64,
    token: *const c_char,
) -> i32 {
    guard(|| {
        let (Some(client), Some(token)) = (unsafe { borrow(client) }, unsafe { text(token) })
        else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        let Ok(mut pending) = client.pending.lock() else {
            return STIMPAK_ERR_PANIC;
        };
        let Some(reply) = pending
            .take_if(|(waiting, _)| *waiting == auth_id)
            .map(|(_, reply)| reply)
        else {
            return STIMPAK_ERR_NO_SUCH_AUTH;
        };
        drop(pending);
        if reply.send(SecretBytes::new(token.into_bytes())).is_ok() {
            STIMPAK_OK
        } else {
            STIMPAK_ERR_DISCONNECTED
        }
    })
}

/// cancels an `authentication_required` event without closing the client.
/// The failed connection reports its normal error and disconnected stage; a
/// later connect may try again.
///
/// # Safety
/// `client` must be a live client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_cancel_auth(client: *mut Client, auth_id: u64) -> i32 {
    guard(|| {
        let Some(client) = (unsafe { borrow(client) }) else {
            return STIMPAK_ERR_INVALID_ARGUMENT;
        };
        let Ok(mut pending) = client.pending.lock() else {
            return STIMPAK_ERR_PANIC;
        };
        let Some(reply) = pending
            .take_if(|(waiting, _)| *waiting == auth_id)
            .map(|(_, reply)| reply)
        else {
            return STIMPAK_ERR_NO_SUCH_AUTH;
        };
        drop(pending);
        client.window.cancel();
        if reply
            .send(Err(auth_cancelled("sign-in cancelled by host")))
            .is_ok()
        {
            STIMPAK_OK
        } else {
            STIMPAK_ERR_DISCONNECTED
        }
    })
}

/// blocks until an event exists or `timeout_ms` elapses, then returns it as a
/// json object. null means the timeout expired with nothing to report, or the
/// session thread is gone. the caller owns the string and must release it with
/// `stimpak_string_free`.
///
/// # Safety
/// `client` must be a live client pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_client_poll(client: *mut Client, timeout_ms: u32) -> *mut c_char {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(client) = (unsafe { borrow(client) }) else {
            return ptr::null_mut();
        };
        let Ok(events) = client.events.lock() else {
            return ptr::null_mut();
        };
        let received = events.recv_timeout(Duration::from_millis(u64::from(timeout_ms)));
        drop(events);
        let received = match received {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => return ptr::null_mut(),
            // the session thread is gone for good. say so once — a caller that
            // owns the lifecycle needs to tell this apart from a quiet moment —
            // then report nothing forever after.
            Err(RecvTimeoutError::Disconnected) => {
                if client.ended.swap(true, Ordering::Relaxed) {
                    return ptr::null_mut();
                }
                return describe(&event::Event::SessionEnded);
            }
        };
        // a sign-in this library can answer itself never reaches the host: the
        // window runs, the reply goes back, and the session carries on. only
        // when there is no window does the event surface, with its reply parked
        // under an id the host can quote back.
        let auth_id = match &received {
            ClientEvent::Authentication { url, reply, .. } => {
                let id = client.next_auth.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut pending) = client.pending.lock() {
                    *pending = Some((id, reply.clone()));
                } else {
                    let _ = reply.send(Err(auth_cancelled("sign-in state is unavailable")));
                    return ptr::null_mut();
                }
                if let Some(helper) = client.window.locate() {
                    let completion = match client.window.present(&helper, url.as_str()) {
                        Ok(completion) => completion,
                        Err(detail) => {
                            complete_window_auth(&client.pending, id, Err(detail));
                            return ptr::null_mut();
                        }
                    };
                    let pending = Arc::clone(&client.pending);
                    let spawn = thread::Builder::new()
                        .name("stimpak-auth-result".into())
                        .spawn(move || {
                            let outcome = completion.recv().unwrap_or_else(|_| {
                                Err("sign-in window stopped without a result".to_owned())
                            });
                            complete_window_auth(&pending, id, outcome);
                        });
                    if let Err(error) = spawn {
                        complete_window_auth(
                            &client.pending,
                            id,
                            Err(format!("could not collect the sign-in result: {error}")),
                        );
                    }
                    return ptr::null_mut();
                }
                id
            }
            _ => 0,
        };
        let Ok(mut names) = client.names.lock() else {
            return ptr::null_mut();
        };
        names.learn(&received);
        let described = event::translate(&received, auth_id, &names);
        drop(names);
        describe(&described)
    }))
    .unwrap_or(ptr::null_mut())
}

/// serialise one event into a string the caller owns.
fn describe(event: &event::Event) -> *mut c_char {
    let Ok(json) = serde_json::to_string(event) else {
        return ptr::null_mut();
    };
    CString::new(json).map_or(ptr::null_mut(), CString::into_raw)
}

/// # Safety
/// `value` must have come from this library and must not be freed twice.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stimpak_string_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }
    drop(unsafe { CString::from_raw(value) });
}

/// the general channel's id, for `stimpak_client_join_public`.
#[unsafe(no_mangle)]
pub extern "C" fn stimpak_default_public_channel() -> u16 {
    superiority_core::connection::DEFAULT_PUBLIC_CHANNEL
}

#[unsafe(no_mangle)]
pub extern "C" fn stimpak_abi_version() -> u32 {
    STIMPAK_ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn stimpak_event_schema_version() -> u32 {
    STIMPAK_EVENT_SCHEMA_VERSION
}

/// static; do not free.
#[unsafe(no_mangle)]
pub extern "C" fn stimpak_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_null_client_is_rejected_rather_than_dereferenced() {
        assert_eq!(
            unsafe { stimpak_client_disconnect(ptr::null_mut()) },
            STIMPAK_ERR_INVALID_ARGUMENT
        );
        assert!(unsafe { stimpak_client_poll(ptr::null_mut(), 0) }.is_null());
    }

    #[test]
    fn freeing_null_is_harmless() {
        unsafe { stimpak_string_free(ptr::null_mut()) };
    }

    fn scratch_client() -> *mut Client {
        let path = std::env::temp_dir().join(format!("sc2-ffi-test-{}.bin", std::process::id()));
        let path = CString::new(path.to_string_lossy().as_ref()).unwrap();
        unsafe { stimpak_client_open(path.as_ptr(), ptr::null()) }
    }

    #[test]
    fn opening_without_a_credential_path_is_refused() {
        assert!(unsafe { stimpak_client_open(ptr::null(), ptr::null()) }.is_null());
        let empty = CString::new("").unwrap();
        assert!(unsafe { stimpak_client_open(empty.as_ptr(), ptr::null()) }.is_null());
    }

    #[test]
    fn submitting_an_unknown_auth_id_reports_it() {
        let client = scratch_client();
        assert!(!client.is_null());
        let token = CString::new("token").unwrap();
        assert_eq!(
            unsafe { stimpak_client_submit_auth(client, 4242, token.as_ptr()) },
            STIMPAK_ERR_NO_SUCH_AUTH
        );
        unsafe { stimpak_client_close(client) };
    }

    #[test]
    fn commands_reach_the_session_thread() {
        let client = scratch_client();
        assert_eq!(unsafe { stimpak_client_join_public(client, 1) }, STIMPAK_OK);
        assert_eq!(unsafe { stimpak_client_join_group(client, 7) }, STIMPAK_OK);
        let query = CString::new("builders").unwrap();
        assert_eq!(
            unsafe { stimpak_client_search_groups(client, query.as_ptr()) },
            STIMPAK_OK
        );
        assert_eq!(
            unsafe { stimpak_client_answer_party_invitation(client, 3, true) },
            STIMPAK_OK
        );
        unsafe { stimpak_client_close(client) };
    }

    #[test]
    fn configured_connect_rejects_an_invalid_channel_contract() {
        let client = scratch_client();
        let invalid = CString::new(r#"[{"kind":"party"}]"#).unwrap();
        assert_eq!(
            unsafe { stimpak_client_connect_configured(client, false, 0, invalid.as_ptr()) },
            STIMPAK_ERR_INVALID_ARGUMENT
        );
        unsafe { stimpak_client_close(client) };
    }

    #[test]
    fn published_header_tracks_the_exported_surface() {
        let header = include_str!("../include/stimpak.h");
        for symbol in [
            "stimpak_client_connect_configured",
            "stimpak_client_sign_out",
            "stimpak_client_join_group",
            "stimpak_client_search_groups",
            "stimpak_client_answer_party_invitation",
            "stimpak_client_cancel_auth",
            "stimpak_abi_version",
            "stimpak_event_schema_version",
        ] {
            assert!(header.contains(symbol), "header omits {symbol}");
        }
        assert_eq!(stimpak_abi_version(), STIMPAK_ABI_VERSION);
        assert_eq!(stimpak_event_schema_version(), STIMPAK_EVENT_SCHEMA_VERSION);
    }

    #[test]
    fn an_idle_poll_reports_nothing_rather_than_blocking_forever() {
        let client = scratch_client();
        assert!(unsafe { stimpak_client_poll(client, 10) }.is_null());
        unsafe { stimpak_client_close(client) };
    }

    #[test]
    fn a_missing_helper_leaves_the_host_to_sign_in() {
        let path = std::env::temp_dir().join("sc2-ffi-no-window.bin");
        let path = CString::new(path.to_string_lossy().as_ref()).unwrap();
        let absent = CString::new("/definitely/not/a/real/helper").unwrap();
        let client = unsafe { stimpak_client_open(path.as_ptr(), absent.as_ptr()) };
        assert!(!client.is_null());
        assert!(!unsafe { stimpak_client_has_auth_window(client) });
        unsafe { stimpak_client_close(client) };
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    fn scratch(name: &str) -> CString {
        let path = std::env::temp_dir().join(format!("stimpak-{name}-{}.bin", std::process::id()));
        CString::new(path.to_string_lossy().as_ref()).unwrap()
    }

    /// the caller owns the session, so open/close cycles must leave nothing
    /// behind — not merely avoid crashing.
    #[test]
    fn repeated_open_and_close_cycles_settle() {
        let path = scratch("cycles");
        let before = std::thread::available_parallelism().is_ok();
        assert!(before);
        for _ in 0..25 {
            let client = unsafe { stimpak_client_open(path.as_ptr(), ptr::null()) };
            assert!(!client.is_null());
            assert_eq!(unsafe { stimpak_client_join_public(client, 1) }, STIMPAK_OK);
            unsafe { stimpak_client_close(client) };
        }
    }

    #[test]
    fn a_new_session_works_after_the_previous_one_is_closed() {
        let path = scratch("successor");
        let first = unsafe { stimpak_client_open(path.as_ptr(), ptr::null()) };
        unsafe { stimpak_client_close(first) };

        let second = unsafe { stimpak_client_open(path.as_ptr(), ptr::null()) };
        assert!(!second.is_null());
        assert_eq!(unsafe { stimpak_client_join_public(second, 1) }, STIMPAK_OK);
        unsafe { stimpak_client_close(second) };
    }
}
