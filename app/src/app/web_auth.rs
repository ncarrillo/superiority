#[cfg(target_os = "macos")]
mod macos {
    use super::callback_credential;
    use std::{
        cell::{OnceCell, RefCell},
        ptr::NonNull,
        rc::Rc,
        sync::mpsc::Sender,
    };

    use objc2::runtime::ProtocolObject;
    use objc2::{
        DefinedClass, MainThreadOnly, define_class, msg_send,
        rc::{Retained, Weak},
        sel,
    };
    use objc2_app_kit::{
        NSAutoresizingMaskOptions, NSBackingStoreType, NSView, NSWindow, NSWindowDelegate,
        NSWindowStyleMask,
    };
    use objc2_foundation::{
        MainThreadMarker, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize,
        NSString, NSTimer,
    };
    use url::Url;
    use wry::raw_window_handle::{
        AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
    };
    use wry::{
        NewWindowResponse, Rect, WebView, WebViewBuilder, WebViewBuilderExtDarwin, WebViewExtMacOS,
        dpi::{LogicalPosition, LogicalSize},
    };

    use crate::{Error, Result};
    use superiority_core::{Product, bgs::SecretBytes};

    // every product mints a different credential, but all authorizations must
    // inherit one Battle.net SSO identity. A single persistent WKWebView store
    // is the browser side of that invariant.
    const AUTHENTICATION_STORE_IDENTIFIER: [u8; 16] = [
        0x7A, 0x48, 0xAF, 0x1D, 0x4B, 0x08, 0x4F, 0x37, 0x9F, 0x4F, 0x5D, 0x6E, 0x81, 0x3F, 0xE2,
        0xA4,
    ];
    /// give the shared Battle.net SSO store time to complete a redirect before
    /// presenting chrome. A real login still appears; an automatic product
    /// authorization never flashes a window for a page nobody has to read.
    const SILENT_SSO_GRACE_SECONDS: f64 = 1.0;

    type WebViewSlot = Rc<RefCell<Option<Rc<WebView>>>>;

    #[derive(Default)]
    pub(crate) struct WebAuthenticatorIvars {
        window: OnceCell<Retained<NSWindow>>,
        web_view: WebViewSlot,
        reply: RefCell<Option<Sender<Result<SecretBytes>>>>,
    }

    define_class!(
        #[unsafe(super = NSObject)]
        #[thread_kind = MainThreadOnly]
        #[ivars = WebAuthenticatorIvars]
        pub(crate) struct WebAuthenticator;

        unsafe impl NSObjectProtocol for WebAuthenticator {}

        impl WebAuthenticator {
            #[unsafe(method(showIfAuthenticationIsPending:))]
            fn show_if_authentication_is_pending(&self, _timer: &NSTimer) {
                if self.ivars().reply.borrow().is_some()
                    && let Some(window) = self.ivars().window.get()
                {
                    window.makeKeyAndOrderFront(None);
                }
            }
        }

        unsafe impl NSWindowDelegate for WebAuthenticator {
            #[unsafe(method(windowWillClose:))]
            fn window_will_close(&self, _notification: &NSNotification) {
                self.cancel();
            }
        }
    );

    impl WebAuthenticator {
        pub(crate) fn present(
            authentication_url: &Url,
            reply: Sender<Result<SecretBytes>>,
            _product: Product,
            fresh_account: bool,
        ) -> Retained<Self> {
            let mtm = MainThreadMarker::new().expect("the application must run on the main thread");
            let this = Self::alloc(mtm).set_ivars(WebAuthenticatorIvars {
                reply: RefCell::new(Some(reply)),
                ..WebAuthenticatorIvars::default()
            });
            let this: Retained<Self> = unsafe { msg_send![super(this), init] };

            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(980.0, 760.0));
            let content = NSView::initWithFrame(NSView::alloc(mtm), frame);
            content.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );

            let window = unsafe {
                NSWindow::initWithContentRect_styleMask_backing_defer(
                    NSWindow::alloc(mtm),
                    frame,
                    NSWindowStyleMask::Titled
                        | NSWindowStyleMask::Closable
                        | NSWindowStyleMask::Miniaturizable
                        | NSWindowStyleMask::Resizable,
                    NSBackingStoreType::Buffered,
                    false,
                )
            };
            unsafe { window.setReleasedWhenClosed(false) };
            window.setTitle(&NSString::from_str(
                "Superiority — Battle.net Authentication",
            ));
            window.setContentView(Some(&content));
            window.setDelegate(Some(ProtocolObject::from_ref(&*this)));
            window.center();

            this.ivars()
                .window
                .set(window)
                .expect("authentication window is initialized once");

            let popup_authenticator = Weak::new(&*this);
            let popup_web_view = Rc::downgrade(&this.ivars().web_view);
            let navigation_authenticator = Weak::new(&*this);
            let process_authenticator = Weak::new(&*this);
            let initial_url = if fresh_account {
                "about:blank"
            } else {
                authentication_url.as_str()
            };
            let builder = WebViewBuilder::new()
                .with_url(initial_url)
                .with_bounds(Rect {
                    position: LogicalPosition::new(0, 0).into(),
                    size: LogicalSize::new(980, 760).into(),
                })
                .with_data_store_identifier(AUTHENTICATION_STORE_IDENTIFIER)
                .with_navigation_handler(move |location| {
                    let Some(authenticator) = navigation_authenticator.load() else {
                        return true;
                    };
                    !authenticator.complete_callback(&location)
                })
                .with_on_web_content_process_terminate_handler(move || {
                    if let Some(authenticator) = process_authenticator.load() {
                        authenticator.fail("Battle.net's authentication page stopped responding");
                    }
                })
                .with_new_window_req_handler(move |location, _features| {
                    let Some(authenticator) = popup_authenticator.load() else {
                        return NewWindowResponse::Deny;
                    };
                    if authenticator.complete_callback(&location) {
                        return NewWindowResponse::Deny;
                    }

                    let web_view = popup_web_view
                        .upgrade()
                        .and_then(|slot| slot.borrow().clone());
                    if let Some(web_view) = web_view
                        && let Err(error) = web_view.load_url(&location)
                    {
                        authenticator
                            .fail(&format!("Battle.net authentication page failed: {error}"));
                    }
                    NewWindowResponse::Deny
                });

            let native_content = NativeViewHandle(&content);
            let web_view = match builder.build_as_child(&native_content) {
                Ok(web_view) => Rc::new(web_view),
                Err(error) => {
                    this.fail(&format!("Battle.net authentication window failed: {error}"));
                    return this;
                }
            };
            if fresh_account {
                let cookies = match web_view.cookies() {
                    Ok(cookies) => cookies,
                    Err(error) => {
                        this.fail(&format!("Battle.net cookie reset failed: {error}"));
                        return this;
                    }
                };
                for cookie in &cookies {
                    if let Err(error) = web_view.delete_cookie(cookie) {
                        this.fail(&format!("Battle.net cookie reset failed: {error}"));
                        return this;
                    }
                }
                if let Err(error) = web_view.clear_all_browsing_data() {
                    this.fail(&format!("Battle.net session reset failed: {error}"));
                    return this;
                }
                if let Err(error) = web_view.load_url(authentication_url.as_str()) {
                    this.fail(&format!("Battle.net authentication page failed: {error}"));
                    return this;
                }
            }
            let native_web_view = web_view.webview();
            native_web_view.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            *this.ivars().web_view.borrow_mut() = Some(web_view);

            if fresh_account {
                if let Some(window) = this.ivars().window.get() {
                    window.makeKeyAndOrderFront(None);
                }
            } else {
                // SAFETY: the selector is implemented above with NSTimer's
                // single target argument, and the scheduled timer retains the
                // authenticator until this one-shot callback has fired.
                let _timer = unsafe {
                    NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                        SILENT_SSO_GRACE_SECONDS,
                        &this,
                        sel!(showIfAuthenticationIsPending:),
                        None,
                        false,
                    )
                };
            }
            this
        }

        fn complete_callback(&self, location: &str) -> bool {
            let Some(credential) = callback_credential(location) else {
                return false;
            };
            let Some(reply) = self.ivars().reply.borrow_mut().take() else {
                return true;
            };
            let _ = reply.send(Ok(credential));
            if let Some(window) = self.ivars().window.get() {
                window.orderOut(None);
            }
            true
        }

        fn fail(&self, message: &str) {
            let Some(reply) = self.ivars().reply.borrow_mut().take() else {
                return;
            };
            let _ = reply.send(Err(Error::Authentication(message.into())));
            if let Some(window) = self.ivars().window.get() {
                window.orderOut(None);
            }
        }

        pub(crate) fn dismiss(&self) {
            self.cancel();
            if let Some(window) = self.ivars().window.get() {
                window.close();
            }
        }

        fn cancel(&self) {
            let Some(reply) = self.ivars().reply.borrow_mut().take() else {
                return;
            };
            let _ = reply.send(Err(Error::Authentication(
                "Battle.net authentication was cancelled".into(),
            )));
        }
    }

    pub(crate) type WebAuthenticatorHandle = Retained<WebAuthenticator>;

    struct NativeViewHandle<'a>(&'a NSView);

    impl HasWindowHandle for NativeViewHandle<'_> {
        fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
            let handle = AppKitWindowHandle::new(NonNull::from(self.0).cast());
            // SAFETY: the borrowed handle cannot outlive `self`, which borrows the retained
            // content view for the duration of Wry's synchronous construction.
            Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::AppKit(handle)) })
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use macos::{WebAuthenticator, WebAuthenticatorHandle};

#[cfg(target_os = "windows")]
mod windows {
    use super::callback_credential;
    use std::{
        cell::RefCell,
        collections::HashMap,
        num::NonZeroIsize,
        rc::{Rc, Weak},
        sync::mpsc::Sender,
    };

    use url::Url;
    use windows::{
        Win32::{
            Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
            System::LibraryLoader::GetModuleHandleW,
            UI::WindowsAndMessaging::{
                AdjustWindowRectEx, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW,
                DestroyWindow, GetSystemMetrics, IsWindow, KillTimer, LoadCursorW,
                RegisterClassExW, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW, SetForegroundWindow,
                SetTimer, ShowWindow, WINDOW_EX_STYLE, WM_CLOSE, WM_NCDESTROY, WM_TIMER,
                WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
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

    use crate::{Error, Result};
    use superiority_core::{Product, bgs::SecretBytes};

    const WINDOW_WIDTH: i32 = 980;
    const WINDOW_HEIGHT: i32 = 760;
    const AUTHENTICATION_REVEAL_TIMER: usize = 1;
    const SILENT_SSO_GRACE_MILLIS: u32 = 1_000;

    type ReplySlot = Rc<RefCell<Option<Sender<Result<SecretBytes>>>>>;
    type WebViewSlot = Rc<RefCell<Option<Rc<WebView>>>>;

    thread_local! {
        static WINDOW_REPLIES: RefCell<HashMap<isize, ReplySlot>> = RefCell::new(HashMap::new());
    }

    pub(crate) struct WebAuthenticator {
        window: Option<HWND>,
        web_view: WebViewSlot,
        reply: ReplySlot,
    }

    impl WebAuthenticator {
        pub(crate) fn present(
            authentication_url: &Url,
            reply: Sender<Result<SecretBytes>>,
            _product: Product,
            fresh_account: bool,
        ) -> Self {
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
            let initial_url = if fresh_account {
                "about:blank"
            } else {
                authentication_url.as_str()
            };
            let builder = WebViewBuilder::new()
                .with_url(initial_url)
                .with_profile_name("superiority-battle-net")
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
                    if fresh_account {
                        let cookies = match view.cookies() {
                            Ok(cookies) => cookies,
                            Err(error) => {
                                fail_reply(
                                    &reply,
                                    &format!("Battle.net cookie reset failed: {error}"),
                                );
                                destroy_window(window, &web_view);
                                return Self {
                                    window: None,
                                    web_view,
                                    reply,
                                };
                            }
                        };
                        for cookie in &cookies {
                            if let Err(error) = view.delete_cookie(cookie) {
                                fail_reply(
                                    &reply,
                                    &format!("Battle.net cookie reset failed: {error}"),
                                );
                                destroy_window(window, &web_view);
                                return Self {
                                    window: None,
                                    web_view,
                                    reply,
                                };
                            }
                        }
                        if let Err(error) = view.clear_all_browsing_data() {
                            fail_reply(
                                &reply,
                                &format!("Battle.net session reset failed: {error}"),
                            );
                            destroy_window(window, &web_view);
                            return Self {
                                window: None,
                                web_view,
                                reply,
                            };
                        }
                        if let Err(error) = view.load_url(authentication_url.as_str()) {
                            fail_reply(
                                &reply,
                                &format!("Battle.net authentication page failed: {error}"),
                            );
                            destroy_window(window, &web_view);
                            return Self {
                                window: None,
                                web_view,
                                reply,
                            };
                        }
                    }
                    *web_view.borrow_mut() = Some(Rc::new(view));
                    unsafe {
                        if fresh_account {
                            let _ = ShowWindow(window, SW_SHOW);
                            let _ = SetForegroundWindow(window);
                        } else {
                            let _ = SetTimer(
                                Some(window),
                                AUTHENTICATION_REVEAL_TIMER,
                                SILENT_SSO_GRACE_MILLIS,
                                None,
                            );
                        }
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

        pub(crate) fn dismiss(&self) {
            cancel_reply(&self.reply);
            if let Some(window) = self.window {
                destroy_window(window, &self.web_view);
            }
        }
    }

    impl Drop for WebAuthenticator {
        fn drop(&mut self) {
            self.dismiss();
        }
    }

    fn create_window(reply: ReplySlot) -> std::result::Result<HWND, String> {
        let instance = unsafe { GetModuleHandleW(None) }
            .map_err(|error| error.to_string())?
            .into();
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(authentication_window_proc),
            hInstance: instance,
            hCursor: unsafe {
                LoadCursorW(None, windows::Win32::UI::WindowsAndMessaging::IDC_ARROW)
            }
            .unwrap_or_default(),
            lpszClassName: w!("SuperiorityWebAuthenticator"),
            ..WNDCLASSEXW::default()
        };
        unsafe {
            // RegisterClassExW returning zero is harmless when this process already registered
            // the fixed class name during an earlier authentication attempt.
            RegisterClassExW(&class);
        }

        let style = WS_OVERLAPPEDWINDOW;
        let ex_style = WINDOW_EX_STYLE::default();
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: WINDOW_WIDTH,
            bottom: WINDOW_HEIGHT,
        };
        unsafe { AdjustWindowRectEx(&mut bounds, style, false, ex_style) }
            .map_err(|error| error.to_string())?;
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        let (screen_width, screen_height) =
            unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };

        let window = unsafe {
            CreateWindowExW(
                ex_style,
                w!("SuperiorityWebAuthenticator"),
                w!("Superiority — Battle.net Authentication"),
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
            WM_TIMER if wparam.0 == AUTHENTICATION_REVEAL_TIMER => {
                let _ = unsafe { KillTimer(Some(window), AUTHENTICATION_REVEAL_TIMER) };
                let pending = WINDOW_REPLIES.with(|replies| {
                    replies
                        .borrow()
                        .get(&window_key(window))
                        .is_some_and(|reply| reply.borrow().is_some())
                });
                if pending {
                    unsafe {
                        let _ = ShowWindow(window, SW_SHOW);
                        let _ = SetForegroundWindow(window);
                    }
                }
                LRESULT(0)
            }
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
        let Some(credential) = callback_credential(location) else {
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
            // SAFETY: the borrowed handle cannot outlive this wrapper, and the native window
            // remains alive throughout Wry's synchronous construction.
            Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
        }
    }

    pub(crate) type WebAuthenticatorHandle = WebAuthenticator;
}

#[cfg(target_os = "windows")]
pub(crate) use windows::{WebAuthenticator, WebAuthenticatorHandle};

#[cfg(any(target_os = "macos", target_os = "windows", test))]
use url::Url;

#[cfg(any(target_os = "macos", target_os = "windows", test))]
use superiority_core::bgs::SecretBytes;

#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn callback_credential(location: &str) -> Option<SecretBytes> {
    let url = Url::parse(location).ok()?;
    if url.scheme() != "http" || url.host_str() != Some("localhost") || url.port() != Some(0) {
        return None;
    }
    let credential = url
        .query_pairs()
        .find_map(|(name, value)| (name == "ST").then(|| value.into_owned()))?;
    let bytes = credential.into_bytes();
    if bytes.is_empty() || bytes.len() > 1024 || !bytes.iter().all(u8::is_ascii_graphic) {
        return None;
    }
    SecretBytes::new(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_port_zero_battle_net_callback() {
        let credential = callback_credential("http://localhost:0/?ST=US-test-token")
            .expect("valid callback must be accepted");
        assert_eq!(credential.expose(), b"US-test-token");
    }

    #[test]
    fn rejects_non_callback_and_malformed_credentials() {
        assert!(callback_credential("http://localhost:8080/?ST=US-test-token").is_none());
        assert!(callback_credential("https://localhost:0/?ST=US-test-token").is_none());
        assert!(callback_credential("http://localhost:0/?ST=").is_none());
        assert!(callback_credential("http://example.com:0/?ST=US-test-token").is_none());
    }
}
