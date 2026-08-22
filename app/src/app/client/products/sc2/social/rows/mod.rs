use super::*;

mod friend;
mod group;
mod whisper;

use friend::friend_row;

impl SocialComponent {
    /// the social list as one column: whispers on top in purple, then friends
    /// ordered online-first. refinement N deleted the online/offline
    /// sub-headers, so the section rules are the only headings left.
    pub(in crate::app::client) fn rows(
        &self,
        variant: ui_shared_modal::ModalVariant,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> Vec<AnyElement> {
        let skin = SocialSkin::for_variant(variant);
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
                variant,
                cx,
            ));
            if !self.social_collapsed[SOCIAL_GROUP_WHISPERS] {
                rows.extend(
                    whisper_peers
                        .iter()
                        .enumerate()
                        .map(|(index, peer)| self.whisper_row(index, peer, variant, chrome, cx)),
                );
            }
            rows.push(section_gap());
        }

        rows.push(self.section_header(
            SOCIAL_GROUP_FRIENDS,
            "FRIENDS",
            Some(online_summary(&friends)),
            false,
            variant,
            cx,
        ));
        if self.social_collapsed[SOCIAL_GROUP_FRIENDS] {
            return rows;
        }
        if self.friends.is_empty() {
            rows.push(
                div()
                    .h(px(if variant == ui_shared_modal::ModalVariant::Reforged {
                        ui_wc3_theme::ROSTER_ROW_HEIGHT
                    } else {
                        ROSTER_ROW_HEIGHT
                    }))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px(px(SOCIAL_ROW_INSET))
                    .text_size(px(11.5))
                    .font_family(skin.body_font)
                    .text_color(rgb(skin.muted))
                    .child("No Battle.net friends found.")
                    .into_any_element(),
            );
            return rows;
        }
        rows.extend(
            friends
                .iter()
                .enumerate()
                .map(|(row_id, friend)| friend_row(row_id, friend, variant, chrome, cx)),
        );
        rows
    }
}

pub(super) fn social_portrait(
    friend: &UiFriend,
    variant: ui_shared_modal::ModalVariant,
    chrome: &ChromeComponent,
) -> Div {
    if variant == ui_shared_modal::ModalVariant::Sc2 {
        let user = friend.roster_user(&chrome.ui_assets);
        return ui_roster::framed_portrait(
            user.portrait.as_ref(),
            &chrome.ui_assets,
            ui_roster::PORTRAIT_FRAME,
            ui_roster::PORTRAIT_FACE,
        );
    }
    let skin = SocialSkin::for_variant(variant);
    let (frame, face, fallback, edge, fill) = match variant {
        ui_shared_modal::ModalVariant::Remastered => (
            28.0,
            24.0,
            "images/products/scr/avatars/avatar_default_scrlogo.png",
            ui_scr_theme::BORDER_FOCUSED,
            ui_scr_theme::PANEL_BACKGROUND,
        ),
        ui_shared_modal::ModalVariant::Reforged => (
            30.0,
            28.0,
            "images/products/wc3/portraits/p003.png",
            ui_wc3_theme::STONE_BRIGHT,
            ui_wc3_theme::PANEL,
        ),
        ui_shared_modal::ModalVariant::Sc2 => unreachable!("handled above"),
    };
    let source = friend
        .portrait
        .as_ref()
        .and_then(|portrait| match portrait {
            Portrait::Image(source) => Some(source.clone()),
            Portrait::Atlas { .. } => None,
        });
    div()
        .relative()
        .size(px(frame))
        .flex_shrink_0()
        .border_1()
        .border_color(rgb(edge))
        .bg(rgb(fill))
        .child(
            img(source.unwrap_or_else(|| fallback.into()))
                .absolute()
                .left(px((frame - face) / 2.0))
                .top(px((frame - face) / 2.0))
                .size(px(face))
                .object_fit(ObjectFit::Cover),
        )
        .text_color(rgb(skin.text))
}

pub(super) fn social_presence_dot(presence: PresenceState, skin: SocialSkin) -> Div {
    let color = skin.presence_color(presence);
    let dot = div()
        .flex_shrink_0()
        .size(px(9.0))
        .rounded(px(4.5))
        .bg(rgb(color));
    if matches!(
        presence,
        PresenceState::Available | PresenceState::Busy | PresenceState::InGame
    ) {
        dot.shadow(vec![
            gpui::BoxShadow::new(px(0.0), px(0.0), rgba((color << 8) | 0xcc).into())
                .blur_radius(px(5.0)),
        ])
    } else {
        dot
    }
}

pub(super) fn social_person_row(
    friend: &UiFriend,
    variant: ui_shared_modal::ModalVariant,
    chrome: &ChromeComponent,
) -> Div {
    let skin = SocialSkin::for_variant(variant);
    div()
        .relative()
        .size_full()
        .flex()
        .items_center()
        .gap(px(10.0))
        .px(px(SOCIAL_ROW_INSET))
        .child(social_portrait(friend, variant, chrome))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .font_family(skin.body_font)
                .text_size(px(12.5))
                .text_color(rgb(skin.text))
                .child(friend.name.clone()),
        )
        .child(social_presence_dot(friend.presence, skin))
}

/// the section rules carry no padding, so the gap between one section and the
/// next lives here.
fn section_gap() -> AnyElement {
    div().h(px(6.0)).flex_shrink_0().into_any_element()
}
