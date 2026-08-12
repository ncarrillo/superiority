use super::*;

mod friend;
mod group;
mod whisper;

impl SocialComponent {
    pub(super) fn rows(
        &self,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> Vec<AnyElement> {
        let whisper_peers = self.conversations.keys().cloned().collect::<Vec<_>>();
        let mut online = self
            .friends
            .iter()
            .filter(|friend| {
                friend.is_online() && !whisper_peers.iter().any(|peer| peer == &friend.name)
            })
            .collect::<Vec<_>>();
        let mut offline = self
            .friends
            .iter()
            .filter(|friend| {
                !friend.is_online() && !whisper_peers.iter().any(|peer| peer == &friend.name)
            })
            .collect::<Vec<_>>();
        online.sort_by_key(|friend| friend.name.to_ascii_lowercase());
        offline.sort_by_key(|friend| friend.name.to_ascii_lowercase());

        let mut rows = Vec::new();
        if !whisper_peers.is_empty() {
            rows.push(self.group_header(
                SOCIAL_GROUP_WHISPERS,
                "Whispers",
                whisper_peers.len(),
                true,
                cx,
            ));
            if !self.social_collapsed[SOCIAL_GROUP_WHISPERS] {
                rows.push(div().h(px(8.0)).flex_shrink_0().into_any_element());
                rows.extend(
                    whisper_peers
                        .iter()
                        .enumerate()
                        .map(|(index, peer)| self.whisper_row(index, peer, cx)),
                );
            }
            rows.push(div().h(px(16.0)).flex_shrink_0().into_any_element());
        }

        let friend_count = online.len() + offline.len();
        rows.push(self.group_header(SOCIAL_GROUP_FRIENDS, "Friends", friend_count, false, cx));
        if self.friends.is_empty() && !self.social_collapsed[SOCIAL_GROUP_FRIENDS] {
            rows.push(
                div()
                    .h(px(54.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px(px(14.0))
                    .text_size(px(11.5))
                    .text_color(rgb(0x7d8fa8))
                    .child("No Battle.net friends found.")
                    .into_any_element(),
            );
        } else if !self.social_collapsed[SOCIAL_GROUP_FRIENDS] {
            rows.push(div().h(px(8.0)).flex_shrink_0().into_any_element());
            let mut row_id = 0;
            for (section, entries, dimmed) in
                [("Online", &online, false), ("Offline", &offline, true)]
            {
                if entries.is_empty() {
                    continue;
                }
                rows.push(
                    div()
                        .h(px(22.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .px(px(8.0))
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(11.5))
                        .text_color(rgb(0x7d8fa8))
                        .child(section)
                        .into_any_element(),
                );
                for friend in entries {
                    rows.push(self.friend_row(row_id, friend, dimmed, chrome, cx));
                    row_id += 1;
                }
            }
        }
        rows
    }
}
