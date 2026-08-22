use std::{
    cell::RefCell,
    collections::HashMap,
    num::NonZeroIsize,
    rc::{Rc, Weak},
    sync::mpsc::{self, Sender, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use superiority_core::{Error, Result, bgs::SecretBytes};
use url::Url;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            AdjustWindowRectEx, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW,
            DestroyWindow, DispatchMessageW, GetSystemMetrics, IsWindow, LoadCursorW, MSG,
            PM_REMOVE, PeekMessageW, RegisterClassExW, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW,
            SetForegroundWindow, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_CLOSE,
            WM_NCDESTROY, WM_QUIT, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
        },
    },
    core::w,
};
use wry::{
    NewWindowResponse, WebView, WebViewBuilder,
    raw_window_handle::{
        HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
    },
};

use super::{Cancellation, cancelled, parse_credential_redirect};

const WINDOW_WIDTH: i32 = 980;
const WINDOW_HEIGHT: i32 = 760;
const PUMP_INTERVAL: Duration = Duration::from_millis(10);

type ReplySlot = Rc<RefCell<Option<Sender<Result<SecretBytes>>>>>;
type WebViewSlot = Rc<RefCell<Option<Rc<WebView>>>>;

thread_local! {
    static WINDOW_REPLIES: RefCell<HashMap<isize, ReplySlot>> = RefCell::new(HashMap::new());
}

struct Authenticator {
    window: Option<HWND>,
    web_view: WebViewSlot,
    reply: ReplySlot,
}

impl Authenticator {
    fn present(authentication_url: &Url, reply: Sender<Result<SecretBytes>>) -> Self {
        let reply = Rc::new(RefCell::new(Some(reply)));
        let web_view = Rc::new(RefCell::new(None));
        let window = match create_window(Rc::clone(&reply)) {
            Ok(window) => window,
            Err(error) => {
                fail_reply(
                    &reply,
                    &format!("Battle.net authentication window failed: {error}"),
                );
                return Self {
                    window: None,
                    web_view,
                    reply,
                };
            }
        };

        let navigation_reply = Rc::clone(&reply);
        let popup_reply = Rc::clone(&reply);
        let popup_web_view = Rc::downgrade(&web_view);
        let builder = WebViewBuilder::new()
            .with_url(authentication_url.as_str())
            .with_devtools(false)
            .with_navigation_handler(move |location| {
                !complete_callback(&navigation_reply, window, &location)
            })
            .with_new_window_req_handler(move |location, _features| {
                if complete_callback(&popup_reply, window, &location) {
                    return NewWindowResponse::Deny;
                }

                if let Some(view) = current_web_view(&popup_web_view)
                    && let Err(error) = view.load_url(&location)
                {
                    fail_reply(
                        &popup_reply,
                        &format!("Battle.net authentication page failed: {error}"),
                    );
                    hide_window(window);
                }
                NewWindowResponse::Deny
            });

        let native_window = NativeWindowHandle(window);
        match builder.build(&native_window) {
            Ok(view) => {
                *web_view.borrow_mut() = Some(Rc::new(view));
                unsafe {
                    let _ = ShowWindow(window, SW_SHOW);
                    let _ = SetForegroundWindow(window);
                }
            }
            Err(error) => {
                fail_reply(
                    &reply,
                    &format!("Battle.net authentication window failed: {error}"),
                );
                destroy_window(window, &web_view);
                return Self {
                    window: None,
                    web_view,
                    reply,
                };
            }
        }

        Self {
            window: Some(window),
            web_view,
            reply,
        }
    }

    fn dismiss(&self) {
        cancel_reply(&self.reply);
        if let Some(window) = self.window {
            destroy_window(window, &self.web_view);
        }
    }
}

impl Drop for Authenticator {
    fn drop(&mut self) {
        self.dismiss();
    }
}

pub fn request_credential(
    url: &Url,
    timeout: Duration,
    cancellation: &Cancellation,
) -> Result<SecretBytes> {
    let (sender, receiver) = mpsc::channel();
    let authenticator = Authenticator::present(url, sender);
    let deadline = Instant::now() + timeout;

    let outcome = loop {
        if cancellation() {
            break Err(cancelled());
        }
        match receiver.try_recv() {
            Ok(outcome) => break outcome,
            Err(TryRecvError::Disconnected) => {
                break Err(Error::Authentication(
                    "Battle.net login window went away".into(),
                ));
            }
            Err(TryRecvError::Empty) => {}
        }
        if Instant::now() >= deadline {
            break Err(Error::Authentication(format!(
                "Battle.net web login timed out after {} seconds",
                timeout.as_secs()
            )));
        }
        if !pump_messages() {
            break Err(Error::Authentication(
                "Battle.net login window was asked to quit".into(),
            ));
        }
        thread::sleep(PUMP_INTERVAL);
    };

    authenticator.dismiss();
    outcome
}

fn pump_messages() -> bool {
    let mut message = MSG::default();
    while unsafe { PeekMessageW(&raw mut message, None, 0, 0, PM_REMOVE).as_bool() } {
        if message.message == WM_QUIT {
            return false;
        }
        unsafe {
            let _ = TranslateMessage(&raw const message);
            DispatchMessageW(&raw const message);
        }
    }
    true
}

fn create_window(reply: ReplySlot) -> std::result::Result<HWND, String> {
    let instance = unsafe { GetModuleHandleW(None) }
        .map_err(|error| error.to_string())?
        .into();
    let class = WNDCLASSEXW {
        cbSize: u32::try_from(std::mem::size_of::<WNDCLASSEXW>())
            .expect("WNDCLASSEXW size fits in u32"),
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(authentication_window_proc),
        hInstance: instance,
        hCursor: unsafe { LoadCursorW(None, windows::Win32::UI::WindowsAndMessaging::IDC_ARROW) }
            .unwrap_or_default(),
        lpszClassName: w!("SC2GatewayWebAuthenticator"),
        ..WNDCLASSEXW::default()
    };
    unsafe {
        // a zero result is harmless when an earlier login window in this process
        // already registered the fixed class name.
        RegisterClassExW(&raw const class);
    }

    let style = WS_OVERLAPPEDWINDOW;
    let ex_style = WINDOW_EX_STYLE::default();
    let mut bounds = RECT {
        left: 0,
        top: 0,
        right: WINDOW_WIDTH,
        bottom: WINDOW_HEIGHT,
    };
    unsafe { AdjustWindowRectEx(&raw mut bounds, style, false, ex_style) }
        .map_err(|error| error.to_string())?;
    let width = bounds.right - bounds.left;
    let height = bounds.bottom - bounds.top;
    let (screen_width, screen_height) =
        unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };

    let window = unsafe {
        CreateWindowExW(
            ex_style,
            w!("SC2GatewayWebAuthenticator"),
            w!("SC2 Gateway — Battle.net Authentication"),
            style,
            (screen_width - width).max(0) / 2,
            (screen_height - height).max(0) / 2,
            width,
            height,
            None,
            None,
            Some(instance),
            None,
        )
    }
    .map_err(|error| error.to_string())?;

    WINDOW_REPLIES.with(|replies| {
        replies.borrow_mut().insert(window_key(window), reply);
    });
    Ok(window)
}

unsafe extern "system" fn authentication_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CLOSE => {
            WINDOW_REPLIES.with(|replies| {
                if let Some(reply) = replies.borrow().get(&window_key(window)) {
                    cancel_reply(reply);
                }
            });
            hide_window(window);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            WINDOW_REPLIES.with(|replies| {
                replies.borrow_mut().remove(&window_key(window));
            });
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn complete_callback(reply: &ReplySlot, window: HWND, location: &str) -> bool {
    let Ok(credential) = parse_credential_redirect(location) else {
        return false;
    };
    let Some(reply) = reply.borrow_mut().take() else {
        return true;
    };
    let _ = reply.send(Ok(credential));
    hide_window(window);
    true
}

fn fail_reply(reply: &ReplySlot, message: &str) {
    let Some(reply) = reply.borrow_mut().take() else {
        return;
    };
    let _ = reply.send(Err(Error::Authentication(message.into())));
}

fn cancel_reply(reply: &ReplySlot) {
    fail_reply(reply, "Battle.net authentication was cancelled");
}

fn current_web_view(slot: &Weak<RefCell<Option<Rc<WebView>>>>) -> Option<Rc<WebView>> {
    slot.upgrade().and_then(|slot| slot.borrow().clone())
}

fn hide_window(window: HWND) {
    unsafe {
        let _ = ShowWindow(window, SW_HIDE);
    }
}

fn destroy_window(window: HWND, web_view: &WebViewSlot) {
    web_view.borrow_mut().take();
    if unsafe { IsWindow(Some(window)).as_bool() } {
        let _ = unsafe { DestroyWindow(window) };
    }
}

fn window_key(window: HWND) -> isize {
    window.0 as isize
}

struct NativeWindowHandle(HWND);

impl HasWindowHandle for NativeWindowHandle {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        let Some(window) = NonZeroIsize::new(self.0.0 as isize) else {
            return Err(HandleError::Unavailable);
        };
        let handle = Win32WindowHandle::new(window);
        // safety: the borrowed handle cannot outlive this wrapper, and the native
        // window remains alive throughout wry's synchronous construction.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
    }
}
