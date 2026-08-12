use super::*;

impl LiveView {
    pub(super) fn select_channel(&mut self, key: String, cx: &mut Context<Self>) {
        if self.selected.as_deref() == Some(key.as_str()) {
            return;
        }
        let outgoing = self.selected.replace(key.clone());
        self.workspace.transcript.selection.clear();
        self.workspace.roster.focused = false;
        self.workspace.roster.filter.clear();
        self.workspace.roster.animation = None;
        let now = js_sys::Date::now();
        let fresh = self.channel_data.get(&key).is_some_and(|data| {
            data.stream_loaded
                && data
                    .refreshed_at
                    .is_some_and(|refreshed_at| now - refreshed_at <= CHANNEL_FRESH_MS)
        });
        self.channel_transition = outgoing.map(|outgoing| {
            let outgoing_data = self
                .channel_data
                .get(&outgoing)
                .cloned()
                .unwrap_or_default();
            let outgoing_roster_scroll = UniformListScrollHandle::new();
            *outgoing_roster_scroll.0.borrow_mut() =
                self.workspace.roster.scroll.0.borrow().clone();
            ChannelTransition::pending(ChannelTransitionSnapshot {
                outgoing,
                outgoing_data,
                outgoing_roster_scroll,
                incoming: key.clone(),
            })
        });
        if fresh && let Some(transition) = &mut self.channel_transition {
            transition.start(now);
        }
        self.workspace.transcript.scroll.scroll_to_bottom();
        cx.notify();
    }

    pub(super) fn start_pending_transition(&mut self, channel: &str) {
        if let Some(transition) = &mut self.channel_transition
            && transition.incoming == channel
            && transition.is_pending()
        {
            transition.start(js_sys::Date::now());
        }
    }

    pub(super) fn transition_progress(&self) -> Option<f32> {
        self.channel_transition
            .as_ref()?
            .progress(js_sys::Date::now())
    }

    pub(super) fn update_transition(&mut self, window: &mut Window) {
        let now = js_sys::Date::now();
        if self
            .channel_transition
            .as_ref()
            .is_some_and(|transition| transition.is_complete(now))
        {
            self.channel_transition = None;
        }
        self.workspace.roster.finish_transition(now);
        self.workspace.navigation.tabs.retain_name_animations(now);
        let running = self
            .channel_transition
            .as_ref()
            .is_some_and(|transition| transition.is_running(now))
            || self.workspace.navigation.tabs.shift_is_running(now)
            || self
                .workspace
                .navigation
                .tabs
                .name_animation_is_running(now)
            || self.workspace.roster.animation.is_some();
        if running {
            window.request_animation_frame();
        }
    }

    pub(super) fn set_tab_name_hover(
        &mut self,
        key: String,
        hovered: bool,
        travel: f32,
        cx: &mut Context<Self>,
    ) {
        if self
            .workspace
            .navigation
            .tabs
            .set_name_hover(key, hovered, travel, js_sys::Date::now())
        {
            cx.notify();
        }
    }

    pub(super) fn asset(&self, path: &str) -> String {
        format!("{}/{}", self.asset_root, path.trim_start_matches('/'))
    }

    pub(super) fn begin_tab_pointer(&mut self, index: usize, event: &MouseDownEvent) {
        self.workspace.navigation.tabs.begin_pointer(
            index,
            f32::from(event.position.x),
            self.channels.len(),
        );
    }

    pub(super) fn update_tab_drag(
        &mut self,
        event: &DragMoveEvent<TabDragPayload>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let payload = *event.drag(cx);
        let widths = self
            .channels
            .iter()
            .map(|channel| navigation::tab_width(&channel.label(), false))
            .collect::<Vec<_>>();
        if self.workspace.navigation.tabs.update_drag(
            payload.index,
            f32::from(event.event.position.x),
            &widths,
            js_sys::Date::now(),
        ) {
            cx.notify();
        }
    }

    pub(super) fn click_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        self.workspace.navigation.tabs.cancel_pointer();
        if let Some(channel) = self.channels.get(index) {
            self.select_channel(channel.key.clone(), cx);
        }
    }

    pub(super) fn finish_tab_drag(
        &mut self,
        payload: &TabDragPayload,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let navigation::TabRelease::Reorder { from, to } =
            self.workspace.navigation.tabs.finish(payload.index)
        else {
            self.click_tab(payload.index, cx);
            return;
        };
        if from < self.channels.len() && to < self.channels.len() && from != to {
            let channel = self.channels.remove(from);
            self.channels.insert(to, channel);
        }
        cx.notify();
    }
}
