use super::*;

pub(super) struct SuperiorityView {
    pub(super) focus_handle: FocusHandle,
    pub(super) runtime: ClientRuntime,
    pub(super) connection: ConnectionComponent,
    pub(super) warnings: WarningComponent,
    pub(super) updates: UpdateComponent,
    pub(super) composer: ComposerComponent,
    pub(super) join: JoinComponent,
    pub(super) channels: ChannelComponent,
    pub(super) chat: ChatComponent,
    pub(super) roster: RosterComponent,
    pub(super) overlays: OverlayComponent,
    pub(super) settings: SettingsComponent,
    pub(super) social: SocialComponent,
    pub(super) chrome: ChromeComponent,
    pub(super) _input_subscriptions: Vec<Subscription>,
}

impl Focusable for SuperiorityView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Drop for SuperiorityView {
    fn drop(&mut self) {
        if let Some(authenticator) = self.runtime.authenticator.take() {
            authenticator.dismiss();
        }
        if let Some(commands) = &self.runtime.commands {
            let _ = commands.send(ClientCommand::Quit);
        }
    }
}
