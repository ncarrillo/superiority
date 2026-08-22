use super::*;

/// a party seats four.
const PARTY_SEATS: usize = 4;
pub(in crate::app::client) const PARTY_DOCK_HEIGHT: f32 = 38.0;
/// the dock sits clear of the field rather than on top of it — they are two
/// surfaces, one for state and one for what you are about to say.
pub(in crate::app::client) const PARTY_DOCK_GAP: f32 = 8.0;
/// the dock is a strip, not a banner. it matches the popup above the same field
/// so the two surfaces agree, and its contents stay inside one eye-span instead
/// of being flung to the corners of an ultrawide window.
pub(in crate::app::client) const PARTY_DOCK_MAX_WIDTH: f32 = 480.0;
const SEAT_FRAME: f32 = 26.0;
const SEAT_FACE: f32 = 22.0;
const SEAT_GAP: f32 = 6.0;
const DOT_SIZE: f32 = 8.0;

const DOCK_FILL: u32 = 0x030c_08eb;
const DOCK_BORDER: u32 = 0x47d1_8473;
const SEAT_BORDER: u32 = 0x47d1_8480;
const SEAT_HOVER: u32 = 0x47d1_841f;
const PARTY_GREEN: u32 = 0x0047_d184;
const ACTION_TEXT: u32 = 0x000a_1f12;
const LEADER: u32 = 0x00f0_aa64;
const COUNT: u32 = 0x00a9_b8cc;
/// an absent member dims whole rather than in parts, the way an offline roster
/// row does.
const ABSENT_OPACITY: f32 = 0.6;

/// how many seats are taken, and whether there is room for one more. the dock
/// offers a single invite slot however many seats are free: it is an action,
/// not a diagram of the empty chairs.
fn seats(members: usize) -> (usize, bool) {
    let taken = members.min(PARTY_SEATS);
    (taken, taken < PARTY_SEATS)
}

/// the party strip that rides above the input in every channel. a party has no
/// window of its own — this is where its state lives, and it travels with you
/// from channel to channel because the party does.
///
/// everything reads left to right in one run — label, faces, invite, count —
/// with the one action alone at the right edge. a full-width spread would put
/// dead space through the middle of a strip that is mostly empty already.
pub(in crate::app::client) fn party_dock(
    members: &[UiUser],
    local: Option<u32>,
    assets: &Sc2Assets,
    cx: &mut Context<SuperiorityView>,
) -> Div {
    let (taken, room) = seats(members.len());
    let leader = members.first().map(|member| member.handle);
    let mut run = div().flex().items_center().gap(px(SEAT_GAP));
    for member in members.iter().take(PARTY_SEATS) {
        run = run.child(seat(member, leader == Some(member.handle), assets));
    }
    if room {
        run = run.child(invite_seat(cx));
    }
    run = run.child(
        div()
            .flex_shrink_0()
            .ml(px(4.0))
            .font_family(FONT_INTERFACE)
            .text_size(px(10.0))
            .text_color(rgb(COUNT))
            .child(format!("{taken} / {PARTY_SEATS}")),
    );

    div()
        .h(px(PARTY_DOCK_HEIGHT))
        .max_w(px(PARTY_DOCK_MAX_WIDTH))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(10.0))
        .px(px(10.0))
        .bg(rgba(DOCK_FILL))
        .border_1()
        .border_color(rgba(DOCK_BORDER))
        .rounded(px(2.0))
        .child(
            div()
                .flex_shrink_0()
                .font_family(FONT_NAVIGATION)
                .font_weight(FontWeight::BOLD)
                .text_size(px(9.5))
                .text_color(rgb(PARTY_GREEN))
                .child("PARTY"),
        )
        .child(run)
        .child(div().flex_1())
        .child(action(leader == local && leader.is_some(), cx))
}

/// one member: the same framed face the roster and the popup show, dimmed when
/// they are not around, with a dot for the state the dimming does not say, and
/// the leader's diamond above it.
fn seat(member: &UiUser, leader: bool, assets: &Sc2Assets) -> Div {
    let portrait = member
        .portrait
        .clone()
        .map(|portrait| Portrait::Image(portrait.into()));
    let mut tile =
        div()
            .relative()
            .size(px(SEAT_FRAME))
            .flex_shrink_0()
            .child(ui_roster::framed_portrait(
                portrait.as_ref(),
                assets,
                SEAT_FRAME,
                SEAT_FACE,
            ));
    if member.presence.absent() {
        tile = tile.opacity(ABSENT_OPACITY);
    }
    // available says nothing a face does not already say; every other state
    // earns the dot
    if member.presence != PresenceKind::Available {
        tile = tile.child(
            div()
                .absolute()
                .right(px(-2.0))
                .bottom(px(-2.0))
                .size(px(DOT_SIZE))
                .rounded_full()
                .bg(rgb(member.presence.dot_color()))
                .border_1()
                .border_color(rgba(DOCK_FILL)),
        );
    }
    if leader {
        tile = tile.child(
            div()
                .absolute()
                .left(px(-3.0))
                .top(px(-6.0))
                .font_family(FONT_INTERFACE)
                .text_size(px(9.0))
                .text_color(rgb(LEADER))
                .child("\u{2666}"),
        );
    }
    tile
}

/// the seat that is not a seat: one dashed slot that invites, however many
/// chairs are actually free.
fn invite_seat(cx: &mut Context<SuperiorityView>) -> Stateful<Div> {
    div()
        .id("party-invite")
        .size(px(SEAT_FRAME))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_dashed()
        .border_color(rgba(SEAT_BORDER))
        .font_family(FONT_INTERFACE)
        .text_size(px(13.0))
        .text_color(rgb(PARTY_GREEN))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(SEAT_HOVER)))
        .child("+")
        .on_click(cx.listener(|this, _, _, cx| {
            this.invite_to_party(cx);
        }))
}

/// the one action, alone at the right edge. the leader starts the queue;
/// everybody else answers it.
fn action(leader: bool, cx: &mut Context<SuperiorityView>) -> Stateful<Div> {
    div()
        .id("party-action")
        .flex_shrink_0()
        .px(px(8.0))
        .py(px(3.0))
        .bg(rgb(PARTY_GREEN))
        .rounded(px(2.0))
        .font_family(FONT_NAVIGATION)
        .font_weight(FontWeight::BOLD)
        .text_size(px(9.5))
        .text_color(rgb(ACTION_TEXT))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0x006b_e8a4)))
        .child(if leader { "QUEUE" } else { "READY" })
        .on_click(cx.listener(|this, _, _, cx| {
            this.begin_party_queue(cx);
        }))
}

#[cfg(test)]
mod tests {
    use super::{PARTY_SEATS, seats};

    #[test]
    fn the_dock_offers_one_invite_however_many_chairs_are_free() {
        // the dashed slot is an action, not a diagram of the empty seats
        assert_eq!(seats(0), (0, true));
        assert_eq!(seats(1), (1, true));
        assert_eq!(seats(3), (3, true));
        assert_eq!(seats(PARTY_SEATS), (PARTY_SEATS, false));
        // and the strip is a fixed four, whatever the roster claims
        assert_eq!(seats(9), (PARTY_SEATS, false));
    }
}
