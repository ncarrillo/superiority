use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn reconnect(&mut self, cx: &mut Context<Self>) {
        let Some(commands) = self.session.commands.clone() else {
            return;
        };
        if matches!(
            self.warnings.warning_dialog,
            Some(WarningDialog::Disconnected { .. })
        ) {
            self.warnings.warning_closing = true;
            self.warnings.warning_hide_due = Some(Instant::now() + MODAL_CLOSE_DURATION);
        }
        let force_interactive = self.session.connection.signed_out
            || (self.session.wc3().is_some()
                && self
                    .session
                    .connection
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("Classic AuthSession rejected")));
        let expected_battle_tag = self.runtime.authoritative_battle_tag.clone();
        let expected_account_id = self.runtime.authoritative_account_id;
        self.session.connection.begin_attempt();
        self.open_connection_dialog();
        let channels = if self.session.is_sc2() {
            let mut channels = self
                .session
                .channels
                .tabs
                .iter()
                .filter_map(|tab| tab.channel.clone())
                .collect::<Vec<_>>();
            if channels.is_empty() {
                channels.push(ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL));
            }
            channels
        } else {
            Vec::new()
        };
        let _ = commands.send(ClientCommand::Connect {
            force_interactive,
            expected_account_id,
            expected_battle_tag,
            channels,
        });
        cx.notify();
    }
}
