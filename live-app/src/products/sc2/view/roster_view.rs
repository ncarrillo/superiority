use super::*;

impl LiveView {
    pub(super) fn roster_user(&self, member: &RosterMember) -> RosterUser {
        let presence = presence(&member.presence);
        let sender = Sender {
            handle: member.handle,
            name: member.name.clone(),
            clan_tag: member.clan_tag.clone(),
        };
        let portrait = member.portrait.as_ref().and_then(|value| {
            (value.t < 17 && value.o < 36).then(|| Portrait::Atlas {
                image: self
                    .asset(&format!("ui/portraits/atlas-{:02}.png", value.t))
                    .into(),
                cell: value.o,
                columns: 6,
                cell_size: 38.0,
            })
        });
        RosterUser {
            handle: member.handle,
            name: sender.display_name(),
            presence_id: None,
            presence,
            presence_label: presence.label().to_owned(),
            presence_icon: self.ui_assets.presence_icon(presence),
            portrait,
            clan_tag: member.clan_tag.clone(),
            clan_is_local: member.is_local,
            tone: member.tone,
            dimmed: presence == PresenceKind::Away,
        }
    }

    pub(super) fn roster_row(
        &self,
        channel: &str,
        member: &RosterMember,
        cx: &mut Context<Self>,
    ) -> roster::RosterRow {
        let user = self.roster_user(member);
        let selected = self.workspace.roster.selection(channel) == Some(member.handle);
        let handle = member.handle;
        let channel = channel.to_owned();
        let hover_group = format!("live-roster-user-{channel}-{handle}");
        let tooltip_channel = self
            .channel_summary(Some(&channel))
            .map_or_else(|| channel.clone(), ChannelSummary::label);
        let selection_channel = channel.clone();
        roster::RosterRow::new(
            format!("live-roster-user-{channel}-{handle}"),
            hover_group,
            user,
            tooltip_channel,
            selected,
            self.ui_assets.clone(),
        )
        .on_click(cx.listener(move |view, _, _, cx| {
            view.workspace
                .roster
                .set_selection(selection_channel.clone(), Some(handle));
            cx.stop_propagation();
            cx.notify();
        }))
    }

    pub(super) fn channel_summary(&self, key: Option<&str>) -> Option<&ChannelSummary> {
        key.and_then(|key| self.channels.iter().find(|channel| channel.key == key))
    }

    pub(super) fn roster_header(
        &self,
        key: Option<&str>,
        interactive: bool,
        cx: &mut Context<Self>,
    ) -> roster::RosterHeader {
        let channel = self.channel_summary(key);
        let title = channel.map_or_else(|| "No channel".to_owned(), ChannelSummary::label);
        let snapshot = (!interactive)
            .then_some(self.channel_transition.as_ref())
            .flatten()
            .filter(|transition| key == Some(transition.outgoing.as_str()))
            .map(|transition| &transition.outgoing_data);
        let members = snapshot
            .or_else(|| key.and_then(|channel| self.channel_data.get(channel)))
            .map(|data| data.roster.as_slice())
            .unwrap_or_default();
        let total = if snapshot.is_some() {
            members.len()
        } else {
            channel.map_or(members.len(), |channel| channel.member_count)
        };
        let filter = self.roster_filter(key);
        let filtered = members
            .iter()
            .filter(|member| roster_member_matches(member, filter))
            .count();
        let focused = interactive && self.workspace.roster.focused;
        let availability = match self.feed_state.as_str() {
            "online" => roster::RosterAvailability::Online,
            "reconnecting" => roster::RosterAvailability::Loading,
            _ => roster::RosterAvailability::Offline,
        };
        let model =
            roster::RosterHeaderModel::new(title, total, filtered, filter, focused, availability);
        let heading_color = model.heading_color(focused);
        let header_id = format!(
            "live-roster-header-{}-{}",
            key.unwrap_or("none"),
            if interactive { "active" } else { "transition" }
        );
        let mut header =
            roster::RosterHeader::new(header_id, model.heading, model.count, heading_color);
        if interactive {
            header = header.on_focus(cx.listener(|view, _, window, cx| {
                view.focus_handle.focus(window, cx);
                view.workspace.roster.focused = true;
                cx.notify();
            }));
        }
        if interactive && !filter.is_empty() {
            header = header.on_clear(cx.listener(|view, _, _, cx| {
                view.set_roster_filter(String::new());
                cx.stop_propagation();
                cx.notify();
            }));
        }
        header
    }

    pub(super) fn roster_row_slots(
        &self,
        channel: &str,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let members = self
            .channel_data
            .get(channel)
            .map(|data| {
                presented_roster_members(
                    &self.channels,
                    &self.channel_data,
                    channel,
                    &data.roster,
                    self.roster_filter(Some(channel)),
                )
            })
            .unwrap_or_default();
        let now = js_sys::Date::now();
        let animation = self
            .workspace
            .roster
            .animation
            .as_ref()
            .filter(|animation| animation.scope == channel);
        roster::animated_rows(
            members,
            animation,
            now,
            |member| member.handle,
            ROSTER_ROW_GAP,
            |member, motion| {
                let selected = self.workspace.roster.selection(channel) == Some(member.handle);
                let row = if motion == roster::RowMotion::Removed {
                    roster::static_row(&self.roster_user(member), &self.ui_assets, selected)
                        .into_any_element()
                } else {
                    self.roster_row(channel, member, cx).into_any_element()
                };
                row
            },
        )
    }

    pub(super) fn roster_rows(
        &self,
        key: Option<&str>,
        interactive: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let mut rows = roster::list_layer(if interactive {
            "live-roster"
        } else {
            "live-roster-transition"
        });
        if interactive {
            rows = rows
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _, window, cx| {
                        view.focus_handle.focus(window, cx);
                        view.workspace.roster.focused = true;
                        cx.notify();
                    }),
                )
                .on_scroll_wheel(cx.listener(|view, _, _, cx| {
                    if view.workspace.roster.animation.take().is_some() {
                        cx.notify();
                    }
                }));
        }
        if key.is_some() {
            if let Some(channel) = key {
                let animating = interactive
                    && self
                        .workspace
                        .roster
                        .animation
                        .as_ref()
                        .is_some_and(|animation| {
                            animation.scope == channel && animation.is_running(js_sys::Date::now())
                        });
                if animating {
                    let scroll = self.workspace.roster.scroll.0.borrow().base_handle.clone();
                    return rows
                        .overflow_y_scroll()
                        .track_scroll(&scroll)
                        .children(self.roster_row_slots(channel, cx))
                        .vertical_scrollbar_for(&scroll, window, cx);
                }
                let snapshot_members = (!interactive)
                    .then_some(self.channel_transition.as_ref())
                    .flatten()
                    .filter(|transition| transition.outgoing == channel)
                    .map(|transition| transition.outgoing_data.roster.clone())
                    .unwrap_or_default();
                let filter = self.roster_filter(Some(channel)).to_owned();
                let item_count = if interactive {
                    self.channel_data.get(channel).map_or(0, |data| {
                        data.roster
                            .iter()
                            .filter(|member| roster_member_matches(member, &filter))
                            .count()
                    })
                } else {
                    snapshot_members
                        .iter()
                        .filter(|member| roster_member_matches(member, &filter))
                        .count()
                };
                let channel = channel.to_owned();
                let list_id = format!(
                    "live-roster-list-{channel}-{}",
                    if interactive { "active" } else { "transition" }
                );
                let mut list = uniform_list(
                    list_id,
                    item_count,
                    cx.processor(move |view, range: Range<usize>, _, cx| {
                        let members = if interactive {
                            view.channel_data
                                .get(&channel)
                                .map(|data| {
                                    filtered_roster_range(
                                        &view.channels,
                                        &view.channel_data,
                                        &channel,
                                        &data.roster,
                                        view.roster_filter(Some(&channel)),
                                        range,
                                    )
                                })
                                .unwrap_or_default()
                        } else {
                            filtered_roster_range(
                                &view.channels,
                                &view.channel_data,
                                &channel,
                                &snapshot_members,
                                &filter,
                                range,
                            )
                        };
                        members
                            .into_iter()
                            .map(|member| {
                                let row = if interactive {
                                    view.roster_row(&channel, &member, cx).into_any_element()
                                } else {
                                    let user = view.roster_user(&member);
                                    let selected = view.workspace.roster.selections.get(&channel)
                                        == Some(&member.handle);
                                    roster::static_row(&user, &view.ui_assets, selected)
                                        .into_any_element()
                                };
                                roster::virtual_row_slot(row, ROSTER_ROW_GAP).into_any_element()
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .size_full();
                let scroll = if interactive {
                    Some(&self.workspace.roster.scroll)
                } else {
                    self.channel_transition
                        .as_ref()
                        .map(|transition| &transition.outgoing_roster_scroll)
                };
                if let Some(scroll) = scroll {
                    list = list.track_scroll(scroll);
                }
                rows = rows.child(list);
            }
        } else {
            rows = rows
                .items_center()
                .justify_center()
                .font_family(FONT_INTERFACE)
                .text_size(px(11.0))
                .text_color(rgb(MUTED))
                .child("NO ROSTER SNAPSHOT");
        }
        if interactive {
            let scroll = self.workspace.roster.scroll.0.borrow().base_handle.clone();
            rows.vertical_scrollbar_for(&scroll, window, cx)
        } else {
            rows
        }
    }

    pub(super) fn roster_panel(
        &self,
        width: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> workspace::ChannelRoster {
        let outgoing_interactive = self.transition_progress().is_none();
        let mut panel = workspace::ChannelRoster::new(
            self.roster_header(self.selected.as_deref(), true, cx),
            self.roster_rows(self.selected.as_deref(), true, window, cx),
        )
        .width(width);
        if let Some(transition) = &self.channel_transition {
            panel = panel.outgoing(
                self.roster_header(Some(&transition.outgoing), outgoing_interactive, cx),
                self.roster_rows(Some(&transition.outgoing), outgoing_interactive, window, cx),
                self.transition_progress(),
            );
        }
        panel
    }
}
