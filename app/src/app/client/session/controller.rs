use super::*;

const UPDATE_PREVIEW_NOTES: &str = r#"# Native Markdown rendering

Superiority renders **formatted release notes** directly in the application, including *emphasis*, `inline code`, and [clickable links](https://superioritybot.com).

## What changed

- Styled headings and paragraphs
- Nested lists
  - Native scrolling
  - Text selection and copying
- [x] Signed update metadata
- [ ] Installation pending

> Release notes use the application's standard spacing, colors, and typography.

```rust
let renderer = "native";
```

| Surface | Status |
| --- | --- |
| Desktop | Native |
| Browser | Native |
"#;

impl SuperiorityView {
    pub(in crate::app::client) fn trace(message: impl std::fmt::Display) {
        if std::env::var_os("SUPERIORITY_TRACE").is_some() {
            eprintln!("superiority: {message}");
        }
    }

    pub(in crate::app::client) fn new(
        cx: &mut Context<Self>,
        app_menu_events: Receiver<AppMenuCommand>,
        app_menu_target: NativeAppMenuTarget,
    ) -> Self {
        let resources = platform::resource_directory();
        let preview_toast = std::env::var_os("SUPERIORITY_PREVIEW_TOAST").is_some();
        let preview_join = std::env::var_os("SUPERIORITY_PREVIEW_JOIN").is_some();
        let preview_update = std::env::var_os("SUPERIORITY_PREVIEW_UPDATE").is_some();
        let live_mode = !preview_toast && !preview_join && !preview_update;
        let preferences = preferences::UserPreferences::load();
        let remembered_group_names = preferences::load_group_names();
        let restored_channels = preferences::load_open_channels(DEFAULT_PUBLIC_CHANNEL);
        let restored_tabs = restored_channels
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, channel)| {
                let mut tab = ChannelState::pending_live(index as u64, channel.clone());
                if let ChatChannel::Club(club_id) = channel
                    && let Some(name) = remembered_group_names.get(&club_id)
                {
                    tab.title.clone_from(name);
                }
                tab
            })
            .collect::<Vec<_>>();
        let next_live_tab_id = restored_tabs.len() as u64;
        let show_timestamps = preferences.show_timestamps;
        let show_membership = preferences.show_membership;
        let live_enabled = preferences.live_enabled;
        let uplink = uplink::UplinkControl::new();
        let live_identity = uplink::config::load_identity();
        uplink.update_config(|config| {
            config.enabled = live_enabled;
            if let Some(identity) = &live_identity {
                config.adopt_identity(identity.token.clone(), identity.feed_id.clone());
            }
        });
        if let Some(identity) = &live_identity {
            uplink.stats.set_feed_url(Some(identity.url.clone()));
        }
        let (update_service, update_events) = if live_mode {
            UpdateService::start().map_or((None, None), |(service, events)| {
                (Some(service), Some(events))
            })
        } else {
            (None, None)
        };
        let startup_update_check_pending = update_service.is_some();
        let startup_connection_pending = startup_update_check_pending;
        let (commands, events) = if live_mode {
            let client = spawn_client(Box::new(uplink::spawn(
                uplink.clone(),
                remembered_group_names.clone(),
            )));
            if !startup_connection_pending {
                let _ = client.commands.send(ClientCommand::Connect {
                    force_interactive: false,
                    channels: restored_channels,
                });
            }
            (Some(client.commands), Some(client.events))
        } else {
            (None, None)
        };
        let mut update_model = UpdateModel::default();
        if preview_update {
            update_model.apply_event(
                &serde_json::json!({
                    "kind": "available",
                    "version": "0.2.0-preview",
                    "title": "Markdown renderer preview",
                    "notes": UPDATE_PREVIEW_NOTES,
                    "notes_format": "markdown",
                    "size": 18_874_368,
                })
                .to_string(),
            );
        }
        let composer = ui_text_input::TextInput::new("Press Enter to chat", cx);
        let join_input = ui_text_input::TextInput::new("Search, or type a channel name", cx);
        let roster_input = ui_text_input::TextInput::new("Filter members", cx);
        let conversation_input = ui_text_input::TextInput::new("Write a message", cx);
        let input_subscriptions = [&composer, &join_input, &roster_input, &conversation_input]
            .into_iter()
            .map(|input| {
                input.subscribe(cx, |this, cx| {
                    this.sync_text_inputs();
                    cx.notify();
                })
            })
            .collect();
        let this = Self {
            focus_handle: cx.focus_handle(),
            runtime: ClientRuntime {
                app_menu_events,
                _app_menu_target: app_menu_target,
                live_mode,
                commands,
                events,
                authenticator: None,
                uplink,
                live_auth_notified: false,
            },
            connection: ConnectionComponent {
                stage: if live_mode && !startup_connection_pending {
                    ConnectionStage::WebAuthentication
                } else {
                    ConnectionStage::Disconnected
                },
                error: None,
                starting: false,
                signed_out: false,
                sign_out_requested: false,
                dialog_visible: live_mode && !startup_connection_pending,
                dialog_closing: false,
                close_due: None,
                hide_due: None,
                fill: 0.0,
                floor: 0.0,
                ceiling: 1.0 / CONNECTION_STEPS as f32,
                progress_updated: Instant::now(),
            },
            warnings: WarningComponent {
                warning_dialog: None,
                warning_closing: false,
                warning_hide_due: None,
            },
            updates: UpdateComponent {
                update_service,
                update_events,
                update_model,
                update_notes_selection: ui_release_notes::ReleaseNotesSelection::default(),
                update_notes_scroll: ScrollHandle::new(),
                update_dialog_visible: preview_update,
                update_dialog_closing: false,
                update_hide_due: None,
                manual_update_check_deadline: None,
                startup_update_check_pending,
                startup_update_check_started: None,
                startup_connection_pending,
            },
            composer: ComposerComponent {
                composer_focused: false,
                composer,
            },
            join: JoinComponent {
                join_focused: false,
                join_input,
                join_query: String::new(),
                join_selected: 0,
                join_scroll: ScrollHandle::new(),
                awaiting_joins: Vec::new(),
                group_search_due: None,
                public_channels: BTreeMap::new(),
                groups: BTreeMap::new(),
                remembered_group_names,
                member_groups: BTreeSet::new(),
                group_search: Vec::new(),
                invitations: preview_toast
                    .then(|| {
                        vec![UiInvitation {
                            id: 1,
                            kind: InvitationKind::Group { club_id: 5322 },
                            inviter: Some("Sledgehammer".to_owned()),
                            destination: Some("<MDGTN> Blood Nation".to_owned()),
                            closing: false,
                        }]
                    })
                    .unwrap_or_default(),
                next_invitation_id: if preview_toast { 2 } else { 1 },
                join_assets_warming: true,
                join_warmup_started: false,
            },
            channels: ChannelComponent {
                navigation: ui_workspace::NavigationState::default(),
                tabs: if live_mode {
                    restored_tabs
                } else {
                    vec![ChannelState::fixture()]
                },
                next_tab_id: if live_mode { next_live_tab_id } else { 1 },
                active_tab: 0,
                tab_close: None,
                hovered_tab: None,
                tab_selection_started: Some(Instant::now()),
                channel_transition: None,
                chat_entry_reveal: None,
            },
            chat: ChatComponent {
                transcript: ui_workspace::TranscriptState::default(),
            },
            roster: RosterComponent {
                roster: ui_workspace::RosterState::default(),
                roster_input,
                roster_hovered: false,
                roster_defer_started: None,
                pending_rosters: HashMap::new(),
                roster_batch_started: None,
                roster_flush_at: None,
                roster_debounce_window: ROSTER_DEBOUNCE_BASE,
                portraits: PortraitRegistry::load(&resources),
            },
            overlays: OverlayComponent {
                active: preview_join.then_some(Overlay::Join),
                closing: false,
                epoch: 0,
            },
            settings: SettingsComponent {
                show_timestamps,
                show_membership,
                live_enabled,
                background: preferences.background().path,
                active_settings_page: 0,
                settings_page_transition: None,
                settings_tooltip: None,
                checkbox_animations: [None; 3],
                privacy_scroll: ScrollHandle::new(),
            },
            social: SocialComponent {
                social_collapsed: [false, false],
                friends_snapshot: Vec::new(),
                friends: Vec::new(),
                blocked_accounts: Vec::new(),
                social_scroll: ScrollHandle::new(),
                social_detail_open: false,
                social_pane_transition: None,
                conversation_peer: None,
                conversation_input,
                conversation_focused: false,
                conversation_scroll: ScrollHandle::new(),
                conversations: BTreeMap::new(),
                whisper_unread: BTreeMap::new(),
            },
            _input_subscriptions: input_subscriptions,
            chrome: ChromeComponent {
                modal_frame: ModalFrame::load(&resources),
                button_frames: ButtonFrames::load(&resources),
                top_nav_background: load_top_nav_background(&resources),
                ui_assets: UiAssets::native(),
            },
        };
        if live_mode {
            let executor = cx.background_executor().clone();
            cx.spawn(async move |entity, cx| {
                loop {
                    executor.timer(Duration::from_millis(50)).await;
                    if entity.update(cx, |this, cx| this.poll_client(cx)).is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        this
    }

    pub(in crate::app::client) fn local_account(&self) -> Option<(&UiUser, &str)> {
        let channel = self.channels.active()?;
        let handle = channel.local_member_handle?;
        let user = channel.users.iter().find(|user| user.handle == handle)?;
        Some((user, channel.title.as_str()))
    }
}
