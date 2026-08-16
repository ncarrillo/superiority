use super::super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn sync_text_inputs(&mut self) {
        let join_query = self.join.join_input.content();
        if join_query != self.join.join_query {
            self.join.join_query = join_query;
            self.join.join_selected = 0;
            self.join.schedule_group_search();
        }

        let mut roster_filter = self.roster.roster_input.content();
        if roster_filter.chars().count() > 64 {
            roster_filter = roster_filter.chars().take(64).collect();
            self.roster.roster_input.set_content(roster_filter.clone());
        }
        if roster_filter != self.active_roster_filter() {
            self.set_roster_filter(roster_filter);
        }
    }

    pub(in crate::app::client) fn text_input_focused(&self, window: &Window) -> bool {
        self.composer.composer.is_focused(window)
            || self.join.join_input.is_focused(window)
            || self.roster.roster_input.is_focused(window)
            || self.social.conversation_input.is_focused(window)
    }

    pub(in crate::app::client) fn set_roster_pointer_focus(
        &mut self,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if hovered {
            if self.overlays.active.is_some()
                || self.connection.dialog_visible
                || self.warnings.warning_dialog.is_some()
                || self.updates.update_dialog_visible
                || self.channels.active().is_none()
            {
                return;
            }
            if self.roster.roster.focused && self.roster.roster_input.is_focused(window) {
                return;
            }
            self.roster.roster_input.focus(window, cx);
            self.composer.composer_focused = false;
            self.join.join_focused = false;
            self.roster.roster.focused = true;
            cx.notify();
        } else if self.roster.roster_input.is_focused(window) {
            self.roster.roster.focused = false;
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    pub(in crate::app::client) fn set_composer_pointer_focus(
        &mut self,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if hovered {
            if self.overlays.active.is_some()
                || self.connection.dialog_visible
                || self.warnings.warning_dialog.is_some()
                || self.updates.update_dialog_visible
                || self.channels.active().is_none()
            {
                return;
            }
            if self.composer.composer_focused && self.composer.composer.is_focused(window) {
                return;
            }
            self.composer.composer.focus(window, cx);
            self.composer.composer_focused = true;
            self.join.join_focused = false;
            self.roster.roster.focused = false;
            cx.notify();
        } else if self.composer.composer.is_focused(window) {
            self.composer.composer_focused = false;
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }
}
