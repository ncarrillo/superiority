use super::*;

impl RosterComponent {
    fn set_hovered(&mut self, hovered: bool) {
        self.roster_hovered = hovered;
        if !hovered {
            self.roster_defer_started = None;
            if !self.pending_rosters.is_empty() {
                self.roster_flush_at = Some(Instant::now());
            }
        }
    }

    fn scroll_layer(
        &self,
        channels: &ChannelComponent,
        channel: Option<&ChannelState>,
        selected_user: Option<u32>,
        interactive: bool,
        assets: &UiAssets,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        let now = Instant::now();
        let animating = interactive
            && channel.is_some_and(|channel| {
                self.roster.animation.as_ref().is_some_and(|animation| {
                    animation.scope == channel.id && animation.is_running(now)
                })
            });
        let rows = if animating || !interactive {
            self.row_slots(channel, selected_user, interactive, assets, cx)
        } else {
            Vec::new()
        };
        let mut layer = ui_roster::list_layer(if interactive {
            "roster-scroll"
        } else {
            "roster-scroll-snapshot"
        });
        if interactive {
            layer = layer
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.roster.roster_input.focus(window, cx);
                        this.composer.composer_focused = false;
                        this.join.join_focused = false;
                        this.roster.roster.focused = true;
                        cx.notify();
                    }),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set_selected_user(None);
                    cx.notify();
                }))
                .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                    if this.roster.roster.animation.take().is_some() {
                        cx.notify();
                    }
                }))
                .on_hover(cx.listener(|this, hovered, _, cx| {
                    this.roster.set_hovered(*hovered);
                    cx.notify();
                }));
            if animating {
                let scroll = self.roster.scroll.0.borrow().base_handle.clone();
                layer
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .children(rows)
            } else {
                let item_count = channels.active().map_or(0, |channel| {
                    filtered_roster_count(&channel.users, &self.roster.filter)
                });
                layer.child(
                    uniform_list(
                        "roster-users",
                        item_count,
                        cx.processor(|this, range: Range<usize>, _, cx| {
                            let users = this
                                .channels
                                .active()
                                .map(|channel| {
                                    filtered_roster_range(
                                        &channel.users,
                                        &this.roster.roster.filter,
                                        range,
                                    )
                                })
                                .unwrap_or_default();
                            users
                                .into_iter()
                                .map(|user| {
                                    let selected = this.selected_user() == Some(user.handle);
                                    ui_roster::virtual_row_slot(
                                        this.roster.row(
                                            &user,
                                            selected,
                                            this.channels
                                                .active()
                                                .map_or_else(String::new, |channel| {
                                                    channel.title.clone()
                                                }),
                                            &this.chrome.ui_assets,
                                            cx,
                                        ),
                                        ROSTER_ROW_GAP,
                                    )
                                    .into_any_element()
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .size_full()
                    .track_scroll(&self.roster.scroll),
                )
            }
        } else {
            let scroll = self.roster.scroll.0.borrow().base_handle.clone();
            layer.child(
                div()
                    .relative()
                    .top(scroll.offset().y)
                    .flex()
                    .flex_col()
                    .w_full()
                    .flex_shrink_0()
                    .children(rows),
            )
        }
    }

    fn header(
        &self,
        channel: Option<&ChannelState>,
        cx: &mut Context<SuperiorityView>,
    ) -> ui_roster::RosterHeader {
        let (title, total, filtered, complete) =
            channel.map_or(("No channel".to_owned(), 0, 0, true), |channel| {
                let filtered = filtered_roster_count(&channel.users, &self.roster.filter);
                (
                    channel.title.clone(),
                    channel.users.len(),
                    filtered,
                    channel.roster_complete,
                )
            });
        let availability = if complete {
            ui_roster::RosterAvailability::Online
        } else {
            ui_roster::RosterAvailability::Loading
        };
        let model = ui_roster::RosterHeaderModel::new(
            title,
            total,
            filtered,
            &self.roster.filter,
            self.roster.focused,
            availability,
        );
        let heading_color = model.heading_color(self.roster.focused);
        let header_id = channel.map_or(usize::MAX, |channel| channel.id as usize);
        let header = ui_roster::RosterHeader::new(
            format!("roster-header-{header_id}"),
            model.heading,
            model.count,
            heading_color,
        )
        .on_focus(cx.listener(|this, _, window, cx| {
            this.roster.roster_input.focus(window, cx);
            this.composer.composer_focused = false;
            this.join.join_focused = false;
            this.roster.roster.focused = true;
            cx.notify();
        }));
        if self.roster.filter.is_empty() {
            header
        } else {
            header.on_clear(cx.listener(|this, _, _, cx| {
                this.roster.roster_input.clear();
                this.set_roster_filter(String::new());
                cx.stop_propagation();
                cx.notify();
            }))
        }
    }

    pub(in crate::app::client) fn panel(
        &self,
        channels: &ChannelComponent,
        selected_user: Option<u32>,
        assets: &UiAssets,
        cx: &mut Context<SuperiorityView>,
    ) -> ui_workspace::ChannelRoster {
        let now = Instant::now();
        let mut panel = ui_workspace::ChannelRoster::new(
            self.header(channels.active(), cx),
            self.scroll_layer(channels, channels.active(), selected_user, true, assets, cx),
        );
        if let Some(transition) = &channels.channel_transition {
            panel = panel.outgoing(
                self.header(transition.outgoing.as_ref(), cx),
                self.scroll_layer(
                    channels,
                    transition.outgoing.as_ref(),
                    transition.outgoing_selected_user,
                    false,
                    assets,
                    cx,
                ),
                channels.transition_progress(now),
            );
        }
        panel
            .overlay(
                div()
                    .absolute()
                    .right(px(4.0))
                    .top(px(4.0))
                    .size(px(1.0))
                    .opacity(0.001)
                    .child(self.roster_input.element()),
            )
            .focused(self.roster.focused)
    }
}
