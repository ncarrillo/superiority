use super::*;

impl ChannelComponent {
    pub(super) fn tab_effect_opacity(&self, effect: usize, now: Instant) -> f32 {
        let Some(started) = self.tab_selection_started else {
            return 1.0;
        };
        let duration = 0.16 + effect as f32 * 0.025;
        let progress = ease_in_out(
            (now.saturating_duration_since(started).as_secs_f32() / duration).clamp(0.0, 1.0),
        );
        let from = if effect == 0 { 0.48 } else { 0.0 };
        from + (1.0 - from) * progress
    }

    pub(super) fn view(
        &self,
        account_portrait: Option<Arc<RenderImage>>,
        chrome: &ChromeComponent,
        _window: &Window,
        cx: &mut Context<SuperiorityView>,
    ) -> Div {
        let now = Instant::now();
        let drag_offsets = self.navigation.tabs.offsets(now, self.tabs.len());
        let (closing_index, closing_progress) =
            self.tab_close.as_ref().map_or((None, 0.0), |closing| {
                let elapsed = closing.started.map_or(0.0, |started| {
                    now.saturating_duration_since(started).as_secs_f32()
                });
                (
                    Some(closing.index),
                    ease_in_out((elapsed / TAB_CLOSE_DURATION.as_secs_f32()).clamp(0.0, 1.0)),
                )
            });
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let id = tab.id;
                ui_navigation::ChannelTab::new(format!("channel-tab-{id}"), tab.title.clone())
                    .unread(tab.unread)
                    .active(index == self.active_tab)
                    .hovered(self.hovered_tab == Some(id))
                    .marquee_offset(self.navigation.tabs.name_offset(&id, now))
                    .drag_offset(drag_offsets[index])
                    .dragged_travel(
                        self.navigation
                            .tabs
                            .is_dragging(index)
                            .then(|| self.navigation.tabs.dragged_travel()),
                    )
                    .close_progress((closing_index == Some(index)).then_some(closing_progress))
                    .effect_opacity([
                        self.tab_effect_opacity(1, now),
                        self.tab_effect_opacity(2, now),
                        self.tab_effect_opacity(3, now),
                    ])
                    .on_mouse_down(cx.listener(move |this, event, _, cx| {
                        this.begin_tab_pointer(index, event, cx);
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.click_tab(index, cx);
                    }))
                    .on_hover(cx.listener(
                        move |this, event: &ui_navigation::TabHoverEvent, window, cx| {
                            this.set_tab_name_hover(id, event.hovered, event.travel, window, cx);
                        },
                    ))
                    .on_close(cx.listener(move |this, _, window, cx| {
                        this.begin_tab_close(index, window, cx);
                    }))
            })
            .collect();
        let tabs = ui_navigation::ChannelTabs::new(tabs, chrome.ui_assets.clone())
            .leading(TAB_LEADING_SPACE)
            .on_add(cx.listener(|this, _, window, cx| {
                this.composer.composer_focused = false;
                this.roster.roster.focused = false;
                this.join.join_focused = true;
                this.join.join_input.clear();
                this.join.join_input.focus(window, cx);
                this.join.join_query.clear();
                this.join.join_selected = 0;
                this.join.group_search_due = None;
                this.join.group_search.clear();
                this.overlays.active = Some(Overlay::Join);
                this.overlays.closing = false;
                cx.notify();
            }));
        let control_end = TAB_LEADING_SPACE + tabs.tail() + 47.0;

        let background = chrome
            .top_nav_background
            .as_ref()
            .map(|background| gpui::ImageSource::from(background.clone()));

        ui_navigation::bar(background)
            .child(tabs)
            .child(
                div()
                    .id("window-drag-region")
                    .absolute()
                    .top_0()
                    .left(px(control_end))
                    .right(px(TAB_BAR_HEIGHT))
                    .h(px(TAB_BAR_HEIGHT))
                    .on_mouse_down(MouseButton::Left, |_, window, cx| {
                        cx.stop_propagation();
                        platform::begin_window_drag(window);
                    }),
            )
            .child(
                div()
                    .id("account")
                    .absolute()
                    .right_0()
                    .top_0()
                    .size(px(TAB_BAR_HEIGHT * 69.0 / 72.0))
                    .bg(rgb(0x0b121a))
                    .border_1()
                    .border_color(rgb(0x1d649c))
                    .rounded_tr(px(11.0))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0x16273e)).shadow_lg())
                    .active(|style| style.bg(rgb(0x1d3a5c)).opacity(0.84))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.composer.composer_focused = false;
                        this.roster.roster.focused = false;
                        if this.overlays.active == Some(Overlay::Account) {
                            this.dismiss_overlay(window, cx);
                        } else {
                            this.overlays.active = Some(Overlay::Account);
                            this.overlays.closing = false;
                            cx.notify();
                        }
                    }))
                    .child(
                        account_portrait
                            .map_or_else(|| img("images/icons/account-placeholder.png"), img)
                            .absolute()
                            .top(px(3.0))
                            .left(px(3.0))
                            .size(px(35.0))
                            .object_fit(ObjectFit::Contain),
                    )
                    .child(
                        img("images/nine-patch/portraits/frame.png")
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .object_fit(ObjectFit::Fill),
                    ),
            )
    }
}
