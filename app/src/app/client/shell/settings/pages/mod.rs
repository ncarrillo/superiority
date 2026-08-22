use super::*;

mod appearance;
mod chat;
mod live;
mod privacy;

impl SettingsComponent {
    fn settings_checkbox_visual(
        &self,
        id: impl Into<ElementId>,
        variant: ui_shared_modal::ModalVariant,
        animation_index: usize,
        checked: bool,
    ) -> Stateful<Div> {
        let now = Instant::now();
        let amount = self
            .checkbox_animations
            .get(animation_index)
            .and_then(Option::as_ref)
            .map_or(if checked { 1.0 } else { 0.0 }, |animation| {
                animation.value(now)
            });
        // the modern checkbox, in the dialog's own dressing; the amount
        // still walks the mark in the way the legacy toggle grew its tick
        let mark = if amount <= 0.01 {
            ui_inputs::CheckMark::Empty
        } else {
            ui_inputs::CheckMark::Check(amount)
        };
        ui_inputs::checkbox(id, variant, mark, ui_inputs::FieldLife::Ready, false)
    }

    pub(in crate::app::client) fn settings_page(
        &self,
        page_index: usize,
        opacity: f32,
        product: Product,
        variant: ui_shared_modal::ModalVariant,
        live_url: Option<String>,
        live_error: Option<String>,
        window: &mut Window,
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
        match SettingsPage::shown(page_index) {
            SettingsPage::Appearance => {
                self.appearance_settings_page(page, product, variant, window, cx)
            }
            SettingsPage::Chat => self.chat_settings_page(page, variant, cx),
            SettingsPage::Live => self.live_settings_page(page, variant, live_url, live_error, cx),
        }
    }
}
