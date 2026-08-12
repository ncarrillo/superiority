use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn send_message(&mut self, cx: &mut Context<Self>) {
        let body = self.composer.composer.content().trim().to_owned();
        if body.is_empty() {
            return;
        }
        if self.runtime.live_mode {
            if let Some(channel_index) = self
                .channels
                .active()
                .and_then(|channel| channel.channel_index)
                && let Some(commands) = &self.runtime.commands
            {
                let _ = commands.send(ClientCommand::SendMessage {
                    channel_index,
                    body,
                });
                self.composer.composer.clear();
                cx.notify();
            }
            return;
        }
        if self.channels.active_tab < self.channels.tabs.len() {
            self.append_chat_line(
                self.channels.active_tab,
                ChatLine::Message {
                    time: "7:36 PM".to_owned(),
                    sender: UiUser::fixture(0),
                    text: body,
                },
            );
            self.composer.composer.clear();
            Self::trace("sent composer message");
            cx.notify();
        }
    }
}
