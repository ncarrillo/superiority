use super::*;

impl SettingsComponent {
    pub(super) fn privacy_settings_page(
        &self,
        mut page: Stateful<Div>,
        blocked_accounts: &[BlockedAccount],
        window: &mut Window,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        let mut blocked_list = div()
            .id("blocked-accounts-scroll")
            .size_full()
            .flex()
            .flex_col()
            .p(px(6.0))
            .gap(px(4.0))
            .overflow_y_scroll()
            .track_scroll(&self.privacy_scroll)
            .bg(rgba(0x04070bc7))
            .border_1()
            .border_color(rgba(0x144f78d9));
        if blocked_accounts.is_empty() {
            blocked_list = blocked_list.child(
                div()
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .px(px(10.0))
                    .text_size(px(11.0))
                    .text_color(rgb(0x7d8fa8))
                    .child("No blocked Battle.net accounts."),
            );
        } else {
            for (index, account) in blocked_accounts.iter().enumerate() {
                let detail = account
                    .full_name
                    .as_ref()
                    .filter(|name| *name != &account.name)
                    .cloned();
                blocked_list = blocked_list.child(
                    div()
                        .id(("blocked-account", index))
                        .h(px(36.0))
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .px(px(10.0))
                        .bg(rgb(0x09121a))
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(11.5))
                                .text_color(rgb(0xd6e0f0))
                                .child(account.name.clone()),
                        )
                        .children(detail.map(|detail| {
                            div()
                                .text_size(px(9.8))
                                .text_color(rgb(0x7d8fa8))
                                .child(detail)
                        })),
                );
            }
        }
        let blocked_list = div()
            .id("blocked-accounts-viewport")
            .absolute()
            .left(px(22.0))
            .top(px(211.0))
            .w(px(650.0))
            .h(px(286.0))
            .child(blocked_list)
            .vertical_scrollbar_for(&self.privacy_scroll, window, cx);
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
                    .child("Privacy"),
            )
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(152.0))
                    .w(px(380.0))
                    .h(px(22.0))
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(13.0))
                    .text_color(rgb(0x6bc2f2))
                    .child("Blocked Battle.net accounts"),
            )
            .child(
                div()
                    .absolute()
                    .right(px(28.0))
                    .top(px(152.0))
                    .w(px(190.0))
                    .flex()
                    .justify_end()
                    .text_size(px(10.2))
                    .text_color(rgb(0x7d8fa8))
                    .child(format!("{} blocked", blocked_accounts.len())),
            )
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(178.0))
                    .w(px(560.0))
                    .text_size(px(10.2))
                    .text_color(rgb(0x7d8fa8))
                    .child("Accounts currently ignored by your Battle.net account."),
            )
            .child(blocked_list);
        page
    }
}
