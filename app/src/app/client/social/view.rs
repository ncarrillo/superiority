use super::*;

const SOCIAL_HEADER: ui_modal::HeaderLayout =
    ui_modal::HeaderLayout::new((28.0, 6.0, 344.0, 56.0), 76.0, 248.0, 18.0);

impl SocialComponent {
    pub(in crate::app::client) fn overlay(
        &self,
        chrome: &ChromeComponent,
        overlays: &OverlayComponent,
        window: &mut Window,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let pane_offset = self.pane_offset(Instant::now());
        let social_rows = self.rows(chrome, cx);
        let social_results = div()
            .id("social-results-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.social_scroll)
            .children(social_rows);
        let social_viewport = div()
            .id("social-results-viewport")
            .absolute()
            .left(px(SOCIAL_CONTENT_GUTTER))
            .right(px(SOCIAL_CONTENT_GUTTER))
            .top(px(10.0))
            .bottom(px(20.0))
            .child(social_results)
            .vertical_scrollbar_for(&self.social_scroll, window, cx);
        let list_pane = div()
            .absolute()
            .left_0()
            .top_0()
            .w(px(400.0))
            .h(px(SOCIAL_BODY_HEIGHT))
            .font_family(FONT_INTERFACE)
            .child(social_viewport);
        let body_track = div()
            .absolute()
            .left(px(pane_offset - SOCIAL_FRAME_CLIP_GUTTER))
            .top_0()
            .w(px(800.0))
            .h(px(SOCIAL_BODY_HEIGHT))
            .child(list_pane)
            .child(self.conversation_pane(chrome, window, cx));
        let body_clip = div()
            .absolute()
            .left(px(SOCIAL_FRAME_CLIP_GUTTER))
            .right(px(SOCIAL_FRAME_CLIP_GUTTER))
            .top(px(SOCIAL_BODY_TOP))
            .bottom(px(SOCIAL_FRAME_CLIP_GUTTER))
            .overflow_hidden()
            .child(body_track);
        let mut modal = div()
            .id("friends-modal")
            .relative()
            .w(px(400.0))
            .h(px(440.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(chrome.modal_chrome(400.0, 440.0))
            .child(body_clip);
        if let Some(frame) = &chrome.modal_frame {
            modal = modal.child(
                img(frame.image(400.0, 440.0))
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(400.0))
                    .h(px(440.0))
                    .object_fit(ObjectFit::Fill),
            );
        }
        let modal = modal
            .child(chrome.modal_header(SOCIAL_HEADER, "SOCIAL"))
            .child(
                ui_controls::close_button("friends-close")
                    .absolute()
                    .left(px(362.0))
                    .top(px(18.0))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_overlay(window, cx);
                    })),
            );
        let overlay = div()
            .id("friends-dismiss")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, window, cx| {
                this.dismiss_overlay(window, cx);
            }))
            .child(overlays.dimmer())
            .child(
                ui_modal::animated(
                    modal,
                    overlays.closing,
                    false,
                    400.0,
                    440.0,
                    "friends-panel-open",
                    "friends-panel-close",
                    "friends-scan-open",
                    "friends-scan-close",
                )
                .absolute()
                .right(px(22.0))
                .bottom(px(88.0)),
            )
            .child(
                div()
                    .id("friends-toggle-close")
                    .absolute()
                    .right(px(22.0))
                    .bottom(px(22.0))
                    .w(px(56.0))
                    .h(px(COMPOSER_HEIGHT))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.dismiss_overlay(window, cx);
                    })),
            );
        overlays.animated(
            overlay,
            "friends-overlay-open",
            "friends-overlay-close",
            false,
        )
    }
}
