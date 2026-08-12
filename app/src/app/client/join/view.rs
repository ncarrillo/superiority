use super::*;

const JOIN_HEADER: ui_modal::HeaderLayout =
    ui_modal::HeaderLayout::new((28.0, 6.0, 584.0, 56.0), 76.0, 488.0, 20.0);

impl JoinComponent {
    pub(in crate::app::client) fn modal(
        &self,
        tabs: &[ChannelState],
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        let border = if self.join_focused {
            rgb(0x33a8f0)
        } else {
            rgb(0x174f78)
        };
        let rows = self.rows(tabs);
        let selected = self.join_selected.min(rows.len().saturating_sub(1));
        let confirm_target = rows.get(selected).map(|row| row.target.clone());
        let mut list = div()
            .id("join-results-scroll")
            .absolute()
            .left(px(28.0))
            .top(px(140.0))
            .w(px(584.0))
            .h(px(356.0))
            .overflow_y_scroll()
            .track_scroll(&self.join_scroll)
            .font_family(FONT_INTERNATIONAL)
            .text_size(px(13.0));
        if rows.is_empty() {
            list = list.child(
                div()
                    .ml(px(14.0))
                    .mt(px(12.0))
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .text_size(px(14.0))
                    .text_color(rgb(0x7d8fa8))
                    .child("No channels match."),
            );
        } else {
            let mut previous = None;
            for (index, row) in rows.iter().enumerate() {
                if row.source != JoinSource::Typed && previous != Some(row.source) {
                    list = list.child(
                        div().h(px(46.0)).flex_shrink_0().flex().items_end().child(
                            div()
                                .h(px(35.0))
                                .w_full()
                                .flex()
                                .items_center()
                                .gap(px(12.0))
                                .px(px(12.0))
                                .font_family(FONT_INTERFACE)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(12.0))
                                .text_color(rgb(0x7d8fa8))
                                .child(row.source.heading())
                                .child(div().h(px(1.0)).flex_1().bg(rgb(0x174f78))),
                        ),
                    );
                }
                previous = Some(row.source);
                let selected = index == selected;
                let mut item = div()
                    .id(("join-option", index))
                    .h(px(44.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .pl(px(12.0))
                    .border_1()
                    .border_color(if selected {
                        rgb(0x3d9be6)
                    } else {
                        rgba(0x174f7800)
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(rgba(0x12315eb8)))
                    .active(|style| style.bg(rgba(0x1a4778e8)).opacity(0.82))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.join.join_selected = index;
                        cx.notify();
                    }))
                    .child(img(row.icon).size(px(30.0)).object_fit(ObjectFit::Contain))
                    .child(
                        div()
                            .ml(px(12.0))
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_color(rgb(0xd6e0f0))
                            .child(row.name.clone()),
                    );
                if let Some(note) = &row.note {
                    item = item.child(
                        div()
                            .w(px(160.0))
                            .mr(px(16.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_size(px(12.0))
                            .text_align(gpui::TextAlign::Right)
                            .text_color(rgb(0x7d8fa8))
                            .child(note.clone()),
                    );
                }
                if selected {
                    item = item.bg(rgba(0x12315ef5));
                }
                list = list.child(item);
            }
        }

        div()
            .id("join-modal")
            .relative()
            .w(px(640.0))
            .h(px(580.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(chrome.modal_chrome(640.0, 580.0))
            .child(chrome.modal_header(JOIN_HEADER, "JOIN A CHANNEL"))
            .child(
                div()
                    .id("join-field")
                    .absolute()
                    .left(px(28.0))
                    .top(px(74.0))
                    .w(px(584.0))
                    .h(px(48.0))
                    .flex()
                    .items_center()
                    .px(px(16.0))
                    .bg(rgba(0x030b13f7))
                    .border_1()
                    .border_color(border)
                    .rounded(px(2.0))
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(13.5))
                    .text_color(rgb(0xd6e0f0))
                    .cursor(gpui::CursorStyle::IBeam)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.join.join_focused = true;
                        this.join.join_input.focus(window, cx);
                        cx.notify();
                    }))
                    .child(self.join_input.element()),
            )
            .child(list)
            .child(
                chrome
                    .action_button("join-cancel", "CANCEL", 132.0, 44.0, false)
                    .absolute()
                    .left(px(340.0))
                    .top(px(514.0))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_overlay(window, cx);
                    })),
            )
            .child(
                chrome
                    .action_button("join-confirm", "JOIN", 132.0, 44.0, true)
                    .absolute()
                    .left(px(480.0))
                    .top(px(514.0))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(target) = &confirm_target {
                            this.join_channel_target(target.clone(), cx);
                        }
                    })),
            )
            .child(
                ui_controls::close_button("join-close")
                    .absolute()
                    .left(px(602.0))
                    .top(px(18.0))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_overlay(window, cx);
                    })),
            )
    }

    pub(in crate::app::client) fn overlay(
        &self,
        tabs: &[ChannelState],
        chrome: &ChromeComponent,
        overlays: &OverlayComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let modal = self.modal(tabs, chrome, cx);
        let overlay = div()
            .id("join-dismiss")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, window, cx| {
                this.dismiss_overlay(window, cx);
            }))
            .child(overlays.dimmer())
            .child(ui_modal::animated(
                modal,
                overlays.closing,
                false,
                640.0,
                580.0,
                "join-panel-open",
                "join-panel-close",
                "join-scan-open",
                "join-scan-close",
            ));
        overlays.animated(overlay, "join-overlay-open", "join-overlay-close", false)
    }
}
