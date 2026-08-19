use super::*;

mod friend;
mod group;
mod whisper;

use friend::friend_row;

impl SocialComponent {
    /// the social list as one column: whispers on top in purple, then friends
    /// ordered online-first. refinement N deleted the online/offline
    /// sub-headers, so the section rules are the only headings left.
    pub(super) fn rows(
        &self,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> Vec<AnyElement> {
        let whisper_peers = self.conversations.keys().cloned().collect::<Vec<_>>();
        let friends = friend_order(&self.friends, &whisper_peers);

        let mut rows = Vec::new();
        if !whisper_peers.is_empty() {
            let unread = self.whisper_unread.values().sum::<usize>();
            rows.push(self.section_header(
                SOCIAL_GROUP_WHISPERS,
                "WHISPERS",
                (unread > 0).then(|| format!("{unread} unread")),
                true,
                cx,
            ));
            if !self.social_collapsed[SOCIAL_GROUP_WHISPERS] {
                rows.extend(
                    whisper_peers
                        .iter()
                        .enumerate()
                        .map(|(index, peer)| self.whisper_row(index, peer, chrome, cx)),
                );
            }
            rows.push(section_gap());
        }

        rows.push(self.section_header(
            SOCIAL_GROUP_FRIENDS,
            "FRIENDS",
            Some(online_summary(&friends)),
            false,
            cx,
        ));
        if self.social_collapsed[SOCIAL_GROUP_FRIENDS] {
            return rows;
        }
        if self.friends.is_empty() {
            rows.push(
                div()
                    .h(px(ROSTER_ROW_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px(px(SOCIAL_ROW_INSET))
                    .text_size(px(11.5))
                    .text_color(rgb(MUTED))
                    .child("No Battle.net friends found.")
                    .into_any_element(),
            );
            return rows;
        }
        rows.extend(
            friends
                .iter()
                .enumerate()
                .map(|(row_id, friend)| friend_row(row_id, friend, chrome, cx)),
        );
        rows
    }
}

/// the section rules carry no padding, so the gap between one section and the
/// next lives here.
fn section_gap() -> AnyElement {
    div().h(px(6.0)).flex_shrink_0().into_any_element()
}
