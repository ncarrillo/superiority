use super::*;

impl ChatComponent {
    pub(in crate::app::client) fn panel(
        &self,
        channels: &ChannelComponent,
        settings: &SettingsComponent,
        chrome: &ChromeComponent,
        invitations: Option<Div>,
    ) -> ui_workspace::ChannelChat {
        let now = Instant::now();
        let mut panel = ui_workspace::ChannelChat::new(self.transcript_view(
            channels.active(),
            true,
            settings.show_membership,
            settings.show_timestamps,
            channels.chat_entry_reveal.as_ref(),
            &chrome.ui_assets,
        ));
        if let Some(transition) = &channels.channel_transition {
            panel = panel.outgoing(
                self.transcript_view(
                    transition.outgoing.as_ref(),
                    false,
                    settings.show_membership,
                    settings.show_timestamps,
                    None,
                    &chrome.ui_assets,
                ),
                channels.transition_progress(now),
            );
        }
        if let Some(invitations) = invitations {
            panel = panel.overlay(invitations);
        }
        panel
    }
}
