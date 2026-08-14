use super::super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_text_inputs();
        if self.updates.update_dialog_visible {
            if event.keystroke.modifiers.platform
                && event.keystroke.key.eq_ignore_ascii_case("c")
                && let Some(text) = self
                    .updates
                    .update_notes_selection
                    .selected_text(&self.updates.update_model.notes)
            {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            } else if event.keystroke.key == "escape" {
                self.close_update_dialog(cx);
            }
            cx.stop_propagation();
            return;
        }
        if self.warnings.warning_dialog.is_some() {
            if event.keystroke.key == "escape"
                && matches!(
                    self.warnings.warning_dialog,
                    Some(WarningDialog::Channel { .. })
                )
            {
                self.begin_warning_close(cx);
            }
            cx.stop_propagation();
            return;
        }
        if self.connection.dialog_visible {
            cx.stop_propagation();
            return;
        }
        if event.keystroke.key == "escape" && self.overlays.active.is_some() {
            self.dismiss_overlay(window, cx);
            cx.stop_propagation();
            return;
        }
        if self.overlays.active == Some(Overlay::Friends)
            && self.social.social_detail_open
            && self.social.conversation_focused
        {
            if matches!(event.keystroke.key.as_str(), "enter" | "return") {
                if self.social.send_message(
                    self.connection.stage == ConnectionStage::Connected,
                    self.runtime.commands.as_ref(),
                ) {
                    cx.notify();
                }
                cx.stop_propagation();
            }
            return;
        }
        if self.overlays.active == Some(Overlay::Join) && self.join.join_focused {
            let handled = match event.keystroke.key.as_str() {
                "up" => {
                    let count = self.join.rows(&self.channels.tabs).len();
                    if count > 0 {
                        self.join.join_selected = (self.join.join_selected + count - 1) % count;
                        self.join
                            .join_scroll
                            .scroll_to_item(self.join.join_selected);
                    }
                    true
                }
                "down" => {
                    let count = self.join.rows(&self.channels.tabs).len();
                    if count > 0 {
                        self.join.join_selected = (self.join.join_selected + 1) % count;
                        self.join
                            .join_scroll
                            .scroll_to_item(self.join.join_selected);
                    }
                    true
                }
                "enter" | "return" => {
                    let rows = self.join.rows(&self.channels.tabs);
                    if let Some(row) = rows.get(self.join.join_selected) {
                        self.join_channel_target(row.target.clone(), cx);
                    } else {
                        let title = self.join.join_query.trim().to_owned();
                        if !title.is_empty() {
                            self.join_channel(title, cx);
                        }
                    }
                    true
                }
                _ => false,
            };
            if handled {
                cx.stop_propagation();
                cx.notify();
            }
            return;
        }
        if self.overlays.active.is_some() {
            cx.stop_propagation();
            return;
        }
        if event.keystroke.modifiers.platform
            && event.keystroke.key.eq_ignore_ascii_case("c")
            && !self.text_input_focused(window)
            && self.chat.transcript.selection.has_selection()
        {
            let rows = self
                .channels
                .active()
                .into_iter()
                .flat_map(|channel| channel.transcript.iter().enumerate())
                .filter(|(_, line)| {
                    self.settings.show_membership || !matches!(line, ChatLine::Membership { .. })
                })
                .map(|(index, line)| {
                    let line = shared_transcript_line(line);
                    (index, ui_chat::transcript_text(&line))
                })
                .collect::<Vec<_>>();
            if let Some(text) = self.chat.transcript.selection.selected_text(&rows) {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                cx.stop_propagation();
                return;
            }
        }
        if self.roster.roster.focused && self.overlays.active.is_none() {
            let handled = match event.keystroke.key.as_str() {
                "escape" => {
                    if self.active_roster_filter().is_empty() {
                        self.roster.roster.focused = false;
                        self.focus_handle.focus(window, cx);
                    } else {
                        self.roster.roster_input.clear();
                        self.set_roster_filter(String::new());
                    }
                    true
                }
                "up" => {
                    self.move_roster_selection(-1);
                    true
                }
                "down" => {
                    self.move_roster_selection(1);
                    true
                }
                "home" => {
                    self.select_roster_index(0);
                    true
                }
                "end" => {
                    let count = self.visible_roster_users().len();
                    if count > 0 {
                        self.select_roster_index(count - 1);
                    }
                    true
                }
                "pageup" => {
                    let rows = (f32::from(self.roster_base_scroll().bounds().size.height)
                        / (ROSTER_ROW_HEIGHT + ROSTER_ROW_GAP))
                        .floor()
                        .max(1.0) as isize;
                    self.move_roster_selection(-rows);
                    true
                }
                "pagedown" => {
                    let rows = (f32::from(self.roster_base_scroll().bounds().size.height)
                        / (ROSTER_ROW_HEIGHT + ROSTER_ROW_GAP))
                        .floor()
                        .max(1.0) as isize;
                    self.move_roster_selection(rows);
                    true
                }
                _ => false,
            };
            if handled {
                cx.stop_propagation();
                cx.notify();
            }
            return;
        }
        if !self.composer.composer_focused || self.channels.active().is_none() {
            return;
        }
        if matches!(event.keystroke.key.as_str(), "enter" | "return") {
            self.send_message(cx);
            cx.stop_propagation();
        }
    }
}
