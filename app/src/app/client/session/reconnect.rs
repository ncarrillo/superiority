use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn reconnect(&mut self, cx: &mut Context<Self>) {
        let Some(commands) = self.runtime.commands.clone() else {
            return;
        };
        if matches!(
            self.warnings.warning_dialog,
            Some(WarningDialog::Disconnected { .. })
        ) {
            self.warnings.warning_closing = true;
            self.warnings.warning_hide_due = Some(Instant::now() + MODAL_CLOSE_DURATION);
        }
        self.connection.sign_out_requested = false;
        self.connection.signed_out = false;
        self.connection.starting = true;
        self.connection.error = None;
        self.connection.fill = 0.0;
        self.connection.floor = 0.0;
        self.connection.ceiling = 0.0;
        self.connection.progress_updated = Instant::now();
        self.open_connection_dialog();
        let mut channels = self
            .channels
            .tabs
            .iter()
            .filter_map(|tab| tab.channel.clone())
            .collect::<Vec<_>>();
        if channels.is_empty() {
            channels.push(ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL));
        }
        let _ = commands.send(ClientCommand::Connect {
            force_interactive: false,
            channels,
        });
        cx.notify();
    }
}
