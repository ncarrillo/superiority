use super::*;

impl ChatComponent {
    pub(in crate::app::client) fn panel(
        &self,
        channels: &ChannelComponent,
        settings: &SettingsComponent,
        chrome: &ChromeComponent,
        invitations: Option<Div>,
        affinity: &RosterAffinity,
        cx: &mut Context<SuperiorityView>,
    ) -> ui_workspace::ChannelChat {
        let now = Instant::now();
        let transcript = TranscriptChrome {
            show_membership: settings.show_membership,
            show_timestamps: settings.show_timestamps,
            affinity,
            assets: &chrome.ui_assets,
        };
        let mut panel = ui_workspace::ChannelChat::new(self.transcript_view(
            channels.active(),
            true,
            channels.chat_entry_reveal.as_ref(),
            &transcript,
            cx,
        ));
        if let Some(transition) = &channels.channel_transition {
            panel = panel.outgoing(
                self.transcript_view(transition.outgoing.as_ref(), false, None, &transcript, cx),
                channels.transition_progress(now),
            );
        }
        if let Some(invitations) = invitations {
            panel = panel.overlay(invitations);
        }
        panel
    }
}
