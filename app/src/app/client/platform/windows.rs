use std::{
    path::PathBuf,
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
    thread,
    time::Duration,
};

use chrono::Local;
use gpui::{Div, Stateful, Window, WindowControlArea, actions, div, prelude::*, px, rgb};
use superiority_ui::theme::TAB_BAR_HEIGHT;
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_PIPE_CONNECTED, GENERIC_WRITE},
        Storage::FileSystem::{
            CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE, OPEN_EXISTING,
            PIPE_ACCESS_INBOUND, ReadFile, WriteFile,
        },
        System::Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
            PIPE_TYPE_MESSAGE, PIPE_WAIT,
        },
        UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW},
    },
    core::HSTRING,
};

pub(crate) const WINDOW_CONTROLS_WIDTH: f32 = 138.0;

fn dock_action_pipe() -> HSTRING {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "User".into());
    HSTRING::from(format!(r"\\.\pipe\Superiority-{user}-Dock-Actions"))
}

pub(crate) fn forward_dock_action(action: Option<usize>) -> bool {
    let Some(action) = action.and_then(|action| u8::try_from(action).ok()) else {
        return false;
    };
    for _ in 0..10 {
        let pipe = unsafe {
            CreateFileW(
                &dock_action_pipe(),
                GENERIC_WRITE.0,
                FILE_SHARE_MODE::default(),
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES::default(),
                None,
            )
        };
        if let Ok(pipe) = pipe {
            let sent = unsafe { WriteFile(pipe, Some(&[action]), None, None) }.is_ok();
            let _ = unsafe { CloseHandle(pipe) };
            return sent;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

pub(crate) fn listen_for_dock_actions() -> Receiver<usize> {
    let (sender, receiver) = std::sync::mpsc::channel();
    thread::Builder::new()
        .name("DockActions".into())
        .spawn(move || {
            let pipe = unsafe {
                CreateNamedPipeW(
                    &dock_action_pipe(),
                    PIPE_ACCESS_INBOUND,
                    PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                    1,
                    1,
                    1,
                    0,
                    None,
                )
            };
            if pipe.is_invalid() {
                return;
            }
            loop {
                let connected = match unsafe { ConnectNamedPipe(pipe, None) } {
                    Ok(()) => true,
                    Err(error) => error.code() == ERROR_PIPE_CONNECTED.to_hresult(),
                };
                if connected {
                    let mut action = [0_u8; 1];
                    if unsafe { ReadFile(pipe, Some(&mut action), None, None) }.is_ok() {
                        let _ = sender.send(usize::from(action[0]));
                    }
                }
                let _ = unsafe { DisconnectNamedPipe(pipe) };
            }
        })
        .expect("failed to start the dock action listener");
    receiver
}

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

pub(crate) fn show_about() {
    let message = HSTRING::from(format!(
        "Superiority {}\n\nBattle.net chat for StarCraft II.",
        env!("CARGO_PKG_VERSION")
    ));
    unsafe {
        MessageBoxW(
            None,
            &message,
            &HSTRING::from("About Superiority"),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

pub(crate) fn current_timestamp() -> String {
    Local::now().format("%-I:%M %p").to_string()
}

pub(in crate::app) fn configure_window(_window: &Window) {}

pub(crate) fn window_controls(window: &Window) -> Stateful<Div> {
    window_controls_with_height(window, TAB_BAR_HEIGHT)
}

pub(crate) fn window_controls_with_height(window: &Window, height: f32) -> Stateful<Div> {
    let maximize = if window.is_maximized() {
        caption_button("restore-window", "\u{e923}", WindowControlArea::Max, false)
    } else {
        caption_button("maximize-window", "\u{e922}", WindowControlArea::Max, false)
    };
    div()
        .id("windows-window-controls")
        .absolute()
        .top_0()
        .right_0()
        .h(px(height))
        .flex()
        .font_family("Segoe Fluent Icons")
        .text_color(rgb(0xc8d7ea))
        .occlude()
        .child(caption_button(
            "minimize-window",
            "\u{e921}",
            WindowControlArea::Min,
            false,
        ))
        .child(maximize)
        .child(caption_button(
            "close-window",
            "\u{e8bb}",
            WindowControlArea::Close,
            true,
        ))
}

fn caption_button(
    id: &'static str,
    icon: &'static str,
    control: WindowControlArea,
    close: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(WINDOW_CONTROLS_WIDTH / 3.0))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .cursor_default()
        .when(close, |button| {
            button
                .hover(|style| style.bg(rgb(0xe81123)).text_color(rgb(0xffffff)))
                .active(|style| style.bg(rgb(0xb50d1d)))
        })
        .when(!close, |button| {
            button
                .hover(|style| style.bg(rgb(0x1a2c3e)))
                .active(|style| style.bg(rgb(0x27435e)))
        })
        .window_control_area(control)
        .child(icon)
}
