use super::*;

const NAV_TOP: f32 = 68.0;
const NAV_ITEM_HEIGHT: f32 = 44.0;
const NAV_ITEM_GAP: f32 = 6.0;

impl SettingsComponent {
    pub(in crate::app::client) fn modal(
        &self,
        product: Product,
        variant: ui_shared_modal::ModalVariant,
        chrome: &ChromeComponent,
        live_url: Option<String>,
        live_error: Option<String>,
        window: &mut Window,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        // everything inside the plate reads in the realm's own language
        let skin = SettingsSkin::for_variant(variant);
        let mut modal = div()
            .id("settings-modal")
            .relative()
            .w(px(944.0))
            .h(px(620.0))
            .font_family(skin.interface_font)
            .text_color(rgb(skin.text))
            .on_click(|_, _, cx| cx.stop_propagation())
            // the shared modal shell, dressed as whichever realm it is opened
            // over: settings on the Terran console is a console dialog
            .child(ui_shared_modal::frame(
                variant,
                944.0,
                620.0,
                &chrome.modal_textures,
            ))
            .child(
                div()
                    .absolute()
                    .left(px(235.0))
                    .top(px(532.0))
                    .w(px(692.0))
                    .h(px(71.0))
                    .bg(rgba(skin.footer_fill)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(234.0))
                    .top(px(94.0))
                    .w(px(694.0))
                    .h(px(510.0))
                    .border_1()
                    .border_color(rgba(skin.structural_edge))
                    .rounded(px(2.0)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .right(px(18.0))
                    .top(px(32.0))
                    .child(ui_shared_modal::title(variant, "SETTINGS")),
            );

        for (index, title) in SettingsPage::SHOWN
            .iter()
            .map(|page| page.title())
            .enumerate()
        {
            let active = self.active_settings_page == index;
            let mut item = div()
                .id(("settings-navigation", index))
                .absolute()
                .left(px(28.0))
                .top(px(NAV_TOP + index as f32 * (NAV_ITEM_HEIGHT + NAV_ITEM_GAP)))
                .w(px(190.0))
                .h(px(NAV_ITEM_HEIGHT))
                .flex()
                .items_center()
                .justify_start()
                .pl(px(18.0))
                .cursor_pointer()
                .border_1()
                .font_family(skin.nav_font)
                .text_size(px(12.0))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.settings.active_settings_page == index {
                        return;
                    }
                    this.settings.settings_page_transition = Some(SettingsPageTransition {
                        outgoing: this.settings.active_settings_page,
                        started: Instant::now(),
                    });
                    this.settings.active_settings_page = index;
                    this.settings.settings_tooltip = None;
                    cx.notify();
                }))
                .child(div().relative().child(title));
            // only the selected page earns the fill, the bright stroke, and the
            // glow; the rest of the rail stays flat so it stops competing with
            // the page content and the footer button.
            item = if active {
                item.bg(rgba(skin.rail_active_fill))
                    .border_color(rgb(skin.focused))
                    .shadow(skin.focus_glow())
                    .text_color(rgb(skin.rail_active_text))
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top(px(6.0))
                            .bottom(px(6.0))
                            .w(px(2.0))
                            .bg(rgb(skin.rail_bar)),
                    )
            } else {
                let hover_fill = skin.rail_hover_fill;
                item.border_color(rgba(0x0000_0000))
                    .text_color(rgb(skin.rail_text))
                    .hover(move |style| style.bg(rgba(hover_fill)))
            };
            modal = modal.child(item);
        }

        let now = Instant::now();
        if let Some(transition) = &self.settings_page_transition {
            let progress = ease_in_out(
                (now.saturating_duration_since(transition.started)
                    .as_secs_f32()
                    / SETTINGS_PAGE_CROSSFADE_DURATION.as_secs_f32())
                .clamp(0.0, 1.0),
            );
            modal = modal
                .child(self.settings_page(
                    transition.outgoing,
                    1.0 - progress,
                    product,
                    variant,
                    live_url.clone(),
                    live_error.clone(),
                    window,
                    cx,
                ))
                .child(self.settings_page(
                    self.active_settings_page,
                    progress,
                    product,
                    variant,
                    live_url.clone(),
                    live_error.clone(),
                    window,
                    cx,
                ));
        } else {
            modal = modal.child(self.settings_page(
                self.active_settings_page,
                1.0,
                product,
                variant,
                live_url,
                live_error,
                window,
                cx,
            ));
        }

        modal = modal
            .child(
                ui_buttons::button(
                    "settings-close-action",
                    variant,
                    ui_buttons::ButtonWeight::Primary,
                    ui_buttons::ButtonTone::Chrome,
                    ui_buttons::ButtonLife::Ready,
                    ui_buttons::worded(variant, "CLOSE"),
                )
                .w(px(142.0))
                .h(px(42.0))
                .absolute()
                .left(px(774.0))
                .top(px(549.0))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.dismiss_overlay(window, cx);
                })),
            )
            .child(
                ui_shared_modal::close_glyph(variant)
                    .right(px(26.0))
                    .top(px(32.0))
                    .id("settings-close")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_overlay(window, cx);
                    })),
            );
        if let Some(tooltip) = self.tooltip_card(&skin, &chrome.ui_assets) {
            modal = modal.child(tooltip);
        }
        modal
    }
}

impl SettingsComponent {
    pub(in crate::app::client) fn overlay(
        &self,
        product: Product,
        variant: ui_shared_modal::ModalVariant,
        chrome: &ChromeComponent,
        overlays: &OverlayComponent,
        live_url: Option<String>,
        live_error: Option<String>,
        window: &mut Window,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let modal = self.modal(product, variant, chrome, live_url, live_error, window, cx);
        let overlay = div()
            .id("settings-dismiss")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(overlays.dimmer())
            .child(ui_shared_modal::animated(
                variant,
                modal,
                overlays.closing,
                platform::reduce_motion(),
                944.0,
                620.0,
            ));
        overlays.animated(
            overlay,
            "settings-overlay-open",
            "settings-overlay-close",
            false,
        )
    }
}
