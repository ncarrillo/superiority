use super::*;

mod appearance;
mod chat;
mod live;
mod privacy;

impl SettingsComponent {
    fn settings_checkbox_visual(
        &self,
        id: impl Into<ElementId>,
        animation_index: usize,
        checked: bool,
        chrome: &ChromeComponent,
    ) -> Stateful<Div> {
        let now = Instant::now();
        let amount = self
            .checkbox_animations
            .get(animation_index)
            .and_then(Option::as_ref)
            .map_or(if checked { 1.0 } else { 0.0 }, |animation| {
                animation.value(now)
            });
        ui_controls::checkbox(id, amount, chrome.ui_assets.checkbox_mark.clone())
    }

    pub(in crate::app::client) fn settings_page(
        &self,
        page_index: usize,
        opacity: f32,
        chrome: &ChromeComponent,
        blocked_accounts: &[BlockedAccount],
        live_url: Option<String>,
        live_error: Option<String>,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        let page = div()
            .id(("settings-page", page_index))
            .absolute()
            .left(px(234.0))
            .top(px(16.0))
            .w(px(694.0))
            .h(px(515.0))
            .opacity(opacity);
        match page_index {
            0 => self.appearance_settings_page(page, cx),
            1 => self.chat_settings_page(page, chrome, cx),
            2 => self.privacy_settings_page(page, blocked_accounts),
            _ => self.live_settings_page(page, chrome, live_url, live_error, cx),
        }
    }
}
