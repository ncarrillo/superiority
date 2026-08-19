use super::super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn sync_text_inputs(&mut self, cx: &mut Context<Self>) {
        // a mention is the token under the caret, so moving the caret changes
        // what is being asked for even when the line has not changed
        let line = self.composer.composer.content();
        let cursor = self.composer.composer.cursor();
        if line != self.composer.command_line || cursor != self.composer.command_cursor {
            let typed = line != self.composer.command_line;
            let previous = std::mem::replace(&mut self.composer.command_line, line);
            let previous_cursor = std::mem::replace(&mut self.composer.command_cursor, cursor);
            // the popup was open on the previous line unless escape had already
            // shut it
            let was_open = !self.composer.command_dismissed
                && parse_command(&previous, previous_cursor).is_some();
            self.composer.command_selected = 0;
            if typed {
                // dismissal belongs to the line it was made on, so typing
                // brings the popup back rather than needing it retyped
                self.composer.command_dismissed = false;
            }
            if let Some(command) = parse_command(&self.composer.command_line, cursor) {
                if command.kind == CommandKind::Join {
                    self.join.schedule_group_search(&command.query);
                }
                if !was_open {
                    self.composer.command_epoch = self.composer.command_epoch.wrapping_add(1);
                }
                self.cancel_command_close();
            } else if was_open
                && let Some(results) = self.results_for_line(&previous, previous_cursor)
            {
                self.begin_command_close(results, cx);
            }
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

    /// follows the field's focus so the popup answers to it. losing focus with a
    /// command typed dismisses it the way escape does — the list fades out
    /// instead of blinking away — and clicking back into the field brings it
    /// back rather than leaving it shut until the next keystroke.
    pub(in crate::app::client) fn sync_command_focus(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let focused = self.composer.composer_focused && self.composer.composer.is_focused(window);
        if focused == self.composer.command_focused {
            return;
        }
        self.composer.command_focused = focused;
        if focused {
            if parse_command(&self.composer.command_line, self.composer.command_cursor).is_some() {
                self.composer.command_dismissed = false;
                self.composer.command_selected = 0;
                self.composer.command_epoch = self.composer.command_epoch.wrapping_add(1);
                self.cancel_command_close();
            }
            return;
        }
        if let Some(results) = self.command_results() {
            self.composer.command_dismissed = true;
            self.begin_command_close(results, cx);
        }
    }

    pub(in crate::app::client) fn text_input_focused(&self, window: &Window) -> bool {
        self.composer.composer.is_focused(window)
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
            // a command owns the keyboard until it is answered or dismissed
            if self.composer.active_command().is_some() {
                return;
            }
            self.roster.roster_input.focus(window, cx);
            self.composer.composer_focused = false;
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
            self.roster.roster.focused = false;
            cx.notify();
        } else if self.composer.composer.is_focused(window) {
            if self.composer.active_command().is_some() {
                return;
            }
            self.composer.composer_focused = false;
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }
}
