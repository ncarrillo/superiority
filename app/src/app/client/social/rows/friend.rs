use super::*;

/// a friend at member-list density (refinement I via N): a 28px portrait, a
/// name, and a status dot — no presence label, because the dot says it, and no
/// "Offline", because the dimming does.
pub(super) fn friend_row(
    row_id: usize,
    friend: &UiFriend,
    chrome: &ChromeComponent,
    cx: &mut Context<SuperiorityView>,
) -> AnyElement {
    let peer = friend.name.clone();
    let dimmed = !friend.is_online();
    let user = friend.roster_user(&chrome.ui_assets);
    let mut row = div()
        .id(("social-friend", row_id))
        .relative()
        .h(px(ROSTER_ROW_HEIGHT))
        .flex_shrink_0()
        .cursor_pointer()
        .hover(move |style| {
            let style = style.bg(rgba(0x1231_5e59));
            // offline rows lift back toward legible on hover so they still feel
            // clickable.
            if dimmed { style.opacity(0.7) } else { style }
        })
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.social.open_conversation(peer.clone(), window, cx);
            cx.notify();
        }))
        .child(ui_roster::person_row(&user, &chrome.ui_assets));
    if dimmed {
        // one dim value for the whole row rather than a value per portrait,
        // name, and dot.
        row = row.opacity(SOCIAL_DIMMED_OPACITY);
    }
    row.into_any_element()
}
