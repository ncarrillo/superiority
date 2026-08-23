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
        // the same file the worker writes to, so a UI question and a protocol
        // question can be answered from one log
        if let Some(path) = std::env::var_os("SUPERIORITY_TRACE_FILE")
            && let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
        {
            use std::io::Write as _;
            let _ = writeln!(file, "superiority: {message}");
        }
    }

    pub(in crate::app::client) fn new(
        cx: &mut Context<Self>,
        app_menu_events: Receiver<AppMenuCommand>,
        app_menu_target: NativeAppMenuTarget,
    ) -> Self {
        let resources = platform::resource_directory();
        let preview_toast = std::env::var_os("SUPERIORITY_PREVIEW_TOAST").is_some();
        let preview_update = std::env::var_os("SUPERIORITY_PREVIEW_UPDATE").is_some();
        // seeds a catalogue so the /join popup has something to autocomplete
        // without a live session behind it
        let preview_join = std::env::var_os("SUPERIORITY_PREVIEW_JOIN").is_some();
        // the game screens are dressed in design-time data, so they run instead
        // of a session rather than alongside one
        let preview_states = std::env::var_os("SUPERIORITY_PREVIEW_GAMES").is_some();
        let preview_picker = std::env::var_os("SUPERIORITY_PREVIEW_PICKER").is_some();
        let preview_games = preview_picker || preview_states;
        // the shared modal demo: the frame, the title, and the motion, with
        // fixture data behind them
        let preview_modal = std::env::var_os("SUPERIORITY_PREVIEW_MODAL").is_some();
        let preview_scr_social = std::env::var_os("SUPERIORITY_PREVIEW_SCR_SOCIAL").is_some();
        let games_screen = if preview_modal || preview_scr_social {
            // the demo owns the window, so the front door stays out of its way
            None
        } else if preview_states {
            Some(GamesScreen::States)
        } else {
            // the picker is the front door: a live session opens on it and
            // hands over to the client once you have gone through it
            Some(GamesScreen::Picker)
        };
        // a way to see the picker as somebody who does not own everything:
        // `SUPERIORITY_PREVIEW_OWNED=S2,W3` leaves Remastered on the shelf.
        // unset means the account has said nothing yet, which offers all three.
        let owned_games = std::env::var("SUPERIORITY_PREVIEW_OWNED")
            .ok()
            .map(|owned| {
                owned
                    .split(',')
                    .map(str::trim)
                    .filter(|program| !program.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            });
        let live_mode = !preview_toast
            && !preview_update
            && !preview_join
            && !preview_games
            && !preview_modal
            && !preview_scr_social;
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
        // one publisher, shared by every product's worker: `StarCraft II` taps
        // its own chat, and the classic products tap theirs through
        // `begin_classic_session`. A feed carries all of them at once.
        let publisher = uplink::spawn(uplink.clone(), remembered_group_names.clone());
        let (commands, events) = if live_mode {
            let client = spawn_client(Product::StarCraft2, Box::new(publisher.clone()));
            if !startup_connection_pending {
                let _ = client.commands.send(ClientCommand::Connect {
                    force_interactive: false,
                    expected_account_id: None,
                    expected_battle_tag: None,
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
        let roster_input = ui_text_input::TextInput::new("Filter members", cx);
        let conversation_input = ui_text_input::TextInput::new("Write a message", cx);
        let input_subscriptions = [&composer, &roster_input, &conversation_input]
            .into_iter()
            .map(|input| {
                input.subscribe(cx, |this, cx| {
                    this.sync_text_inputs(cx);
                    cx.notify();
                })
            })
            .collect();
        let mut this = Self {
            session: ProductSession {
                commands,
                events,
                _input_subscriptions: input_subscriptions,
                account_id: None,
                account_battle_tag: None,
                account_region: None,
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
                    whisper_targets: BTreeMap::new(),
                },
                ui: ProductUiState::Sc2(Sc2SessionUi {
                    composer: ComposerComponent {
                        composer_focused: false,
                        composer,
                        command_line: String::new(),
                        command_selected: 0,
                        command_dismissed: false,
                        command_closing: None,
                        command_close_epoch: 0,
                        command_epoch: 0,
                        command_focused: false,
                        command_cursor: 0,
                        party_scope: false,
                    },
                    join: JoinComponent {
                        awaiting_joins: Vec::new(),
                        group_search_due: None,
                        public_channels: if preview_join {
                            preview::CATALOGUE
                                .iter()
                                .map(|(identifier, name, _)| (*identifier, (*name).to_owned()))
                                .collect()
                        } else {
                            BTreeMap::new()
                        },
                        groups: if preview_join {
                            let (club_id, name, members, online) = preview::CATALOGUE_GROUP;
                            BTreeMap::from([(
                                club_id,
                                UiGroupSummary {
                                    name: name.to_owned(),
                                    private: false,
                                    kind: 1,
                                    category: 1,
                                    member_count: Some(members),
                                    online: Some(online),
                                },
                            )])
                        } else {
                            BTreeMap::new()
                        },
                        remembered_group_names,
                        member_groups: BTreeSet::new(),
                        group_search: if preview_join {
                            vec![preview::CATALOGUE_GROUP.0]
                        } else {
                            Vec::new()
                        },
                        invitations: if preview_toast {
                            {
                                vec![UiInvitation {
                                    id: 1,
                                    kind: InvitationKind::Group { club_id: 5322 },
                                    inviter: Some("Sledgehammer".to_owned()),
                                    destination: Some("<MDGTN> Blood Nation".to_owned()),
                                    closing: false,
                                }]
                            }
                        } else {
                            Default::default()
                        },
                        next_invitation_id: if preview_toast { 2 } else { 1 },
                        channel_conferences: if preview_join {
                            preview::CATALOGUE
                                .iter()
                                .map(|(identifier, _, _)| {
                                    (*identifier, vec![u32::from(*identifier)])
                                })
                                .collect()
                        } else {
                            BTreeMap::new()
                        },
                        conference_members: if preview_join {
                            preview::CATALOGUE
                                .iter()
                                .map(|(identifier, _, online)| (u32::from(*identifier), *online))
                                .collect()
                        } else {
                            BTreeMap::new()
                        },
                        directory_complete: preview_join,
                    },
                    channels: ChannelComponent {
                        navigation: ui_workspace::NavigationState::default(),
                        tabs: if live_mode {
                            restored_tabs
                        } else if preview_join {
                            vec![ChannelState::fixture(), ChannelState::fixture_party(1)]
                        } else {
                            vec![ChannelState::fixture()]
                        },
                        next_tab_id: if live_mode { next_live_tab_id } else { 2 },
                        active_tab: 0,
                        tab_close: None,
                        hovered_tab: None,
                        tab_selection_started: Some(Instant::now()),
                        channel_transition: None,
                        chat_entry_reveal: None,
                    },
                    chat: ChatComponent {
                        transcript: ui_workspace::TranscriptState::default(),
                        expanded_digest: None,
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
                }),
            },
            focused: Product::StarCraft2,
            // filled in below: every other playable product gets a session and
            // a worker of its own, all connecting at once
            suspended: BTreeMap::new(),
            focus_handle: cx.focus_handle(),
            runtime: ClientRuntime {
                app_menu_events,
                _app_menu_target: app_menu_target,
                live_mode,
                legacy_connect_dialog: std::env::var_os("SUPERIORITY_LEGACY_CONNECT").is_some(),
                authenticator: None,
                uplink,
                publisher,
                live_auth_notified: false,
                authoritative_account_id: None,
                authoritative_battle_tag: None,
                authoritative_region: None,
                provisioned: BTreeSet::new(),
                connect_queue: VecDeque::new(),
                // StarCraft II's Connect is sent above rather than queued, so
                // the queue is told it is in flight and waits for it. If the
                // update check deferred it there is nothing to wait for, and
                // the next product goes first rather than waiting on a sign-in
                // that has not been asked for yet.
                connecting: (live_mode && !startup_connection_pending)
                    .then_some(Product::StarCraft2),
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
            overlays: OverlayComponent {
                active: None,
                closing: false,
                epoch: 0,
            },
            settings: SettingsComponent {
                show_timestamps,
                show_membership,
                live_enabled,
                backgrounds: Product::ALL
                    .into_iter()
                    .map(|product| (product, preferences.background(product).path))
                    .collect(),
                active_settings_page: 0,
                settings_page_transition: None,
                settings_tooltip: None,
                checkbox_animations: [None; 3],
                background_scroll: ScrollHandle::new(),
                privacy_scroll: ScrollHandle::new(),
            },
            _input_subscriptions: Vec::new(),
            games: {
                let mut games =
                    GamesComponent::new(games_screen, owned_games, platform::reduce_motion());
                games.rehearsing = std::env::var_os("SUPERIORITY_PREVIEW_ENTER").is_some();
                games.live_mode = live_mode;
                games
            },
            modal_preview: ModalPreviewComponent::new(preview_modal, platform::reduce_motion()),
            chrome: ChromeComponent {
                modal_frame: ModalFrame::load(&resources),
                button_frames: ButtonFrames::load(&resources),
                top_nav_background: load_top_nav_background(&resources),
                ui_assets: Sc2Assets::load(
                    &superiority_ui::foundation::assets::NativeAssetResolver,
                ),
                modal_assets_warming: true,
                modal_warmup_started: false,
                modal_textures: ui_shared_modal::ModalTextures::default(),
            },
        };
        if live_mode {
            let executor = cx.background_executor().clone();
            cx.spawn(async move |entity, cx| {
                loop {
                    executor.timer(Duration::from_millis(50)).await;
                    if entity
                        .update(cx, super::super::state::SuperiorityView::poll_client)
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        }
        this.start_other_products(cx);
        if preview_scr_social {
            this.install_scr_social_preview(cx);
        }
        this
    }

    fn install_scr_social_preview(&mut self, cx: &mut Context<Self>) {
        use superiority_core::games::scr::{
            chat::{
                ChatAttribute as ClassicAttribute, ChatChannel as ClassicChannel,
                ChatFriend as ClassicFriend, ChatUser as ClassicUser,
                FriendPresence as ClassicFriendPresence,
            },
            profile::Avatar as ClassicAvatar,
        };

        self.suspended.insert(
            Product::Remastered,
            ProductSession::blank(Product::Remastered, None, None, cx),
        );
        let _ = self.focus_product(Product::Remastered);
        self.session.connection.stage = ConnectionStage::Connected;
        self.session.account_battle_tag = Some("Commander#1234".into());
        let user = |name: &str, battle_tag: &str, avatar: &str| ClassicUser {
            name: name.into(),
            flags: Some(0),
            is_operator: false,
            avatar: Some(ClassicAvatar {
                image_url: None,
                id: Some(avatar.into()),
            }),
            attributes: vec![ClassicAttribute {
                name: "battle_tag".into(),
                value: battle_tag.into(),
            }],
        };
        self.apply_classic_channel(&ClassicChannel {
            channel_id: 1,
            name: "Public Chat 1".into(),
            display_name: Some("Public Chat 1".into()),
            is_public: true,
            users: vec![
                user("Commander", "Commander#1234", "avatar_terran_marine.jpg"),
                user("Darko2", "Darko#5678", "avatar_protoss_zealot.jpg"),
                user("Kerrigan", "Queen#9999", "avatar_zerg_queen.jpg"),
            ],
        });
        self.apply_classic_friends(vec![
            ClassicFriend {
                name: "Darko2".into(),
                presence: ClassicFriendPresence::Online,
                game: None,
                account_id: None,
            },
            ClassicFriend {
                name: "Kerrigan".into(),
                presence: ClassicFriendPresence::Offline,
                game: None,
                account_id: None,
            },
        ]);
        self.session.social.record_whisper(
            "Darko2".into(),
            "Ready for a game?".into(),
            false,
            "4:52 AM".into(),
        );
        self.overlays.active = Some(Overlay::Friends);
        self.overlays.closing = false;
    }

    /// Prepares every other protocol worker without connecting it. The
    /// authoritative SC2 session must return its license catalogue first; an
    /// unprovisioned product never gets as far as a sign-in request.
    fn start_other_products(&mut self, cx: &mut Context<Self>) {
        if !self.runtime.live_mode {
            return;
        }
        for product in Product::ALL {
            if product == self.focused || product.logon().is_none() {
                continue;
            }
            // every product taps the same Live feed; the classic tap is silent
            // until its session joins a channel and Live is enabled
            let client = spawn_client(product, Box::new(self.runtime.publisher.clone()));
            self.suspended.insert(
                product,
                ProductSession::blank(product, Some(client.commands), Some(client.events), cx),
            );
        }
    }

    /// Installs the authoritative account catalogue and queues only products
    /// licensed to it. Each protocol keeps its own rotating credential, but
    /// every worker must resolve that credential to the same numeric account.
    pub(in crate::app::client) fn adopt_authoritative_account(
        &mut self,
        account_id: u64,
        battle_tag: Option<String>,
        region: Option<u32>,
        games: Vec<String>,
    ) {
        self.runtime.authoritative_account_id = Some(account_id);
        self.runtime.authoritative_battle_tag = battle_tag;
        self.runtime.authoritative_region = region;
        self.runtime.provisioned = games
            .iter()
            .filter_map(|code| Product::from_code(code))
            .filter(|product| product.can_sign_in())
            .collect();
        // this is the signed launcher's retail-product result, not a broad
        // product-record guess. Install it atomically so the picker never
        // flashes an intermediate empty or beta-only product set.
        self.games.set_owned(games);

        self.runtime
            .connect_queue
            .retain(|product| self.runtime.provisioned.contains(product));
        if self
            .runtime
            .connecting
            .is_some_and(|product| !self.runtime.provisioned.contains(&product))
        {
            self.runtime.connecting = None;
        }
        if self.runtime.authoritative_account_id.is_none() {
            return;
        }
        for product in Product::ALL {
            if product == Product::StarCraft2
                || !self.runtime.provisioned.contains(&product)
                || self.runtime.connecting == Some(product)
                || self.runtime.connect_queue.contains(&product)
            {
                continue;
            }
            let Some(session) = self.suspended.get(&product) else {
                continue;
            };
            if session.connection.stage == ConnectionStage::Disconnected
                && session.connection.error.is_none()
            {
                self.runtime.connect_queue.push_back(product);
            }
        }
    }

    /// How many realms are still on their way back in: the one connecting now
    /// and everything queued behind it. The picker's REFRESH chip counts these
    /// down.
    pub(in crate::app::client) fn realms_in_flight(&self) -> usize {
        usize::from(self.runtime.connecting.is_some()) + self.runtime.connect_queue.len()
    }

    /// Sends the next queued sign-in once the one in flight is done, either way.
    ///
    /// "Done" is deliberately generous: a product that failed has still let go
    /// of the browser, and holding the queue behind it would leave every other
    /// product waiting on a game that is never going to connect.
    pub(in crate::app::client) fn advance_connect_queue(&mut self) {
        if let Some(product) = self.runtime.connecting {
            let session = if product == self.focused {
                Some(&self.session)
            } else {
                self.suspended.get(&product)
            };
            let settled = session.is_none_or(|session| {
                session.account_id.is_some() || session.connection.error.is_some()
            });
            if !settled {
                return;
            }
            self.runtime.connecting = None;
        }
        let Some(next) = self.runtime.connect_queue.pop_front() else {
            return;
        };
        let Some(expected_account_id) = self.runtime.authoritative_account_id else {
            self.runtime.connect_queue.push_front(next);
            return;
        };
        let expected_battle_tag = self.runtime.authoritative_battle_tag.clone();
        // a queued product can be restarting after an account-wide sign-out.
        // clear the sign-out gate before its worker emits the first Stage:
        // otherwise `handle_client_event` deliberately rejects every event
        // except Disconnected and the card stays on the signed-out placeholder
        // even though the protocol connected successfully underneath it.
        let commands = self.product_session_mut(next).and_then(|session| {
            session.connection.begin_attempt();
            session.commands.clone()
        });
        if let Some(commands) = commands {
            let _ = commands.send(ClientCommand::Connect {
                force_interactive: false,
                expected_account_id: Some(expected_account_id),
                expected_battle_tag,
                channels: Vec::new(),
            });
            self.runtime.connecting = Some(next);
        }
    }

    pub(in crate::app::client) fn local_account(&self) -> Option<(&UiUser, &str)> {
        if !self.session.is_sc2() {
            return None;
        }
        let channel = self.session.channels.active()?;
        let handle = channel.local_member_handle?;
        let user = channel.users.iter().find(|user| user.handle == handle)?;
        Some((user, channel.title.as_str()))
    }
}
