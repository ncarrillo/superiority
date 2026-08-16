use super::*;

fn window_bounds(cx: &App) -> Bounds<gpui::Pixels> {
    Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx)
}

pub fn run() {
    let resources = platform::resource_directory();
    #[cfg(target_os = "windows")]
    let dock_action = std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|arguments| arguments[0] == "--dock-action")
        .and_then(|arguments| arguments[1].parse::<usize>().ok());
    #[cfg(target_os = "windows")]
    if platform::forward_dock_action(dock_action) {
        return;
    }
    #[cfg(target_os = "windows")]
    let dock_action_events = platform::listen_for_dock_actions();
    platform::application()
        .with_assets(Assets {
            base: resources.clone(),
        })
        .run(move |cx: &mut App| {
            cx.set_app_identity("com.superiority.sc2-chat", "Superiority");
            chrome::load_fonts(&resources, cx);
            ui_text_input::init(cx);
            let (app_menu_commands, app_menu_events) = std::sync::mpsc::channel();
            let app_menu_target = platform::new_app_menu_target(app_menu_commands);
            cx.on_action(|_: &About, _| platform::show_about());
            cx.on_action(|_: &OpenProtocolViewer, cx| super::super::protocol_viewer::open(cx));
            cx.on_action(|_: &Quit, cx| cx.quit());
            #[cfg(target_os = "macos")]
            cx.bind_keys([
                KeyBinding::new("cmd-,", OpenSettings, None),
                KeyBinding::new("cmd-q", Quit, None),
            ]);
            #[cfg(target_os = "windows")]
            cx.bind_keys([
                KeyBinding::new("ctrl-,", OpenSettings, None),
                KeyBinding::new("ctrl-q", Quit, None),
            ]);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let bounds = window_bounds(cx);
            let retained_app_menu_target = app_menu_target.clone();
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(860.0), px(540.0))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("Superiority".into()),
                            appears_transparent: true,
                            ..Default::default()
                        }),
                        is_movable: cfg!(target_os = "windows"),
                        ..Default::default()
                    },
                    move |window, cx| {
                        platform::configure_window(window);
                        cx.new(|cx| {
                            SuperiorityView::new(cx, app_menu_events, retained_app_menu_target)
                        })
                    },
                )
                .expect("the application must be able to open its main window");
            cx.on_action(move |_: &CheckForUpdates, cx| {
                window
                    .update(cx, |view, _, cx| view.check_for_updates(cx))
                    .ok();
            });
            cx.on_action(move |_: &OpenSettings, cx| {
                window.update(cx, |view, _, cx| view.open_settings(cx)).ok();
            });
            cx.set_menus(vec![
                Menu {
                    name: "Superiority".into(),
                    items: vec![
                        MenuItem::action("About Superiority", About),
                        MenuItem::action("Check for Updates…", CheckForUpdates),
                        MenuItem::action("Settings…", OpenSettings),
                        MenuItem::action("Protocol Viewer…", OpenProtocolViewer),
                        MenuItem::separator(),
                        MenuItem::action("Quit Superiority", Quit),
                    ],
                    disabled: false,
                },
                Menu {
                    name: "Edit".into(),
                    items: vec![
                        MenuItem::action("Undo", ui_text_input::Undo),
                        MenuItem::action("Redo", ui_text_input::Redo),
                        MenuItem::separator(),
                        MenuItem::action("Cut", ui_text_input::Cut),
                        MenuItem::action("Copy", ui_text_input::Copy),
                        MenuItem::action("Paste", ui_text_input::Paste),
                        MenuItem::action("Select All", ui_text_input::SelectAll),
                    ],
                    disabled: false,
                },
            ]);
            #[cfg(target_os = "windows")]
            {
                cx.update_jump_list(
                    vec![
                        MenuItem::action("About Superiority", About),
                        MenuItem::action("Check for Updates…", CheckForUpdates),
                        MenuItem::action("Settings…", OpenSettings),
                        MenuItem::action("Protocol Viewer…", OpenProtocolViewer),
                    ],
                    Vec::new(),
                )
                .detach();
                if let Some(index) = dock_action {
                    cx.perform_dock_menu_action(index);
                }
                cx.spawn(async move |cx| {
                    loop {
                        while let Ok(index) = dock_action_events.try_recv() {
                            cx.update(|cx| {
                                cx.perform_dock_menu_action(index);
                                cx.activate(true);
                            });
                        }
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                    }
                })
                .detach();
            }
            platform::install_app_menu_targets(&app_menu_target);
            cx.activate(true);
        });
}
