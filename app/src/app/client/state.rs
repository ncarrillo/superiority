use super::*;

pub(super) struct SuperiorityView {
    pub(super) focus_handle: FocusHandle,
    pub(super) runtime: ClientRuntime,
    /// The focused product's session. Everything a game owns lives in here; the
    /// fields below it are shared across every product.
    pub(super) session: ProductSession,
    /// Which product is in front. The games list is the switcher.
    pub(super) focused: Product,
    /// Sessions that are still connected but not on screen. They keep taking
    /// events, which is how a background game accumulates unread.
    pub(super) suspended: BTreeMap<Product, ProductSession>,
    pub(super) warnings: WarningComponent,
    pub(super) updates: UpdateComponent,
    pub(super) overlays: OverlayComponent,
    pub(super) settings: SettingsComponent,
    pub(super) chrome: ChromeComponent,
    /// The game picker, which only exists under its own flag: it is dressed in
    /// design-time data rather than anything a session knows.
    pub(super) games: GamesComponent,
    /// The shared modal demo, likewise flag-only and dressed in fixture data.
    pub(super) modal_preview: ModalPreviewComponent,
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
        for session in std::iter::once(&self.session).chain(self.suspended.values()) {
            if let Some(commands) = &session.commands {
                let _ = commands.send(ClientCommand::Quit);
            }
        }
    }
}
