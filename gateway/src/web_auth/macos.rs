// starcraft ii handles challengelistener.onexternalchallenge with an embedded
// browser whose navigation handler intercepts the localhost credential redirect.
// nothing listens on port zero, so the redirect is caught before it can be
// dispatched. the credential is never displayed or stored.

use std::{
    cell::{OnceCell, RefCell},
    ptr::NonNull,
    rc::Rc,
    sync::mpsc::{self, Sender, TryRecvError},
    time::{Duration, Instant},
};

use objc2::{
    DefinedClass, MainThreadOnly, define_class, msg_send,
    rc::{Retained, Weak},
    runtime::ProtocolObject,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSAutoresizingMaskOptions, NSBackingStoreType,
    NSEventMask, NSView, NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSDate, NSDefaultRunLoopMode, NSNotification, NSObject, NSObjectProtocol,
    NSPoint, NSRect, NSSize, NSString,
};
use url::Url;
use wry::raw_window_handle::{
    AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
};
use wry::{
    NewWindowResponse, Rect, WebView, WebViewBuilder, WebViewBuilderExtDarwin, WebViewExtMacOS,
    dpi::{LogicalPosition, LogicalSize},
};

use sc2_core::{Error, Result, bgs::SecretBytes};

use super::{Cancellation, cancelled, parse_credential_redirect};

// how long the event pump waits per iteration before re-checking the reply.
const PUMP_INTERVAL: f64 = 0.05;

// wkwebview derives its user agent from the host bundle, which this binary
// does not have. a plain safari agent gets the full login flow.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
     AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";

// use the same persistent wkwebsitedatastore identifier as superiority. wry
// supplies a usable store even though this gateway is an unbundled cli.
const AUTHENTICATION_STORE_IDENTIFIER: [u8; 16] = [
    0x7A, 0x48, 0xAF, 0x1D, 0x4B, 0x08, 0x4F, 0x37, 0x9F, 0x4F, 0x5D, 0x6E, 0x81, 0x3F, 0xE2, 0xA4,
];

type WebViewSlot = Rc<RefCell<Option<Rc<WebView>>>>;

#[derive(Default)]
pub(crate) struct AuthenticatorIvars {
    window: OnceCell<Retained<NSWindow>>,
    web_view: WebViewSlot,
    reply: RefCell<Option<Sender<Result<SecretBytes>>>>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AuthenticatorIvars]
    pub(crate) struct Authenticator;

    unsafe impl NSObjectProtocol for Authenticator {}

    unsafe impl NSWindowDelegate for Authenticator {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            self.cancel();
        }
    }
);

impl Authenticator {
    fn present(
        mtm: MainThreadMarker,
        url: &Url,
        reply: Sender<Result<SecretBytes>>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AuthenticatorIvars {
            reply: RefCell::new(Some(reply)),
            ..AuthenticatorIvars::default()
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
        window.setTitle(&NSString::from_str("StarCraft II — Battle.net Login"));
        window.setContentView(Some(&content));
        window.setDelegate(Some(ProtocolObject::from_ref(&*this)));
        window.center();

        this.ivars()
            .window
            .set(window)
            .expect("window is initialized once");

        let popup_authenticator = Weak::new(&*this);
        let popup_web_view = Rc::downgrade(&this.ivars().web_view);
        let navigation_authenticator = Weak::new(&*this);
        let process_authenticator = Weak::new(&*this);
        let builder = WebViewBuilder::new()
            .with_url(url.as_str())
            .with_bounds(Rect {
                position: LogicalPosition::new(0, 0).into(),
                size: LogicalSize::new(980, 760).into(),
            })
            .with_user_agent(USER_AGENT)
            .with_devtools(false)
            .with_data_store_identifier(AUTHENTICATION_STORE_IDENTIFIER)
            .with_navigation_handler(move |location| {
                let Some(authenticator) = navigation_authenticator.load() else {
                    return true;
                };
                !authenticator.complete(&location)
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
                if authenticator.complete(&location) {
                    return NewWindowResponse::Deny;
                }

                let web_view = popup_web_view
                    .upgrade()
                    .and_then(|slot| slot.borrow().clone());
                if let Some(web_view) = web_view
                    && let Err(error) = web_view.load_url(&location)
                {
                    authenticator.fail(&format!("Battle.net authentication page failed: {error}"));
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
        let native_web_view = web_view.webview();
        native_web_view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        *this.ivars().web_view.borrow_mut() = Some(web_view);

        if let Some(window) = this.ivars().window.get() {
            window.makeKeyAndOrderFront(None);
        }
        this
    }

    // deliver the credential if location is the localhost:0 callback.
    fn complete(&self, location: &str) -> bool {
        let Ok(credential) = parse_credential_redirect(location) else {
            return false;
        };
        self.settle(Ok(credential));
        true
    }

    fn fail(&self, message: &str) {
        self.settle(Err(Error::Authentication(message.to_owned())));
    }

    fn cancel(&self) {
        self.settle(Err(Error::Authentication(
            "Battle.net login was cancelled".into(),
        )));
    }

    // the first outcome wins; later ones are ignored.
    fn settle(&self, outcome: Result<SecretBytes>) {
        let Some(reply) = self.ivars().reply.borrow_mut().take() else {
            return;
        };
        let _ = reply.send(outcome);
        if let Some(window) = self.ivars().window.get() {
            window.orderOut(None);
        }
    }

    fn dismiss(&self) {
        self.cancel();
        if let Some(window) = self.ivars().window.get() {
            window.close();
        }
    }
}

struct NativeViewHandle<'a>(&'a NSView);

impl HasWindowHandle for NativeViewHandle<'_> {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        let handle = AppKitWindowHandle::new(NonNull::from(self.0).cast());
        // safety: the borrowed handle cannot outlive `self`, which borrows the retained
        // content view for the duration of wry's synchronous construction.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::AppKit(handle)) })
    }
}

// blocks until the window yields a credential. must run on the main thread:
// appkit windows and the event pump below both require it.
pub fn request_credential(
    url: &Url,
    timeout: Duration,
    cancellation: &Cancellation,
) -> Result<SecretBytes> {
    let mtm = MainThreadMarker::new().ok_or_else(|| {
        Error::Authentication("Battle.net web login must run on the main thread".into())
    })?;

    let application = NSApplication::sharedApplication(mtm);
    application.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let (sender, receiver) = mpsc::channel();
    let authenticator = Authenticator::present(mtm, url, sender);
    #[allow(deprecated)]
    application.activateIgnoringOtherApps(true);

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
        // draining one event at a time also runs the run loop in the default
        // mode, which is what drives wkwebview's networking and rendering.
        let until = NSDate::dateWithTimeIntervalSinceNow(PUMP_INTERVAL);
        let event = unsafe {
            application.nextEventMatchingMask_untilDate_inMode_dequeue(
                NSEventMask::Any,
                Some(&until),
                NSDefaultRunLoopMode,
                true,
            )
        };
        if let Some(event) = event {
            application.sendEvent(&event);
        }
    };

    authenticator.dismiss();
    application.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    outcome
}
