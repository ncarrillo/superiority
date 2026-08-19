use super::*;

impl SettingsComponent {
    pub(super) fn appearance_settings_page(
        &self,
        mut page: Stateful<Div>,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        page = page
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(100.0))
                    .w(px(400.0))
                    .h(px(30.0))
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(20.0))
                    .child("Appearance"),
            )
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(160.0))
                    .w(px(260.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(12.5))
                    .text_color(rgb(0x6bc2f2))
                    .child("Chat background"),
            )
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(186.0))
                    .w(px(540.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .text_size(px(11.2))
                    .text_color(rgb(0x7d8fa8))
                    .child("Choose the scene shown beneath chat messages."),
            );
        for (index, background) in BACKGROUNDS.into_iter().enumerate() {
            let title = background.title;
            let path = background.path;
            let column = (index % 4) as f32;
            let row = index / 4;
            let selected = self.background == path;
            page = page.child(
                div()
                    .id(("settings-background", index))
                    .absolute()
                    .left(px(14.0 + column * 166.0))
                    .top(px(if row == 0 { 220.0 } else { 362.0 }))
                    .w(px(156.0))
                    .h(px(132.0))
                    .bg(if selected {
                        rgb(0x091c2b)
                    } else {
                        rgb(0x060c11)
                    })
                    .border_1()
                    .border_color(if selected {
                        rgb(0x6bc2f2)
                    } else {
                        rgba(BORDER_STRUCTURAL)
                    })
                    .rounded(px(2.0))
                    .cursor_pointer()
                    // selection reads as the bright stroke plus its glow — the
                    // stroke stays 1px in every state so nothing shifts.
                    .when(selected, |tile| tile.shadow(selection_glow()))
                    .hover(|style| style.border_color(rgb(BORDER_FOCUSED)))
                    .active(|style| style.opacity(0.82))
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        this.settings
                            .set_tooltip(SETTINGS_TOOLTIP_BACKGROUND_START + index, *hovered);
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.background = path;
                        preferences::save_background(index);
                        cx.notify();
                    }))
                    .child(
                        img(path)
                            .absolute()
                            .left(px(4.0))
                            .top(px(4.0))
                            .w(px(148.0))
                            .h(px(100.0))
                            .object_fit(ObjectFit::Fill),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(6.0))
                            .bottom(px(7.0))
                            .w(px(144.0))
                            .h(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(10.2))
                            .child(title),
                    ),
            );
        }
        page
    }
}
