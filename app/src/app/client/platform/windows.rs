use std::{path::PathBuf, rc::Rc, sync::mpsc::Sender};

use chrono::Local;
use gpui::{Window, actions};

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
    let platform = gpui_windows::WindowsPlatform::new(false)
        .expect("failed to initialize the Windows GPUI platform");
    gpui::Application::with_platform(Rc::new(platform))
}

pub(in crate::app) fn resource_directory() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(|parent| parent.join("resources")))
        .unwrap_or_else(|| PathBuf::from("resources"))
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) enum AppMenuCommand {
    CheckForUpdates,
    OpenSettings,
}

pub(crate) type NativeAppMenuTarget = ();

pub(crate) fn new_app_menu_target(_commands: Sender<AppMenuCommand>) -> NativeAppMenuTarget {}

pub(crate) fn install_app_menu_targets(_target: &NativeAppMenuTarget) {}

pub(crate) fn show_about() {}

pub(crate) fn current_timestamp() -> String {
    Local::now().format("%-I:%M %p").to_string()
}

pub(crate) fn begin_window_drag(window: &Window) {
    window.start_window_move();
}

pub(in crate::app) fn configure_window(_window: &Window) {}
