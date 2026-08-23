use super::super::*;

/// the roster filter never grows past this many characters; anything longer
/// is cut back in the field itself so the field and the filter agree
const ROSTER_FILTER_MAX_CHARS: usize = 64;

fn clamped_roster_filter(input: &ui_text_input::TextInput) -> String {
    let content = input.content();
    if content.chars().count() <= ROSTER_FILTER_MAX_CHARS {
        return content;
    }
    let clamped: String = content.chars().take(ROSTER_FILTER_MAX_CHARS).collect();
    input.set_content(clamped.clone());
    clamped
}

impl SuperiorityView {
    pub(in crate::app::client) fn sync_text_inputs(&mut self, cx: &mut Context<Self>) {
        if let Some(wc3) = self.session.wc3() {
            let roster_filter = clamped_roster_filter(&wc3.roster_input);
            let changed = wc3
                .active()
                .is_some_and(|channel| channel.roster_filter != roster_filter);
            if changed && let Some(wc3) = self.session.wc3_mut() {
                wc3.set_roster_filter(roster_filter);
            }
            return;
        }
        if let Some(scr) = self.session.scr() {
            let roster_filter = clamped_roster_filter(&scr.roster_input);
            let changed = scr
                .channel
                .as_ref()
                .is_some_and(|channel| channel.roster_filter != roster_filter);
            if changed && let Some(scr) = self.session.scr_mut() {
                scr.set_roster_filter(roster_filter);
            }
            return;
        }
        // a mention is the token under the caret, so moving the caret changes
        // what is being asked for even when the line has not changed
        let line = self.session.composer.composer.content();
        let cursor = self.session.composer.composer.cursor();
        if line != self.session.composer.command_line
            || cursor != self.session.composer.command_cursor
        {
            let typed = line != self.session.composer.command_line;
            let previous = std::mem::replace(&mut self.session.composer.command_line, line);
            let previous_cursor =
                std::mem::replace(&mut self.session.composer.command_cursor, cursor);
            // the popup was open on the previous line unless escape had already
            // shut it
            let was_open = !self.session.composer.command_dismissed
                && parse_command(&previous, previous_cursor).is_some();
            self.session.composer.command_selected = 0;
            if typed {
                // dismissal belongs to the line it was made on, so typing
                // brings the popup back rather than needing it retyped
                self.session.composer.command_dismissed = false;
            }
            if let Some(command) = parse_command(&self.session.composer.command_line, cursor) {
                if command.kind == CommandKind::Join {
                    self.session.join.schedule_group_search(&command.query);
                }
                if !was_open {
                    self.session.composer.command_epoch =
                        self.session.composer.command_epoch.wrapping_add(1);
                }
                self.cancel_command_close();
            } else if was_open
                && let Some(results) = self.results_for_line(&previous, previous_cursor)
            {
                self.begin_command_close(results, cx);
            }
        }

        let roster_filter = clamped_roster_filter(&self.session.roster.roster_input);
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
        let focused = self.session.composer.composer_focused
            && self.session.composer.composer.is_focused(window);
        if focused == self.session.composer.command_focused {
            return;
        }
        self.session.composer.command_focused = focused;
        if focused {
            if parse_command(
                &self.session.composer.command_line,
                self.session.composer.command_cursor,
            )
            .is_some()
            {
                self.session.composer.command_dismissed = false;
                self.session.composer.command_selected = 0;
                self.session.composer.command_epoch =
                    self.session.composer.command_epoch.wrapping_add(1);
                self.cancel_command_close();
            }
            return;
        }
        if let Some(results) = self.command_results() {
            self.session.composer.command_dismissed = true;
            self.begin_command_close(results, cx);
        }
    }

    pub(in crate::app::client) fn text_input_focused(&self, window: &Window) -> bool {
        if let Some(wc3) = self.session.wc3() {
            wc3.composer.is_focused(window) || wc3.roster_input.is_focused(window)
        } else if let Some(scr) = self.session.scr() {
            scr.composer.is_focused(window)
                || scr.roster_input.is_focused(window)
                || self.session.social.conversation_input.is_focused(window)
        } else {
            self.session.composer.composer.is_focused(window)
                || self.session.roster.roster_input.is_focused(window)
                || self.session.social.conversation_input.is_focused(window)
        }
    }

    pub(in crate::app::client) fn set_roster_pointer_focus(
        &mut self,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if hovered {
            if self.overlays.active.is_some()
                || self.session.connection.dialog_visible
                || self.warnings.warning_dialog.is_some()
                || self.updates.update_dialog_visible
                || self.session.channels.active().is_none()
            {
                return;
            }
            if self.session.roster.roster.focused
                && self.session.roster.roster_input.is_focused(window)
            {
                return;
            }
            // a command owns the keyboard until it is answered or dismissed
            if self.session.composer.active_command().is_some() {
                return;
            }
            self.session.roster.roster_input.focus(window, cx);
            self.session.composer.composer_focused = false;
            self.session.roster.roster.focused = true;
            cx.notify();
        } else if self.session.roster.roster_input.is_focused(window) {
            self.session.roster.roster.focused = false;
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
                || self.session.connection.dialog_visible
                || self.warnings.warning_dialog.is_some()
                || self.updates.update_dialog_visible
                || self.session.channels.active().is_none()
            {
                return;
            }
            if self.session.composer.composer_focused
                && self.session.composer.composer.is_focused(window)
            {
                return;
            }
            self.session.composer.composer.focus(window, cx);
            self.session.composer.composer_focused = true;
            self.session.roster.roster.focused = false;
            cx.notify();
        } else if self.session.composer.composer.is_focused(window) {
            if self.session.composer.active_command().is_some() {
                return;
            }
            self.session.composer.composer_focused = false;
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }
}
