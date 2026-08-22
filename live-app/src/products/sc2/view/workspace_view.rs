use super::*;

impl LiveView {
    pub(super) fn top_navigation(&self, _: &Window, compact: bool, cx: &mut Context<Self>) -> Div {
        let now = js_sys::Date::now();
        let offsets = self
            .workspace
            .navigation
            .tabs
            .offsets(now, self.channels.len());
        let tabs = self
            .channels
            .iter()
            .enumerate()
            .map(|(index, channel)| {
                let key = channel.key.clone();
                let hover_key = key.clone();
                navigation::ChannelTab::new(format!("live-tab-{index}"), channel.label())
                    .active(self.selected.as_deref() == Some(key.as_str()))
                    .tone(match channel.kind.as_str() {
                        "party" => navigation::ChannelTabTone::Party,
                        "club" => navigation::ChannelTabTone::Group,
                        _ => navigation::ChannelTabTone::Standard,
                    })
                    .marquee_offset(self.workspace.navigation.tabs.name_offset(&key, now))
                    .drag_offset(offsets[index])
                    .dragged_travel(
                        self.workspace
                            .navigation
                            .tabs
                            .is_dragging(index)
                            .then(|| self.workspace.navigation.tabs.dragged_travel()),
                    )
                    .on_mouse_down(cx.listener(move |view, event, _, _| {
                        view.begin_tab_pointer(index, event);
                    }))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.click_tab(index, cx);
                    }))
                    .on_hover(
                        cx.listener(move |view, event: &navigation::TabHoverEvent, _, cx| {
                            view.set_tab_name_hover(
                                hover_key.clone(),
                                event.hovered,
                                event.travel,
                                cx,
                            );
                        }),
                    )
            })
            .collect();
        let tabs = navigation::ChannelTabs::new(tabs, self.ui_assets.clone()).leading(if compact {
            4.0
        } else {
            MARGIN
        });
        let tabs = if compact {
            tabs.compact(&self.workspace.navigation.scroll)
        } else {
            tabs
        };

        let (status, status_color) = if let Some(error) = &self.error {
            (compact_error(error), rgb(0xf2705c))
        } else if self.feed_state == "online" {
            ("LIVE".to_owned(), rgb(ONLINE))
        } else if self.feed_state == "reconnecting" {
            ("RECONNECTING".to_owned(), rgb(MUTED))
        } else if self.had_session {
            ("OFFLINE".to_owned(), rgb(MUTED))
        } else {
            ("CONNECTING".to_owned(), rgb(MUTED))
        };

        navigation::bar(Some(self.ui_assets.top_navigation_background.clone()))
            .child(tabs)
            .when(!compact, |bar| {
                bar.child(
                    div()
                        .absolute()
                        .right(px(14.0))
                        .top_0()
                        .h_full()
                        .flex()
                        .items_center()
                        .gap(px(7.0))
                        .font_family(FONT_NAVIGATION)
                        .text_size(px(10.0))
                        .text_color(status_color)
                        .child(div().size(px(7.0)).rounded_full().bg(status_color))
                        .child(status),
                )
            })
    }

    pub(super) fn stream_row(
        &self,
        item: &StreamItem,
        scope: u64,
        row: usize,
        interactive: bool,
    ) -> AnyElement {
        if interactive {
            chat::selectable_transcript_row(
                &stream_line(item, &self.ui_assets),
                true,
                scope,
                row,
                &self.workspace.transcript.selection,
                &self.ui_assets,
                chat::PartyMark::default(),
            )
            .with_animation(
                item.animation_id(),
                Animation::new(Duration::from_millis(350)),
                |row, delta| row.opacity(ease_in_out(delta)),
            )
            .into_any_element()
        } else {
            chat::transcript_row(&stream_line(item, &self.ui_assets), true, &self.ui_assets)
                .into_any_element()
        }
    }

    pub(super) fn transcript_view(
        &self,
        channel: Option<&str>,
        scrollable: bool,
    ) -> chat::TranscriptViewport {
        let mut transcript = chat::TranscriptViewport::new(
            if scrollable {
                "live-transcript"
            } else {
                "live-transcript-transition"
            },
            self.workspace.transcript.selection.clone(),
        );
        if scrollable {
            transcript = transcript.scroll(&self.workspace.transcript.scroll);
        }
        let snapshot = (!scrollable)
            .then_some(self.channel_transition.as_ref())
            .flatten()
            .filter(|transition| channel == Some(transition.outgoing.as_str()))
            .map(|transition| &transition.outgoing_data);
        let items = snapshot
            .or_else(|| channel.and_then(|channel| self.channel_data.get(channel)))
            .map(|data| data.items.as_slice())
            .unwrap_or_default();
        if items.is_empty() {
            transcript = transcript.empty(if channel.is_some() {
                "QUIET SO FAR"
            } else {
                "PICK A CHANNEL"
            });
        } else {
            let scope = channel.map_or(0, transcript_scope);
            transcript = transcript.children(
                items
                    .iter()
                    .enumerate()
                    .map(|(row, item)| self.stream_row(item, scope, row, scrollable)),
            );
        }
        transcript
    }

    pub(super) fn chat_panel(&self) -> workspace::ChannelChat {
        let mut panel =
            workspace::ChannelChat::new(self.transcript_view(self.selected.as_deref(), true));
        if let Some(transition) = &self.channel_transition {
            panel = panel.outgoing(
                self.transcript_view(
                    Some(&transition.outgoing),
                    self.transition_progress().is_none(),
                ),
                self.transition_progress(),
            );
        }
        panel
    }
}
