use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn send_message(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let body = self.session.composer.composer.content().trim().to_owned();
        if body.is_empty() {
            return;
        }
        // a line that opens with a slash is an instruction, even with the popup
        // dismissed. sending "/join general" to the room would be the one thing
        // you did not mean, and a command we do not have is answered in the
        // transcript rather than typed at the channel.
        if let Some(word) = command_word(&body) {
            let command = parse_command(&body, body.len())
                .filter(|command| command.kind != CommandKind::Mention);
            let notice = command.is_none().then(|| unknown_command_notice(word));
            let query = command
                .as_ref()
                .map(|command| command.query.clone())
                .filter(|query| !query.is_empty());
            let kind = command.map(|command| command.kind);
            self.clear_composer_command(cx);
            match (kind, query) {
                (Some(CommandKind::Join), Some(query)) => {
                    let target = self.session.join.target_for_query(&query);
                    self.join_channel_target(target, cx);
                }
                // `/w somebody` opens the thread; `/w somebody hello` opens it
                // and says hello. whispering a name nobody offered is still a
                // whisper — the service resolves it the way the popup's rows do
                (Some(CommandKind::Whisper), Some(query)) => {
                    if let Some((peer, message)) = split_whisper(
                        &self.session.channels.tabs,
                        &self.session.social.friends,
                        &self.session.social.conversations,
                        &query,
                    ) {
                        self.begin_whisper(peer, window, cx);
                        self.send_opening_whisper(message);
                    }
                }
                // `/p` on its own is the scope; `/p ready in 30s` is one line
                // to the party without moving the field off the channel, so
                // neither form can lose what you typed
                (Some(CommandKind::Party), message) => {
                    if self.party_channel_index().is_none() {
                        self.append_error_line(no_party_notice());
                    } else if let Some(message) = message {
                        self.send_to_party(message, cx);
                    } else {
                        self.session.composer.party_scope = !self.session.composer.party_scope;
                    }
                }
                _ => {}
            }
            if let Some(notice) = notice {
                self.append_error_line(notice);
            }
            cx.notify();
            return;
        }
        // the field is addressed to the party, so this line goes there and the
        // scope stays for the next one
        if self.session.composer.party_scope {
            self.send_to_party(body, cx);
            return;
        }
        if self.runtime.live_mode {
            let channel_index = self
                .session
                .channels
                .active()
                .and_then(|channel| channel.channel_index);
            if let Some(channel_index) = channel_index
                && let Some(commands) = &self.session.commands
            {
                let _ = commands.send(ClientCommand::SendMessage {
                    channel_index,
                    body,
                });
                self.session.composer.composer.clear();
                cx.notify();
            }
            return;
        }
        if self.session.channels.active_tab < self.session.channels.tabs.len() {
            self.append_chat_line(
                self.session.channels.active_tab,
                ChatLine::Message {
                    time: "7:36 PM".to_owned(),
                    sender: UiUser::fixture(0),
                    text: body,
                },
            );
            self.session.composer.composer.clear();
            Self::trace("sent composer message");
            cx.notify();
        }
    }

    /// the wire index of the party channel, which is what makes the party
    /// addressable at all: it is an ordinary joined channel that happens to
    /// have no name.
    pub(in crate::app::client) fn party_channel_index(&self) -> Option<u8> {
        self.session
            .channels
            .tabs
            .iter()
            .find(|tab| tab.channel.as_ref() == Some(&ChatChannel::Party))
            .and_then(|tab| tab.channel_index)
    }

    /// the party's own roster, once it is actually joined. this is what the
    /// dock draws, and its absence is what says there is no party at all.
    pub(in crate::app::client) fn party_members(&self) -> Option<(&[UiUser], Option<u32>)> {
        self.session
            .channels
            .tabs
            .iter()
            .find(|tab| {
                tab.channel.as_ref() == Some(&ChatChannel::Party) && tab.channel_index.is_some()
            })
            .map(|tab| (tab.users.as_slice(), tab.local_member_handle))
    }

    /// forming a party from the dock. the flow the design draws — the dashed
    /// slot opens the same people autocomplete `/w` uses — needs an outbound
    /// `Chat::CreateAndInviteRequest`, which this client cannot write yet, so
    /// the dock says so rather than doing nothing.
    pub(in crate::app::client) fn invite_to_party(&mut self, cx: &mut Context<Self>) {
        self.append_error_line(
            "Inviting to a party is not implemented yet. Invite from the game client.".to_owned(),
        );
        cx.notify();
    }

    /// the same for the queue: `Party::BeginReadyProcess` is a command this
    /// client can name but not yet send.
    pub(in crate::app::client) fn begin_party_queue(&mut self, cx: &mut Context<Self>) {
        self.append_error_line(
            "Queueing from Superiority is not implemented yet. Queue from the game client."
                .to_owned(),
        );
        cx.notify();
    }

    /// party talk stays inline in whatever transcript you are reading, so this
    /// sends to the party's own channel without moving you to its tab.
    fn send_to_party(&mut self, body: String, cx: &mut Context<Self>) {
        let Some(channel_index) = self.party_channel_index() else {
            self.append_error_line(no_party_notice());
            self.session.composer.party_scope = false;
            cx.notify();
            return;
        };
        if let Some(commands) = &self.session.commands {
            let _ = commands.send(ClientCommand::SendMessage {
                channel_index,
                body,
            });
        }
        self.session.composer.composer.clear();
        cx.notify();
    }

    pub(in crate::app::client) fn leave_party_scope(&mut self, cx: &mut Context<Self>) {
        if !self.session.composer.party_scope {
            return;
        }
        self.session.composer.party_scope = false;
        cx.notify();
    }

    /// the scope only exists while the party does. leaving one, or being
    /// dropped from it, puts the field back on the channel rather than leaving
    /// it green and pointed at nothing.
    pub(in crate::app::client) fn sync_party_scope(&mut self) {
        if self.session.composer.party_scope && self.party_channel_index().is_none() {
            self.session.composer.party_scope = false;
        }
        // the party has no tab in the strip, so it must never be the selected
        // one — there would be nothing lit to say where you are
        if self
            .session
            .channels
            .active()
            .is_some_and(|tab| tab.channel.as_ref() == Some(&ChatChannel::Party))
            && let Some(index) = self
                .session
                .channels
                .tabs
                .iter()
                .position(|tab| tab.channel.as_ref() != Some(&ChatChannel::Party))
        {
            self.session.channels.active_tab = index;
        }
    }

    pub(in crate::app::client) fn command_results(&self) -> Option<CommandResults> {
        self.results_for(&self.session.composer.active_command()?)
    }

    /// the same, for a line the field no longer holds — a closing popup draws
    /// the results it was closed on.
    pub(in crate::app::client) fn results_for_line(
        &self,
        line: &str,
        cursor: usize,
    ) -> Option<CommandResults> {
        self.results_for(&parse_command(line, cursor)?)
    }

    fn results_for(&self, command: &ChatCommand) -> Option<CommandResults> {
        let mut create = None;
        let rows = match command.kind {
            // the party scope is the whole answer — there is nobody to pick, so
            // the field paints itself green and no list opens
            CommandKind::Party => return None,
            CommandKind::Join => {
                let (rows, offer) = self.session.join.command_matches(
                    &self.session.channels.tabs,
                    &command.query,
                    COMMAND_RESULTS,
                );
                create = offer;
                rows.into_iter().map(CommandRow::Channel).collect()
            }
            CommandKind::Whisper => whisper_candidates(
                &self.session.channels.tabs,
                self.session.channels.active_tab,
                &self.session.social.friends,
                &self.session.social.conversations,
                &command.query,
                COMMAND_RESULTS,
            )
            .into_iter()
            .map(CommandRow::Person)
            .collect(),
            CommandKind::Mention => mention_candidates(
                self.session.channels.active(),
                &command.query,
                COMMAND_RESULTS,
            )
            .into_iter()
            .map(CommandRow::Person)
            .collect(),
        };
        let results = CommandResults {
            kind: command.kind,
            rows,
            create,
            query: command.query.clone(),
            span: command.span.clone(),
        };
        (!results.is_empty()).then_some(results)
    }

    /// takes a row by index — the same path for a click and for the return key,
    /// so a row does the one thing wherever it is taken from.
    pub(in crate::app::client) fn take_command_row(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(action) = self
            .command_results()
            .and_then(|results| results.action(index))
        else {
            return;
        };
        self.perform_command_action(action, window, cx);
    }

    pub(in crate::app::client) fn perform_command_action(
        &mut self,
        action: CommandAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            CommandAction::Join(target) => {
                self.clear_composer_command(cx);
                self.join_channel_target(target, cx);
            }
            CommandAction::Whisper(peer) => self.begin_whisper(peer, window, cx),
            CommandAction::Mention { name, span } => {
                self.insert_mention(&name, &span);
                cx.notify();
            }
        }
    }

    /// takes you to the conversation. `/w` is a way *into* the social panel
    /// rather than a mode the composer wears — the thread, its history, and an
    /// input already addressed to that person are all there — so the composer
    /// itself goes back to addressing the channel you are reading.
    pub(in crate::app::client) fn begin_whisper(
        &mut self,
        peer: WhisperPeer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_composer_command(cx);
        self.session.composer.composer_focused = false;
        self.session.roster.roster.focused = false;
        // the panel slides list-to-conversation only when the list was the
        // thing on screen; opened cold — or already on a thread — it has
        // nothing to slide away from
        let sliding = self.overlays.active == Some(Overlay::Friends)
            && !self.overlays.closing
            && !self.session.social.social_detail_open;
        self.overlays.active = Some(Overlay::Friends);
        self.overlays.closing = false;
        // a close animation may still be counting down over this panel
        self.overlays.epoch = self.overlays.epoch.wrapping_add(1);
        self.session
            .social
            .present_conversation(peer, sliding, window, cx);
        cx.notify();
    }

    /// the message a `/w somebody hello` carried in with it. it goes out
    /// through the conversation's own field, so there is one path a whisper
    /// takes and one place it is recorded; unsent — offline, say — it is left
    /// sitting in that field rather than dropped.
    fn send_opening_whisper(&mut self, message: String) {
        if message.is_empty() {
            return;
        }
        self.session.social.conversation_input.set_content(message);
        let connected = self.session.connection.stage == ConnectionStage::Connected;
        let commands = self.session.commands.clone();
        self.session
            .social
            .send_message(connected, commands.as_ref());
    }

    /// writes the name over the token being typed and leaves the caret after
    /// it, so the sentence carries on where it left off.
    pub(in crate::app::client) fn insert_mention(&mut self, name: &str, span: &Range<usize>) {
        let line = &self.session.composer.command_line;
        if span.end > line.len() {
            return;
        }
        let mention = format!("@{name} ");
        let caret = span.start + mention.len();
        let line = format!("{}{mention}{}", &line[..span.start], &line[span.end..]);
        self.session.composer.composer.set_content(line);
        self.session.composer.composer.set_cursor(caret);
    }

    /// puts the composer into `/join`, focused and empty of anything else. the
    /// button that used to open the dialog now writes the command, so there is
    /// one way into a channel rather than two.
    pub(in crate::app::client) fn open_join_command(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_open = self.session.composer.active_command().is_some();
        self.session.roster.roster.focused = false;
        self.session.join.group_search_due = None;
        self.session.join.group_search.clear();
        self.session.composer.composer.set_content(command_prefix());
        self.session.composer.command_dismissed = false;
        self.session.composer.command_selected = 0;
        if !was_open {
            self.session.composer.command_epoch =
                self.session.composer.command_epoch.wrapping_add(1);
        }
        self.cancel_command_close();
        self.session.composer.composer_focused = true;
        self.session.composer.composer.focus(window, cx);
        cx.notify();
    }

    /// completes the highlighted name into the field, leaving the popup open on
    /// the narrowed query. the command state is left for `sync_text_inputs` to
    /// pick up, the same as it would a typed keystroke.
    pub(in crate::app::client) fn complete_composer_command(&mut self, line: &str) {
        self.session.composer.composer.set_content(line);
    }

    pub(in crate::app::client) fn clear_composer_command(&mut self, cx: &mut Context<Self>) {
        let closing = self.command_results();
        self.session.composer.composer.clear();
        self.session.composer.command_line.clear();
        self.session.composer.command_selected = 0;
        self.session.composer.command_dismissed = false;
        if let Some(results) = closing {
            self.begin_command_close(results, cx);
        }
    }

    /// keeps the popup on screen for one animation after the line that built it
    /// is gone.
    pub(in crate::app::client) fn begin_command_close(
        &mut self,
        results: CommandResults,
        cx: &mut Context<Self>,
    ) {
        self.session.composer.command_closing = Some(results);
        self.session.composer.command_close_epoch =
            self.session.composer.command_close_epoch.wrapping_add(1);
        let epoch = self.session.composer.command_close_epoch;
        let executor = cx.background_executor().clone();
        cx.spawn(async move |entity, cx| {
            // the fade starts on the next frame, so the row outlives the
            // animation rather than being torn out from under its last frames
            executor
                .timer(COMMAND_CLOSE_DURATION + COMMAND_CLOSE_SLACK)
                .await;
            entity
                .update(cx, |this, cx| {
                    if this.session.composer.command_close_epoch == epoch {
                        this.session.composer.command_closing = None;
                        cx.notify();
                    }
                })
                .ok();
        })
        .detach();
    }

    /// drops a fade that a newly typed command has overtaken.
    pub(in crate::app::client) fn cancel_command_close(&mut self) {
        if self.session.composer.command_closing.take().is_some() {
            self.session.composer.command_close_epoch =
                self.session.composer.command_close_epoch.wrapping_add(1);
        }
    }
}
