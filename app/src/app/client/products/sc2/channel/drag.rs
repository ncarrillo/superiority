use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn begin_tab_pointer(
        &mut self,
        index: usize,
        event: &MouseDownEvent,
        _: &mut Context<Self>,
    ) {
        if self.overlays.active.is_some()
            || self.updates.update_dialog_visible
            || self.session.channels.tab_close.is_some()
            || index >= self.session.channels.tabs.len()
        {
            return;
        }
        let tab_count = self.session.channels.tabs.len();
        self.session.channels.navigation.tabs.begin_pointer(
            index,
            f32::from(event.position.x),
            tab_count,
        );
    }

    pub(in crate::app::client) fn update_tab_drag(
        &mut self,
        event: &DragMoveEvent<TabDragPayload>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let payload = *event.drag(cx);
        let widths = self
            .session
            .channels
            .tabs
            .iter()
            .map(|tab| ui_navigation::tab_width(&tab.title, tab.unread))
            .collect::<Vec<_>>();
        if self.session.channels.navigation.tabs.update_drag(
            payload.index,
            f32::from(event.event.position.x),
            &widths,
            Instant::now(),
        ) {
            cx.notify();
        }
    }

    pub(in crate::app::client) fn click_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        self.session.channels.navigation.tabs.cancel_pointer();
        self.select_tab(index, cx);
    }

    pub(in crate::app::client) fn finish_tab_drag(
        &mut self,
        payload: &TabDragPayload,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ui_navigation::TabRelease::Reorder { from, to } =
            self.session.channels.navigation.tabs.finish(payload.index)
        else {
            self.select_tab(payload.index, cx);
            return;
        };
        if from >= self.session.channels.tabs.len() || to >= self.session.channels.tabs.len() {
            return;
        }
        if from != to {
            let carried = self.session.channels.tabs.remove(from);
            self.session.channels.tabs.insert(to, carried);
            self.session.channels.active_tab = if self.session.channels.active_tab == from {
                to
            } else if from < self.session.channels.active_tab
                && self.session.channels.active_tab <= to
            {
                self.session.channels.active_tab - 1
            } else if to <= self.session.channels.active_tab
                && self.session.channels.active_tab < from
            {
                self.session.channels.active_tab + 1
            } else {
                self.session.channels.active_tab
            };
            self.persist_open_channels();
            Self::trace(format_args!("reordered tab {from} to {to}"));
        }
        cx.notify();
    }
}
