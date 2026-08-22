use super::*;

impl SocialComponent {
    pub(in crate::app::client) fn overlay(
        &self,
        variant: ui_shared_modal::ModalVariant,
        chrome: &ChromeComponent,
        overlays: &OverlayComponent,
        window: &mut Window,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let pane_offset = self.pane_offset(Instant::now());
        let skin = SocialSkin::for_variant(variant);
        // the panes slide as one; the header crossfades with them so the
        // conversation's identity takes the SOCIAL title's place instead of
        // stacking a second header under it.
        let detail_progress = (-pane_offset / 400.0).clamp(0.0, 1.0);
        let social_rows = self.rows(variant, chrome, cx);
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
            .vertical_scrollbar_in(&self.social_scroll, variant.scrollbar(), window, cx);
        let list_pane = div()
            .absolute()
            .left_0()
            .top_0()
            .w(px(400.0))
            .h(px(SOCIAL_BODY_HEIGHT))
            .font_family(skin.interface_font)
            .child(social_viewport);
        let body_track = div()
            .absolute()
            .left(px(pane_offset - SOCIAL_FRAME_CLIP_GUTTER))
            .top_0()
            .w(px(800.0))
            .h(px(SOCIAL_BODY_HEIGHT))
            .child(list_pane)
            .child(self.conversation_pane(variant, window, cx));
        let body_clip = div()
            .absolute()
            .left(px(SOCIAL_FRAME_CLIP_GUTTER))
            .right(px(SOCIAL_FRAME_CLIP_GUTTER))
            .top(px(SOCIAL_BODY_TOP))
            .bottom(px(SOCIAL_FRAME_CLIP_GUTTER))
            .overflow_hidden()
            .child(body_track);
        // The interaction is shared, but its shell belongs to the realm the
        // user is standing in. Social opened from Remastered or Reforged must
        // not bring StarCraft II's chrome along with it.
        let modal = div()
            .id("friends-modal")
            .relative()
            .w(px(400.0))
            .h(px(440.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(ui_shared_modal::frame(
                variant,
                400.0,
                440.0,
                &chrome.modal_textures,
            ))
            .child(body_clip);
        let mut modal = modal.child(
            div()
                .absolute()
                .left(px(18.0))
                .right(px(18.0))
                .top(px(32.0))
                .child(ui_shared_modal::title(variant, "SOCIAL"))
                .opacity(1.0 - detail_progress),
        );
        if detail_progress > 0.0 {
            modal = modal.child(
                self.conversation_header(variant, chrome, cx)
                    .opacity(detail_progress),
            );
        }
        let modal = modal.child(
            ui_shared_modal::close_glyph(variant)
                .right(px(26.0))
                .top(px(32.0))
                .id("friends-close")
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
            // Remastered and Reforged render Social beside their realm window,
            // not inside it. Their window-level key listener therefore cannot
            // see Enter from this input. Own the keyboard route here, where it
            // is shared by every product and is an ancestor of the composer.
            .on_key_down(cx.listener(SuperiorityView::on_key_down))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, window, cx| {
                this.dismiss_overlay(window, cx);
            }))
            .child(overlays.dimmer())
            .child(
                // the shared driver: for this realm it is the legacy scan
                // reveal, plus the reduced-motion fade the legacy call lacked
                ui_shared_modal::animated(
                    variant,
                    modal,
                    overlays.closing,
                    platform::reduce_motion(),
                    400.0,
                    440.0,
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
