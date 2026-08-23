use super::super::*;

impl SuperiorityView {
    /// the update dialog's keys, wherever it is open — over the client window
    /// or over the picker: ⌘C copies the selected notes, Escape closes, and
    /// everything else stops here so nothing underneath answers. `true` when
    /// the dialog took the key.
    pub(in crate::app::client) fn update_dialog_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.updates.update_dialog_visible {
            return false;
        }
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
        true
    }

    pub(in crate::app::client) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if superiority_core::trace_enabled() {
            // every link in the chain that has to hold for typing to reach the
            // composer, in one line
            let composer = if let Some(wc3) = self.session.wc3() {
                &wc3.composer
            } else if let Some(scr) = self.session.scr() {
                &scr.composer
            } else {
                &self.session.composer.composer
            };
            Self::trace(format_args!(
                "key {:?} product={:?} focus={} content={:?} cursor={}",
                event.keystroke.key,
                self.focused,
                composer.is_focused(window),
                composer.content(),
                composer.cursor(),
            ));
        }
        self.sync_text_inputs(cx);
        if self.update_dialog_key(event, cx) {
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
        if self.session.connection.dialog_visible {
            cx.stop_propagation();
            return;
        }
        if event.keystroke.key == "escape" && self.overlays.active.is_some() {
            self.dismiss_overlay(window, cx);
            cx.stop_propagation();
            return;
        }
        if self.overlays.active == Some(Overlay::Friends)
            && self.session.social.social_detail_open
            && self.session.social.conversation_focused
        {
            if matches!(event.keystroke.key.as_str(), "enter" | "return") {
                let connected = self.session.connection.stage == ConnectionStage::Connected;
                let commands = self.session.commands.clone();
                if self
                    .session
                    .social
                    .send_message(connected, commands.as_ref())
                {
                    cx.notify();
                }
                cx.stop_propagation();
            }
            return;
        }
        if self.overlays.active.is_some() {
            cx.stop_propagation();
            return;
        }
        if self.focused == Product::Remastered {
            self.on_scr_key_down(event, window, cx);
            return;
        }
        if self.focused == Product::Warcraft3 {
            self.on_wc3_key_down(event, window, cx);
            return;
        }
        if event.keystroke.modifiers.platform
            && event.keystroke.key.eq_ignore_ascii_case("c")
            && !self.text_input_focused(window)
            && self.session.chat.transcript.selection.has_selection()
        {
            let assets = self.chrome.ui_assets.clone();
            let rows = self
                .session
                .channels
                .active()
                .into_iter()
                .flat_map(|channel| {
                    let online = channel.users.len();
                    channel
                        .transcript
                        .iter()
                        .enumerate()
                        .map(move |(index, line)| (index, line, online, &channel.users))
                })
                .filter(|(_, line, _, _)| {
                    self.settings.show_membership
                        || !matches!(line, ChatLine::Membership { .. } | ChatLine::Digest { .. })
                })
                .map(|(index, line, online, roster)| {
                    let line = shared_transcript_line(line, online, roster, None, &assets);
                    (index, ui_chat::transcript_text(&line))
                })
                .collect::<Vec<_>>();
            if let Some(text) = self.session.chat.transcript.selection.selected_text(&rows) {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                cx.stop_propagation();
                return;
            }
        }
        if self.session.roster.roster.focused && self.overlays.active.is_none() {
            let handled = match event.keystroke.key.as_str() {
                "escape" => {
                    if self.active_roster_filter().is_empty() {
                        self.session.roster.roster.focused = false;
                        self.focus_handle.focus(window, cx);
                    } else {
                        self.session.roster.roster_input.clear();
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
        if !self.session.composer.composer_focused {
            return;
        }
        // while a command is being typed the composer answers to the popup
        // above it, not to the transcript below.
        if let Some(results) = self.command_results() {
            let selected = self.session.composer.command_selected;
            let handled = match event.keystroke.key.as_str() {
                "up" => {
                    self.session.composer.command_selected = results.step(selected, -1);
                    true
                }
                "down" => {
                    self.session.composer.command_selected = results.step(selected, 1);
                    true
                }
                "tab" => {
                    if let Some(line) = results.completion(selected) {
                        self.complete_composer_command(&line);
                    } else if let Some(action) = results.action(selected) {
                        // a mention has no line to complete — taking it is the
                        // completion
                        self.perform_command_action(action, window, cx);
                    }
                    true
                }
                "escape" => {
                    self.session.composer.command_dismissed = true;
                    self.begin_command_close(results, cx);
                    true
                }
                "enter" | "return" => {
                    if let Some(action) = results.action(selected) {
                        self.perform_command_action(action, window, cx);
                    }
                    true
                }
                _ => false,
            };
            if handled {
                cx.stop_propagation();
                cx.notify();
                return;
            }
        }
        // with no popup open, escape is what leaves the party scope — the same
        // exit the token and a second `/p` give
        if event.keystroke.key == "escape" && self.session.composer.party_scope {
            self.leave_party_scope(cx);
            cx.stop_propagation();
            return;
        }
        if matches!(event.keystroke.key.as_str(), "enter" | "return") {
            self.send_message(window, cx);
            cx.stop_propagation();
        }
    }
}
