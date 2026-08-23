use super::*;

/// everything that belongs to one product's session.
///
/// Every entered game stays connected and one is focused at a time, so the
/// focused product's state lives here in [`SuperiorityView::session`] and the
/// rest wait in [`SuperiorityView::suspended`]. Switching is a swap of this
/// whole struct. Common runtime state stays directly on the session; the
/// product-only presentation state is carried by [`ProductUiState`].
///
/// What is *not* here is shared: chrome, settings, updates, warnings, overlays,
/// and the games list itself, which is the switcher.
///
/// Every playable product gets one of these at startup and every one of them
/// connects. Taking a card does not connect anything — it chooses which
/// session you are looking at, and you can go back to the games list and choose
/// another whenever you like.
pub(in crate::app::client) struct ProductSession {
    /// this product's worker. A worker is spawned for one product and stays
    /// that product's for its life — `ClientCommand::Connect` carries no
    /// product — so the handles belong to the session, not to the shared
    /// runtime. They were on `ClientRuntime` while `StarCraft II` was the only
    /// game that could connect, which meant every card's CONNECT reached
    /// `StarCraft II`'s worker.
    pub(in crate::app::client) commands: Option<Sender<ClientCommand>>,
    pub(in crate::app::client) events: Option<Receiver<ClientEvent>>,
    /// keeps this session's text inputs talking to the view.
    ///
    /// Without these the inputs still take focus and still take keystrokes —
    /// they update their own state and nothing repaints, so the field looks
    /// dead: no text, no caret. Every session needs its own, because every
    /// session builds its own inputs.
    pub(in crate::app::client) _input_subscriptions: Vec<Subscription>,
    /// who this product's session signed in as, and through which region. The
    /// numeric id is the identity boundary; the BattleTag is display text.
    pub(in crate::app::client) account_id: Option<u64>,
    pub(in crate::app::client) account_battle_tag: Option<String>,
    pub(in crate::app::client) account_region: Option<u32>,
    pub(in crate::app::client) connection: ConnectionComponent,
    /// Friends and one-to-one conversations have the same SC2 presentation in
    /// every Battle.net product that can supply them. Product adapters remain
    /// responsible for translating their own wire identities into this state.
    pub(in crate::app::client) social: SocialComponent,
    pub(in crate::app::client) ui: ProductUiState,
}

pub(in crate::app::client) enum ProductUiState {
    Sc2(Sc2SessionUi),
    Scr(ScrSessionUi),
    Wc3(Wc3SessionUi),
}

/// UI state whose concepts exist specifically in StarCraft II.
pub(in crate::app::client) struct Sc2SessionUi {
    pub(in crate::app::client) composer: ComposerComponent,
    pub(in crate::app::client) join: JoinComponent,
    pub(in crate::app::client) channels: ChannelComponent,
    pub(in crate::app::client) chat: ChatComponent,
    pub(in crate::app::client) roster: RosterComponent,
}

impl ProductSession {
    #[must_use]
    pub(in crate::app::client) const fn is_sc2(&self) -> bool {
        matches!(self.ui, ProductUiState::Sc2(_))
    }

    #[must_use]
    pub(in crate::app::client) const fn scr(&self) -> Option<&ScrSessionUi> {
        match &self.ui {
            ProductUiState::Scr(ui) => Some(ui),
            ProductUiState::Sc2(_) | ProductUiState::Wc3(_) => None,
        }
    }

    #[must_use]
    pub(in crate::app::client) const fn scr_mut(&mut self) -> Option<&mut ScrSessionUi> {
        match &mut self.ui {
            ProductUiState::Scr(ui) => Some(ui),
            ProductUiState::Sc2(_) | ProductUiState::Wc3(_) => None,
        }
    }

    #[must_use]
    pub(in crate::app::client) const fn wc3(&self) -> Option<&Wc3SessionUi> {
        match &self.ui {
            ProductUiState::Wc3(ui) => Some(ui),
            ProductUiState::Sc2(_) | ProductUiState::Scr(_) => None,
        }
    }

    #[must_use]
    pub(in crate::app::client) const fn wc3_mut(&mut self) -> Option<&mut Wc3SessionUi> {
        match &mut self.ui {
            ProductUiState::Wc3(ui) => Some(ui),
            ProductUiState::Sc2(_) | ProductUiState::Scr(_) => None,
        }
    }
}

/// migration bridge for SC2 feature modules. Those modules are only dispatched
/// from the SC2 surface; new product-neutral code must use `is_sc2`, `scr`, or
/// an explicit `ProductUiState` match instead of relying on this dereference.
impl std::ops::Deref for ProductSession {
    type Target = Sc2SessionUi;

    fn deref(&self) -> &Self::Target {
        match &self.ui {
            ProductUiState::Sc2(ui) => ui,
            ProductUiState::Scr(_) => {
                panic!("SC2 UI state requested from a Remastered product session")
            }
            ProductUiState::Wc3(_) => {
                panic!("SC2 UI state requested from a Reforged product session")
            }
        }
    }
}

impl std::ops::DerefMut for ProductSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.ui {
            ProductUiState::Sc2(ui) => ui,
            ProductUiState::Scr(_) => {
                panic!("SC2 UI state requested from a Remastered product session")
            }
            ProductUiState::Wc3(_) => {
                panic!("SC2 UI state requested from a Reforged product session")
            }
        }
    }
}

/// brings `product` to the front, parking whatever was focused. Returns `false`
/// when there is no session to bring — nothing is torn down and the focus does
/// not move, because a half-applied switch is worse than none.
///
/// A parked session is still connected: its worker keeps running and its events
/// keep being applied, which is the point of parking it rather than closing it.
///
/// Generic over the session only so it can be exercised without a `Window`;
/// [`SuperiorityView::focus_product`] is the one caller.
pub(in crate::app::client) fn swap_focus<S>(
    focused: &mut Product,
    session: &mut S,
    suspended: &mut BTreeMap<Product, S>,
    product: Product,
) -> bool {
    if product == *focused {
        return true;
    }
    let Some(incoming) = suspended.remove(&product) else {
        return false;
    };
    let outgoing = std::mem::replace(session, incoming);
    suspended.insert(*focused, outgoing);
    *focused = product;
    true
}

impl SuperiorityView {
    pub(in crate::app::client) fn focus_product(&mut self, product: Product) -> bool {
        swap_focus(
            &mut self.focused,
            &mut self.session,
            &mut self.suspended,
            product,
        )
    }

    /// reads a product session without changing which realm is in front.
    #[must_use]
    pub(in crate::app::client) fn product_session(
        &self,
        product: Product,
    ) -> Option<&ProductSession> {
        if product == self.focused {
            Some(&self.session)
        } else {
            self.suspended.get(&product)
        }
    }

    /// mutates a product session without using focus as a temporary pointer.
    /// The game picker needs this when refreshing every connection together.
    pub(in crate::app::client) fn product_session_mut(
        &mut self,
        product: Product,
    ) -> Option<&mut ProductSession> {
        if product == self.focused {
            Some(&mut self.session)
        } else {
            self.suspended.get_mut(&product)
        }
    }

    /// whether this product has a session at all, focused or parked.
    #[expect(
        dead_code,
        reason = "for the games list's per-card state once a second product can connect"
    )]
    pub(in crate::app::client) fn has_session(&self, product: Product) -> bool {
        self.focused == product || self.suspended.contains_key(&product)
    }
}

impl ProductSession {
    /// a session for a product other than the one the app opened on: empty,
    /// disconnected, and holding its own worker handles.
    ///
    /// `StarCraft II`'s session is built in [`super::controller`] with restored
    /// tabs and preview fixtures; nothing here restores anything, because
    /// nothing has been in this product's session before.
    pub(in crate::app::client) fn blank(
        product: Product,
        commands: Option<Sender<ClientCommand>>,
        events: Option<Receiver<ClientEvent>>,
        cx: &mut Context<SuperiorityView>,
    ) -> Self {
        let composer = ui_text_input::TextInput::new("Press Enter to chat", cx);
        let roster_input = ui_text_input::TextInput::new("Filter members", cx);
        let conversation_input = ui_text_input::TextInput::new("Write a message", cx);
        let subscriptions = std::iter::once(&composer)
            .chain(std::iter::once(&roster_input))
            .chain(std::iter::once(&conversation_input))
            .map(|input| {
                input.subscribe(cx, |this, cx| {
                    this.sync_text_inputs(cx);
                    cx.notify();
                })
            })
            .collect();
        Self {
            commands,
            events,
            _input_subscriptions: subscriptions,
            account_id: None,
            account_battle_tag: None,
            account_region: None,
            connection: ConnectionComponent {
                stage: ConnectionStage::Disconnected,
                error: None,
                starting: false,
                signed_out: false,
                sign_out_requested: false,
                // the card carries this product's progress; the dialog belongs
                // to whatever the app opened on
                dialog_visible: false,
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
            ui: match product {
                Product::Remastered => {
                    ProductUiState::Scr(ScrSessionUi::new(composer, roster_input))
                }
                Product::Warcraft3 => {
                    ProductUiState::Wc3(Wc3SessionUi::new(composer, roster_input))
                }
                Product::StarCraft2 => ProductUiState::Sc2(Sc2SessionUi {
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
                        public_channels: BTreeMap::new(),
                        groups: BTreeMap::new(),
                        remembered_group_names: BTreeMap::new(),
                        member_groups: BTreeSet::new(),
                        group_search: Vec::new(),
                        invitations: Vec::new(),
                        next_invitation_id: 0,
                        channel_conferences: BTreeMap::new(),
                        conference_members: BTreeMap::new(),
                        directory_complete: false,
                    },
                    channels: ChannelComponent {
                        navigation: ui_workspace::NavigationState::default(),
                        tabs: Vec::new(),
                        next_tab_id: 1,
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
                        // SC2's portrait-table registry. Remastered has its own
                        // ToonProfile/Url avatar pipeline in ProductUiState::Scr.
                        portraits: PortraitRegistry::load(&platform::resource_directory()),
                    },
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focusing_a_product_with_no_session_changes_nothing() {
        // the alternative is tearing the focused session down and only then
        // finding there is nothing to put in its place
        let mut focused = Product::StarCraft2;
        let mut session = "sc2";
        let mut suspended = BTreeMap::new();

        assert!(!swap_focus(
            &mut focused,
            &mut session,
            &mut suspended,
            Product::Remastered
        ));
        assert_eq!(focused, Product::StarCraft2);
        assert_eq!(session, "sc2");
        assert!(suspended.is_empty());
    }

    #[test]
    fn switching_parks_the_old_session_rather_than_dropping_it() {
        // a parked session stays connected; losing it here would silently end
        // a game you are still signed in to
        let mut focused = Product::StarCraft2;
        let mut session = "sc2";
        let mut suspended = BTreeMap::from([(Product::Remastered, "scr")]);

        assert!(swap_focus(
            &mut focused,
            &mut session,
            &mut suspended,
            Product::Remastered
        ));
        assert_eq!(focused, Product::Remastered);
        assert_eq!(session, "scr");
        assert_eq!(suspended.get(&Product::StarCraft2), Some(&"sc2"));
        assert!(!suspended.contains_key(&Product::Remastered));

        // and back again, with nothing lost either way
        assert!(swap_focus(
            &mut focused,
            &mut session,
            &mut suspended,
            Product::StarCraft2
        ));
        assert_eq!(session, "sc2");
        assert_eq!(suspended.get(&Product::Remastered), Some(&"scr"));
    }

    #[test]
    fn visiting_every_session_in_turn_puts_the_front_one_back() {
        // background products are drained by focusing each in turn, because
        // every handler writes to the focused session. if the focus were not
        // restored the user would be looking at a game they never chose.
        let mut focused = Product::StarCraft2;
        let mut session = "sc2";
        let mut suspended = BTreeMap::from([(Product::Remastered, "scr")]);

        let front = focused;
        swap_focus(
            &mut focused,
            &mut session,
            &mut suspended,
            Product::Remastered,
        );
        swap_focus(&mut focused, &mut session, &mut suspended, front);

        assert_eq!(focused, Product::StarCraft2);
        assert_eq!(session, "sc2", "the front session came back intact");
        assert_eq!(suspended.get(&Product::Remastered), Some(&"scr"));
    }

    #[test]
    fn focusing_the_product_already_in_front_is_a_no_op() {
        let mut focused = Product::StarCraft2;
        let mut session = "sc2";
        let mut suspended: BTreeMap<Product, &str> = BTreeMap::new();
        assert!(swap_focus(
            &mut focused,
            &mut session,
            &mut suspended,
            Product::StarCraft2
        ));
        assert_eq!(focused, Product::StarCraft2);
        assert!(suspended.is_empty());
    }
}
