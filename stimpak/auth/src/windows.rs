use std::{
    cell::RefCell,
    collections::HashMap,
    num::NonZeroIsize,
    rc::{Rc, Weak},
};

use url::Url;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            AdjustWindowRectEx, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW,
            DestroyWindow, GetSystemMetrics, IsWindow, LoadCursorW, RegisterClassExW, SM_CXSCREEN,
            SM_CYSCREEN, SW_HIDE, SW_SHOW, SetForegroundWindow, ShowWindow, WINDOW_EX_STYLE,
            WM_CLOSE, WM_NCDESTROY, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
        },
    },
    core::w,
};
use wry::{
    NewWindowResponse, WebView, WebViewBuilder, WebViewBuilderExtWindows,
    raw_window_handle::{
        HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
    },
};

use crate::{AuthOutcome, Completion, callback_token};

const WINDOW_WIDTH: i32 = 980;
const WINDOW_HEIGHT: i32 = 760;

type CompletionSlot = Rc<RefCell<Option<Completion>>>;
type WebViewSlot = Rc<RefCell<Option<Rc<WebView>>>>;

thread_local! {
    static WINDOW_COMPLETIONS: RefCell<HashMap<isize, CompletionSlot>> = RefCell::new(HashMap::new());
}

pub(crate) struct Authenticator {
    window: Option<HWND>,
    web_view: WebViewSlot,
    completion: CompletionSlot,
}

impl Authenticator {
    pub(crate) fn empty() -> Self {
        Self {
            window: None,
            web_view: Rc::new(RefCell::new(None)),
            completion: Rc::new(RefCell::new(None)),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn present(url: &Url, fresh_account: bool, completion: Completion) -> Self {
        let completion = Rc::new(RefCell::new(Some(completion)));
        let web_view = Rc::new(RefCell::new(None));
        let window = match create_window(Rc::clone(&completion)) {
            Ok(window) => window,
            Err(error) => {
                settle(&completion, AuthOutcome::Error(error));
                return Self {
                    window: None,
                    web_view,
                    completion,
                };
            }
        };

        let navigation = Rc::clone(&completion);
        let popup = Rc::clone(&completion);
        let popup_web_view = Rc::downgrade(&web_view);
        let initial = if fresh_account {
            "about:blank"
        } else {
            url.as_str()
        };
        let builder = WebViewBuilder::new()
            .with_url(initial)
            .with_profile_name("stimpak-battle-net")
            .with_devtools(false)
            .with_navigation_handler(move |location| {
                !complete_callback(&navigation, window, &location)
            })
            .with_new_window_req_handler(move |location, _features| {
                if complete_callback(&popup, window, &location) {
                    return NewWindowResponse::Deny;
                }
                if let Some(view) = current_web_view(&popup_web_view)
                    && let Err(error) = view.load_url(&location)
                {
                    settle(
                        &popup,
                        AuthOutcome::Error(format!("authentication page failed: {error}")),
                    );
                    hide_window(window);
                }
                NewWindowResponse::Deny
            });

        let native_window = NativeWindowHandle(window);
        match builder.build(&native_window) {
            Ok(view) => {
                if fresh_account {
                    let reset = view
                        .cookies()
                        .and_then(|cookies| {
                            for cookie in &cookies {
                                view.delete_cookie(cookie)?;
                            }
                            view.clear_all_browsing_data()
                        })
                        .and_then(|()| view.load_url(url.as_str()));
                    if let Err(error) = reset {
                        settle(
                            &completion,
                            AuthOutcome::Error(format!(
                                "could not reset the Battle.net session: {error}"
                            )),
                        );
                        destroy_window(window, &web_view);
                        return Self {
                            window: None,
                            web_view,
                            completion,
                        };
                    }
                }
                *web_view.borrow_mut() = Some(Rc::new(view));
                unsafe {
                    let _ = ShowWindow(window, SW_SHOW);
                    let _ = SetForegroundWindow(window);
                }
            }
            Err(error) => {
                settle(
                    &completion,
                    AuthOutcome::Error(format!("could not open the embedded webview: {error}")),
                );
                destroy_window(window, &web_view);
                return Self {
                    window: None,
                    web_view,
                    completion,
                };
            }
        }

        Self {
            window: Some(window),
            web_view,
            completion,
        }
    }

    pub(crate) fn cancel(&self) {
        settle(&self.completion, AuthOutcome::Cancelled);
        if let Some(window) = self.window {
            hide_window(window);
        }
    }
}

impl Drop for Authenticator {
    fn drop(&mut self) {
        self.cancel();
        if let Some(window) = self.window {
            destroy_window(window, &self.web_view);
        }
    }
}

fn create_window(completion: CompletionSlot) -> Result<HWND, String> {
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
        lpszClassName: w!("StimpakEmbeddedAuthenticator"),
        ..WNDCLASSEXW::default()
    };
    unsafe {
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
            w!("StimpakEmbeddedAuthenticator"),
            w!("Stimpak — Battle.net Authentication"),
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
    WINDOW_COMPLETIONS.with(|completions| {
        completions
            .borrow_mut()
            .insert(window_key(window), completion);
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
            WINDOW_COMPLETIONS.with(|completions| {
                if let Some(completion) = completions.borrow().get(&window_key(window)) {
                    settle(completion, AuthOutcome::Cancelled);
                }
            });
            hide_window(window);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            WINDOW_COMPLETIONS.with(|completions| {
                completions.borrow_mut().remove(&window_key(window));
            });
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn complete_callback(completion: &CompletionSlot, window: HWND, location: &str) -> bool {
    let Some(token) = callback_token(location) else {
        return false;
    };
    settle(completion, AuthOutcome::Token(token));
    hide_window(window);
    true
}

fn settle(completion: &CompletionSlot, outcome: AuthOutcome) {
    if let Some(completion) = completion.borrow_mut().take() {
        completion(outcome);
    }
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
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let Some(window) = NonZeroIsize::new(self.0.0 as isize) else {
            return Err(HandleError::Unavailable);
        };
        let handle = Win32WindowHandle::new(window);
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
    }
}
