use std::{path::PathBuf, sync::mpsc::Sender};

use gpui::{Window, actions};
use objc2::{
    DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::Sel, sel,
};
use objc2_app_kit::{
    NSApplication, NSColor, NSMenu, NSTitlebarSeparatorStyle, NSView, NSWorkspace,
};
use objc2_foundation::{
    MainThreadMarker, NSDate, NSDateFormatter, NSObject, NSObjectProtocol, NSString,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

actions!(
    superiority,
    [
        About,
        CheckForUpdates,
        OpenProtocolViewer,
        OpenSettings,
        Quit
    ]
);

pub(in crate::app) fn application() -> gpui::Application {
    gpui_platform::application()
}

pub(in crate::app) fn resource_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("macos/resources")
}

#[derive(Clone, Copy)]
pub(crate) enum AppMenuCommand {
    CheckForUpdates,
    OpenSettings,
}

pub(crate) struct AppMenuTargetIvars {
    commands: Sender<AppMenuCommand>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppMenuTargetIvars]
    pub(crate) struct AppMenuTarget;

    unsafe impl NSObjectProtocol for AppMenuTarget {}

    impl AppMenuTarget {
        #[unsafe(method(showSoftwareUpdate:))]
        fn show_software_update(&self, _sender: &NSObject) {
            let _ = self
                .ivars()
                .commands
                .send(AppMenuCommand::CheckForUpdates);
        }

        #[unsafe(method(showSettings:))]
        fn show_settings(&self, _sender: &NSObject) {
            let _ = self.ivars().commands.send(AppMenuCommand::OpenSettings);
        }
    }
);

pub(crate) type NativeAppMenuTarget = Retained<AppMenuTarget>;

pub(crate) fn main_thread_marker() -> MainThreadMarker {
    MainThreadMarker::new().expect("the application must run on the macOS main thread")
}

pub(crate) fn new_app_menu_target(commands: Sender<AppMenuCommand>) -> NativeAppMenuTarget {
    let target =
        AppMenuTarget::alloc(main_thread_marker()).set_ivars(AppMenuTargetIvars { commands });
    unsafe { msg_send![super(target), init] }
}

fn retarget_menu_item(menu: &NSMenu, title: &str, target: &AppMenuTarget, action: Sel) {
    let Some(item) = menu.itemWithTitle(&NSString::from_str(title)) else {
        return;
    };
    unsafe {
        item.setTarget(Some(target));
        item.setAction(Some(action));
    }
}

pub(crate) fn install_app_menu_targets(target: &AppMenuTarget) {
    let application = NSApplication::sharedApplication(main_thread_marker());
    let Some(application_menu) = application
        .mainMenu()
        .and_then(|menu| menu.itemAtIndex(0))
        .and_then(|item| item.submenu())
    else {
        return;
    };
    retarget_menu_item(
        &application_menu,
        "Check for Updates…",
        target,
        sel!(showSoftwareUpdate:),
    );
    retarget_menu_item(&application_menu, "Settings…", target, sel!(showSettings:));
}

pub(crate) fn show_about() {
    NSApplication::sharedApplication(main_thread_marker()).orderFrontStandardAboutPanel(None);
}

/// Whether the reader has asked the system for less movement. Honoured by the
/// picker's choreography, which keeps every fade and drops every journey.
pub(in crate::app) fn reduce_motion() -> bool {
    if std::env::var_os("SUPERIORITY_REDUCED_MOTION").is_some() {
        return true;
    }
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

pub(crate) fn current_timestamp() -> String {
    let formatter = NSDateFormatter::new();
    formatter.setDateFormat(Some(&NSString::from_str("h:mm a")));
    formatter.stringFromDate(&NSDate::new()).to_string()
}

fn native_content_view(window: &Window) -> Option<Retained<NSView>> {
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return None;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    unsafe { Retained::<NSView>::retain(handle.ns_view.as_ptr().cast()) }
}

pub(crate) fn begin_window_drag(window: &Window) {
    let Some(view) = native_content_view(window) else {
        return;
    };
    let Some(window) = view.window() else {
        return;
    };
    let Some(event) = window.currentEvent() else {
        return;
    };
    window.setMovable(true);
    window.performWindowDragWithEvent(&event);
    window.setMovable(false);
}

pub(in crate::app) fn configure_window(window: &Window) {
    let Some(view) = native_content_view(window) else {
        return;
    };
    let Some(window) = view.window() else {
        return;
    };
    let background = NSColor::colorWithSRGBRed_green_blue_alpha(0.045, 0.059, 0.082, 1.0);
    window.setOpaque(true);
    window.setBackgroundColor(Some(&background));
    window.setTitlebarAppearsTransparent(true);
    window.setTitlebarSeparatorStyle(NSTitlebarSeparatorStyle::None);
    window.setMovable(false);

    fn clear_titlebar_fill(view: &NSView) {
        let class_name = view.class().name().to_bytes();
        if class_name.ends_with(b"_NSTitlebarDecorationView")
            || class_name == b"NSTitlebarBackgroundView"
        {
            view.setHidden(true);
        }
        for child in &view.subviews() {
            clear_titlebar_fill(&child);
        }
    }

    let mut root = view.clone();
    while let Some(parent) = unsafe { root.superview() } {
        root = parent;
    }
    clear_titlebar_fill(&root);
}
