//! the Reforged strip's behaviour: everything StarCraft II's tabs do, on
//! Reforged's channels — a press that may become a drag, drag-to-reorder, the
//! long-name marquee under the pointer, the selection effect, and the fold
//! that closes a tab before the hall is left.
//!
//! The data is the other half (`model.rs`): one `Wc3ChannelState` per hall,
//! keyed by the worker's channel index. This file only moves the strip.

use super::*;

/// how long the selection effect plays on a newly chosen tab.
const SELECTION_EFFECT: Duration = Duration::from_millis(235);

impl SuperiorityView {
    /// the staged brightening of a just-chosen tab's chrome, effect by effect
    /// — the same curve StarCraft II's tabs play.
    pub(in crate::app::client) fn wc3_tab_effect_opacity(
        &self,
        effect: usize,
        now: Instant,
    ) -> f32 {
        let Some(started) = self.session.wc3().and_then(|wc3| wc3.tab_selection_started) else {
            return 1.0;
        };
        let duration = 0.16 + effect as f32 * 0.025;
        let progress = ease_in_out(
            (now.saturating_duration_since(started).as_secs_f32() / duration).clamp(0.0, 1.0),
        );
        let from = if effect == 0 { 0.48 } else { 0.0 };
        from + (1.0 - from) * progress
    }

    /// a press on a tab: the start of a click or of a drag, decided by how
    /// far the pointer goes before it lets go.
    pub(in crate::app::client) fn begin_wc3_tab_pointer(
        &mut self,
        index: usize,
        event: &MouseDownEvent,
        _: &mut Context<Self>,
    ) {
        if self.overlays.active.is_some() || self.updates.update_dialog_visible {
            return;
        }
        let Some(wc3) = self.session.wc3_mut() else {
            return;
        };
        if wc3.tab_close.is_some() || index >= wc3.channels.len() {
            return;
        }
        let tab_count = wc3.channels.len();
        wc3.navigation
            .tabs
            .begin_pointer(index, f32::from(event.position.x), tab_count);
    }

    pub(in crate::app::client) fn update_wc3_tab_drag(
        &mut self,
        event: &DragMoveEvent<TabDragPayload>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let payload = *event.drag(cx);
        let Some(wc3) = self.session.wc3_mut() else {
            return;
        };
        let widths = wc3
            .channels
            .iter()
            .map(|channel| ui_navigation::tab_width(&channel.name, channel.unread))
            .collect::<Vec<_>>();
        if wc3.navigation.tabs.update_drag(
            payload.index,
            f32::from(event.event.position.x),
            &widths,
            Instant::now(),
        ) {
            cx.notify();
        }
    }

    /// a click that did not become a drag chooses the tab.
    pub(in crate::app::client) fn click_wc3_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(wc3) = self.session.wc3_mut() {
            wc3.navigation.tabs.cancel_pointer();
        }
        self.choose_wc3_tab(index, cx);
    }

    /// chooses a tab and starts its selection effect.
    fn choose_wc3_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let before = self.session.wc3().map(|wc3| wc3.active_channel);
        self.select_wc3_channel(index, cx);
        let after = self.session.wc3().map(|wc3| wc3.active_channel);
        if before != after
            && let Some(wc3) = self.session.wc3_mut()
        {
            wc3.tab_selection_started = Some(Instant::now());
            cx.notify();
        }
    }

    /// the pointer let go: either a reorder lands, or it was a click after
    /// all.
    pub(in crate::app::client) fn finish_wc3_tab_drag(
        &mut self,
        payload: &TabDragPayload,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let release = self
            .session
            .wc3_mut()
            .map(|wc3| wc3.navigation.tabs.finish(payload.index));
        let Some(ui_navigation::TabRelease::Reorder { from, to }) = release else {
            self.choose_wc3_tab(payload.index, cx);
            return;
        };
        let Some(wc3) = self.session.wc3_mut() else {
            return;
        };
        if from >= wc3.channels.len() || to >= wc3.channels.len() {
            return;
        }
        if from != to {
            let carried = wc3.channels.remove(from);
            wc3.channels.insert(to, carried);
            wc3.active_channel = reordered_active(wc3.active_channel, from, to);
            Self::trace(format_args!("reordered Reforged tab {from} to {to}"));
        }
        cx.notify();
    }

    /// the × on a tab: the strip folds it over `TAB_CLOSE_DURATION`, and only
    /// then is the hall left.
    pub(in crate::app::client) fn begin_wc3_tab_close(
        &mut self,
        index: usize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(wc3) = self.session.wc3_mut() else {
            return;
        };
        if index >= wc3.channels.len() || wc3.tab_close.is_some() {
            return;
        }
        wc3.navigation.tabs.cancel_pointer();
        wc3.tab_close = Some(Wc3TabClose {
            index,
            started: None,
        });
        cx.notify();
    }

    pub(in crate::app::client) fn set_wc3_tab_name_hover(
        &mut self,
        id: u64,
        hovered: bool,
        travel: f32,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(wc3) = self.session.wc3_mut() else {
            return;
        };
        if hovered {
            wc3.hovered_tab = Some(id);
        } else if wc3.hovered_tab == Some(id) {
            wc3.hovered_tab = None;
        }
        wc3.navigation
            .tabs
            .set_name_hover(id, hovered, travel, Instant::now());
        cx.notify();
    }

    /// moves the strip's clocks on each frame: a finished fold leaves its
    /// hall, marquee and reorder animations are retained while they run, the
    /// selection effect ends. Returns whether anything is still moving.
    pub(in crate::app::client) fn advance_wc3_tab_animations(
        &mut self,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(wc3) = self.session.wc3_mut() else {
            return false;
        };
        let finished_close = wc3.tab_close.as_mut().and_then(|closing| {
            let started = *closing.started.get_or_insert(now);
            (now.saturating_duration_since(started) >= TAB_CLOSE_DURATION).then_some(closing.index)
        });
        if let Some(index) = finished_close {
            wc3.tab_close = None;
            let closed = wc3.channels.get(index).map(|channel| u64::from(channel.id));
            if let Some(id) = closed {
                wc3.navigation.tabs.remove_name(&id);
            }
            self.close_wc3_channel(index, cx);
        }
        let Some(wc3) = self.session.wc3_mut() else {
            return false;
        };
        wc3.navigation.tabs.retain_name_animations(now);
        wc3.roster.finish_transition(now);
        if wc3
            .tab_selection_started
            .is_some_and(|started| now.saturating_duration_since(started) >= SELECTION_EFFECT)
        {
            wc3.tab_selection_started = None;
        }
        wc3.tab_close.is_some()
            || wc3.roster.animation.is_some()
            || wc3.navigation.tabs.shift_is_running(now)
            || wc3.navigation.tabs.name_animation_is_running(now)
            || wc3.tab_selection_started.is_some()
    }
}

/// where the active tab ends up once the tab at `from` has moved to `to`.
pub(in crate::app::client) const fn reordered_active(
    active: usize,
    from: usize,
    to: usize,
) -> usize {
    if active == from {
        to
    } else if from < active && active <= to {
        active - 1
    } else if to <= active && active < from {
        active + 1
    } else {
        active
    }
}

#[cfg(test)]
mod tests {
    use super::reordered_active;

    #[test]
    fn the_active_tab_follows_itself_and_slides_past_a_reorder() {
        assert_eq!(reordered_active(0, 0, 2), 2);
        // a tab moved from before the active one to after it slides it left
        assert_eq!(reordered_active(1, 0, 2), 0);
        // a tab moved from after the active one to before it slides it right
        assert_eq!(reordered_active(1, 2, 0), 2);
        // a reorder entirely elsewhere leaves it be
        assert_eq!(reordered_active(0, 1, 2), 0);
    }
}
