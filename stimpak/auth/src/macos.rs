use std::{
    cell::{OnceCell, RefCell},
    ptr::NonNull,
    rc::Rc,
};

use objc2::runtime::ProtocolObject;
use objc2::{
    DefinedClass, MainThreadOnly, define_class,
    rc::{Retained, Weak},
};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBackingStoreType, NSView, NSWindow, NSWindowDelegate,
    NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use url::Url;
use wry::raw_window_handle::{
    AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
};
use wry::{
    NewWindowResponse, Rect, WebView, WebViewBuilder, WebViewBuilderExtDarwin, WebViewExtMacOS,
    dpi::{LogicalPosition, LogicalSize},
};

use crate::{AuthOutcome, Completion, callback_token};

const DATA_STORE: [u8; 16] = [
    0x1B, 0x9E, 0x54, 0xC2, 0x7A, 0x33, 0x4D, 0x61, 0xB8, 0x2F, 0x0C, 0xE7, 0x95, 0x14, 0xAD, 0x38,
];

type WebViewSlot = Rc<RefCell<Option<Rc<WebView>>>>;

pub(crate) struct Authenticator {
    inner: Option<Retained<WebAuthenticator>>,
}

pub(crate) struct AuthenticatorIvars {
    window: OnceCell<Retained<NSWindow>>,
    web_view: WebViewSlot,
    completion: RefCell<Option<Completion>>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AuthenticatorIvars]
    struct WebAuthenticator;

    unsafe impl NSObjectProtocol for WebAuthenticator {}

    unsafe impl NSWindowDelegate for WebAuthenticator {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            self.settle(AuthOutcome::Cancelled);
        }
    }
);

impl Authenticator {
    pub(crate) fn empty() -> Self {
        Self { inner: None }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn present(url: &Url, fresh_account: bool, completion: Completion) -> Self {
        let Some(mtm) = MainThreadMarker::new() else {
            completion(AuthOutcome::Error(
                "embedded authentication must start on the macOS main thread".into(),
            ));
            return Self::empty();
        };

        let this = WebAuthenticator::alloc(mtm).set_ivars(AuthenticatorIvars {
            window: OnceCell::new(),
            web_view: Rc::new(RefCell::new(None)),
            completion: RefCell::new(Some(completion)),
        });
        let this: Retained<WebAuthenticator> = unsafe { objc2::msg_send![super(this), init] };

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
        window.setTitle(&NSString::from_str("Stimpak — Battle.net Authentication"));
        window.setContentView(Some(&content));
        window.setDelegate(Some(ProtocolObject::from_ref(&*this)));
        window.center();
        this.ivars().window.set(window).expect("window is set once");

        let navigation = Weak::new(&*this);
        let popup_authenticator = Weak::new(&*this);
        let popup_web_view = Rc::downgrade(&this.ivars().web_view);
        let process = Weak::new(&*this);
        let initial = if fresh_account {
            "about:blank"
        } else {
            url.as_str()
        };
        let builder = WebViewBuilder::new()
            .with_url(initial)
            .with_bounds(Rect {
                position: LogicalPosition::new(0, 0).into(),
                size: LogicalSize::new(980, 760).into(),
            })
            .with_data_store_identifier(DATA_STORE)
            .with_devtools(false)
            .with_navigation_handler(move |location| {
                navigation
                    .load()
                    .is_none_or(|authenticator| !authenticator.complete(&location))
            })
            .with_on_web_content_process_terminate_handler(move || {
                if let Some(authenticator) = process.load() {
                    authenticator.fail("Battle.net's authentication page stopped responding");
                }
            })
            .with_new_window_req_handler(move |location, _features| {
                let Some(authenticator) = popup_authenticator.load() else {
                    return NewWindowResponse::Deny;
                };
                if authenticator.complete(&location) {
                    return NewWindowResponse::Deny;
                }
                if let Some(web_view) = popup_web_view
                    .upgrade()
                    .and_then(|slot| slot.borrow().clone())
                    && let Err(error) = web_view.load_url(&location)
                {
                    authenticator.fail(&format!("authentication page failed: {error}"));
                }
                NewWindowResponse::Deny
            });

        let native_content = NativeViewHandle(&content);
        let web_view = match builder.build_as_child(&native_content) {
            Ok(web_view) => Rc::new(web_view),
            Err(error) => {
                this.fail(&format!("could not open the embedded webview: {error}"));
                return Self { inner: Some(this) };
            }
        };

        if fresh_account {
            let reset = web_view
                .cookies()
                .and_then(|cookies| {
                    for cookie in &cookies {
                        web_view.delete_cookie(cookie)?;
                    }
                    web_view.clear_all_browsing_data()
                })
                .and_then(|()| web_view.load_url(url.as_str()));
            if let Err(error) = reset {
                this.fail(&format!("could not reset the Battle.net session: {error}"));
                return Self { inner: Some(this) };
            }
        }

        web_view.webview().setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        *this.ivars().web_view.borrow_mut() = Some(web_view);
        if let Some(window) = this.ivars().window.get() {
            window.makeKeyAndOrderFront(None);
        }
        Self { inner: Some(this) }
    }

    pub(crate) fn cancel(&self) {
        if let Some(inner) = &self.inner {
            inner.cancel();
        }
    }
}

impl Drop for Authenticator {
    fn drop(&mut self) {
        if let Some(inner) = &self.inner {
            inner.cancel();
            if let Some(window) = inner.ivars().window.get() {
                window.close();
            }
        }
    }
}

impl WebAuthenticator {
    fn complete(&self, location: &str) -> bool {
        let Some(token) = callback_token(location) else {
            return false;
        };
        self.settle(AuthOutcome::Token(token));
        true
    }

    fn fail(&self, message: &str) {
        self.settle(AuthOutcome::Error(message.to_owned()));
    }

    fn cancel(&self) {
        self.settle(AuthOutcome::Cancelled);
        if let Some(window) = self.ivars().window.get() {
            window.orderOut(None);
        }
    }

    fn settle(&self, outcome: AuthOutcome) {
        let Some(completion) = self.ivars().completion.borrow_mut().take() else {
            return;
        };
        completion(outcome);
        if let Some(window) = self.ivars().window.get() {
            window.orderOut(None);
        }
    }
}

struct NativeViewHandle<'a>(&'a NSView);

impl HasWindowHandle for NativeViewHandle<'_> {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let handle = AppKitWindowHandle::new(NonNull::from(self.0).cast());
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::AppKit(handle)) })
    }
}
