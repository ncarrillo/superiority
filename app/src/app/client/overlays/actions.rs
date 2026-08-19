use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn dismiss_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(overlay) = self.overlays.active else {
            return;
        };
        if self.overlays.closing {
            return;
        }
        self.overlays.closing = true;
        self.settings.settings_tooltip = None;
        self.social.conversation_focused = false;
        self.focus_handle.focus(window, cx);
        self.overlays.epoch = self.overlays.epoch.wrapping_add(1);
        let epoch = self.overlays.epoch;
        let duration = if overlay == Overlay::Account {
            130
        } else {
            260
        };
        cx.notify();
        let executor = cx.background_executor().clone();
        cx.spawn_in(window, async move |entity, cx| {
            executor.timer(Duration::from_millis(duration)).await;
            entity
                .update_in(cx, |this, _, cx| {
                    if this.overlays.epoch == epoch {
                        this.overlays.active = None;
                        this.overlays.closing = false;
                        this.social.social_detail_open = false;
                        this.social.social_pane_transition = None;
                        this.social.conversation_peer = None;
                        this.social.conversation_input.clear();
                        cx.notify();
                    }
                })
                .ok();
        })
        .detach();
    }
}
