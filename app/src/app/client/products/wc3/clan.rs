//! The Clan Hall: the account's Warcraft III clan, as Battle.net reports it.
//!
//! From the `Clan Hall` design — the carved-stone shell, one crest plaque
//! (tag beside the name, description under it), a roster whose ranks are
//! titles where the title is known, and an empty hall that reads as a
//! beginning rather than a void.
//!
//! What the data allows, and no more: the retail Clan service hands us a
//! tag, a name, a motto, a description, and members as `(name, rank)`. It
//! does not say when the clan was founded, who is online, or — yet — give us
//! any way to invite, promote, kick, re-word the motto, or abandon the clan;
//! the design's actions wait on those protocol surfaces. The one rank whose
//! meaning is established is 1: a freshly founded one-member clan reports
//! its founder at rank 1, and the founder of a Warcraft III clan is its
//! Chieftain. Every other rank is shown as the number it is.

use super::*;

const WIDTH: f32 = 640.0;
const HEIGHT: f32 = 520.0;
/// The plaque, roster, and hall sit 18px inside the plate's 14px inset.
const INSET: f32 = 32.0;
const ROW_HEIGHT: f32 = 40.0;
const CREST: f32 = 26.0;

const PLAQUE_TOP: u32 = 0x1710_06ff;
const PLAQUE_BOTTOM: u32 = 0x0f0a_06ff;
const NAME_OTHER: u32 = 0x00e8_dcc0;
const CREST_WELL: u32 = 0x001a_1206;
const CREST_EDGE_QUIET: u32 = 0x006e_5a38;
const HALL_EDGE: u32 = 0x8a6d_3b66;
const HALL_NOTE: u32 = 0x006e_5a38;
const YOU_ROW: u32 = 0xe8c8_740a;
const ROW_HOVER: u32 = 0xe8c8_7412;
const LIST_EDGE: u32 = 0x5e4a_2680;

/// The rank's title, where one is established. Only the founder's rank has
/// been observed against a known meaning; the rest stay numeric rather than
/// borrowing the classic ladder's names for values that may not be theirs.
pub(in crate::app::client) const fn clan_rank_title(rank: u32) -> Option<&'static str> {
    match rank {
        1 => Some("Chieftain"),
        _ => None,
    }
}

/// How a rank is written on a row: its title, or the number it is.
pub(in crate::app::client) fn clan_rank_label(rank: u32) -> String {
    clan_rank_title(rank).map_or_else(|| format!("Rank {rank}"), str::to_owned)
}

/// Whether a clan member is the signed-in account: the member's name, less its
/// character code, against the battle tag's name half.
pub(in crate::app::client) fn is_local_clan_member(
    member_name: &str,
    battle_tag: Option<&str>,
) -> bool {
    battle_tag.is_some_and(|battle_tag| {
        strip_character_code(member_name).eq_ignore_ascii_case(strip_character_code(battle_tag))
    })
}

/// The roster's order: highest rank first (1 leads), then whoever is in the
/// hall's channel, then by name.
pub(in crate::app::client) fn clan_roster_order(
    members: &[superiority_core::games::wc3::ClanMember],
    present: impl Fn(&str) -> bool,
) -> Vec<usize> {
    let mut order: Vec<usize> = (0..members.len()).collect();
    order.sort_by(|&a, &b| {
        let (ma, mb) = (&members[a], &members[b]);
        ma.rank
            .cmp(&mb.rank)
            .then_with(|| present(&mb.name).cmp(&present(&ma.name)))
            .then_with(|| {
                strip_character_code(&ma.name)
                    .to_lowercase()
                    .cmp(&strip_character_code(&mb.name).to_lowercase())
            })
    });
    order
}

impl SuperiorityView {
    /// The account's authoritative Warcraft III clan state, read-only: no
    /// clan-mutating controls exist on the protocol surface yet.
    pub(in crate::app::client) fn wc3_clan_overlay(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wc3 = self
            .session
            .wc3()
            .expect("the Clan Hall is only available from Reforged");
        let snapshot = wc3.clan.clone();
        let scroll = wc3.clan_scroll.clone();
        let battle_tag = self.session.account_battle_tag.clone();
        let in_channel: Vec<String> = wc3
            .active()
            .map(|channel| {
                channel
                    .members
                    .iter()
                    .map(|member| strip_character_code(&member.name).to_lowercase())
                    .collect()
            })
            .unwrap_or_default();

        let body = match snapshot.membership {
            WarcraftClanMembership::Pending => {
                let mut pending = hall_message()
                    .child(
                        div()
                            .text_size(px(15.0))
                            .text_color(rgb(ui_wc3_theme::GOLD))
                            .child("CONSULTING THE CLAN LEDGER"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(ui_wc3_theme::MUTED))
                            .child("Waiting for Battle.net to report this account's clan state."),
                    );
                if let Some(error) = snapshot.read_error {
                    pending = pending.child(
                        div()
                            .max_w(px(480.0))
                            .text_center()
                            .text_size(px(11.0))
                            .text_color(rgb(ui_wc3_theme::EMBER_BRIGHT))
                            .child(format!("Clan state unavailable: {error}")),
                    );
                }
                pending
            }
            WarcraftClanMembership::None => hall_message()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(ui_wc3_theme::GOLD))
                        .child("NO CLAN"),
                )
                .child(
                    div()
                        .max_w(px(430.0))
                        .text_center()
                        .text_size(px(12.0))
                        .text_color(rgb(ui_wc3_theme::MUTED))
                        .child("Battle.net reported no Warcraft III clan for this account."),
                ),
            WarcraftClanMembership::Member(info) => {
                let present =
                    |name: &str| in_channel.contains(&strip_character_code(name).to_lowercase());
                let order = clan_roster_order(&snapshot.members, present);
                let count = snapshot.members.len();
                let alone = count <= 1;
                let rows = order.into_iter().map(|index| {
                    let member = &snapshot.members[index];
                    let you = is_local_clan_member(&member.name, battle_tag.as_deref());
                    clan_row(index, member, you, present(&member.name))
                });
                let roster = div()
                    .id("wc3-clan-roster-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .px(px(10.0))
                    .py(px(8.0))
                    .children(rows);
                let roster_block = div()
                    .relative()
                    .flex_shrink_0()
                    .min_h_0()
                    .border_1()
                    .border_color(rgba(LIST_EDGE))
                    .bg(rgba(0x0f0a_06d9))
                    .child(roster)
                    .vertical_scrollbar_in(
                        &scroll,
                        ui_shared_modal::ModalVariant::Reforged.scrollbar(),
                        window,
                        cx,
                    );
                // one member's hall is the height of its one row; a full hall
                // takes the room and scrolls
                let roster_block = if alone {
                    roster_block.h(px(ROW_HEIGHT + 16.0))
                } else {
                    roster_block.flex_1()
                };

                let mut column = div()
                    .absolute()
                    .left(px(INSET))
                    .right(px(INSET))
                    .top(px(76.0))
                    .bottom(px(30.0))
                    .flex()
                    .flex_col()
                    .font_family(ui_wc3_theme::FONT_INTERFACE)
                    .child(crest_plaque(&info, count));
                if let Some(error) = snapshot.read_error {
                    column = column.child(
                        div()
                            .flex_shrink_0()
                            .mt(px(10.0))
                            .px(px(10.0))
                            .py(px(7.0))
                            .border_1()
                            .border_color(rgb(ui_wc3_theme::BLOOD))
                            .text_size(px(11.0))
                            .text_color(rgb(ui_wc3_theme::EMBER_BRIGHT))
                            .child(format!("Roster unavailable: {error}")),
                    );
                }
                column = column
                    .child(section_rule("ROSTER").mt(px(16.0)))
                    .child(roster_block.mt(px(6.0)));
                if alone {
                    column = column.child(empty_hall().mt(px(10.0)));
                }
                column
            }
        };

        let modal = div()
            .id("wc3-clan-modal")
            .relative()
            .w(px(WIDTH))
            .h(px(HEIGHT))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(ui_shared_modal::frame(
                ui_shared_modal::ModalVariant::Reforged,
                WIDTH,
                HEIGHT,
                &self.chrome.modal_textures,
            ))
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .right(px(18.0))
                    .top(px(32.0))
                    .child(ui_shared_modal::title(
                        ui_shared_modal::ModalVariant::Reforged,
                        "CLAN HALL",
                    )),
            )
            .child(body)
            .child(
                ui_shared_modal::close_glyph(ui_shared_modal::ModalVariant::Reforged)
                    .right(px(26.0))
                    .top(px(32.0))
                    .id("wc3-clan-close")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_overlay(window, cx);
                    })),
            );
        let overlay = div()
            .id("wc3-clan-dismiss")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, window, cx| {
                this.dismiss_overlay(window, cx);
            }))
            .child(self.overlays.dimmer())
            .child(ui_shared_modal::animated(
                ui_shared_modal::ModalVariant::Reforged,
                modal,
                self.overlays.closing,
                platform::reduce_motion(),
                WIDTH,
                HEIGHT,
            ));
        self.overlays.animated(
            overlay,
            "wc3-clan-overlay-open",
            "wc3-clan-overlay-close",
            false,
        )
    }
}

/// The centred message the hall shows while it has no clan to show.
fn hall_message() -> Div {
    div()
        .absolute()
        .left(px(INSET))
        .right(px(INSET))
        .top(px(82.0))
        .bottom(px(30.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .font_family(ui_wc3_theme::FONT_INTERFACE)
}

/// One home for the clan's identity: the name with its tag beside it, the
/// description under, and the member count at the right. No crest tile —
/// text never gets two homes — and no founding date, because the service
/// does not report one.
fn crest_plaque(info: &superiority_core::games::wc3::ClanInfo, count: usize) -> Div {
    let mut identity = div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .flex_1()
        .min_w_0()
        .child(
            div()
                .flex()
                .items_end()
                .gap(px(9.0))
                .min_w_0()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_family(ui_wc3_theme::FONT_TITLE)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(21.0))
                        .text_color(rgb(ui_wc3_theme::PARCHMENT))
                        .child(info.name.clone()),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .pb(px(3.0))
                        .text_size(px(12.0))
                        .text_color(rgb(ui_wc3_theme::GOLD_DIM))
                        .child(format!("[{}]", info.tag)),
                ),
        );
    if !info.description.is_empty() {
        identity = identity.child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(ui_wc3_theme::MUTED))
                .child(info.description.clone()),
        );
    }
    if !info.motd.is_empty() {
        identity = identity.child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(ui_wc3_theme::GOLD_DIM))
                .child(info.motd.clone()),
        );
    }
    let members = if count == 1 {
        "1 member".to_owned()
    } else {
        format!("{count} members")
    };
    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(14.0))
        .px(px(16.0))
        .py(px(14.0))
        .border_1()
        .border_color(rgb(ui_wc3_theme::STONE))
        .bg(linear_gradient(
            180.0,
            gpui::linear_color_stop(rgba(PLAQUE_TOP), 0.0),
            gpui::linear_color_stop(rgba(PLAQUE_BOTTOM), 1.0),
        ))
        .child(identity)
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .items_end()
                .gap(px(3.0))
                .text_size(px(11.0))
                .text_color(rgb(ui_wc3_theme::GOLD_DIM))
                .child(members),
        )
}

/// A small-caps section word with a gold rule running off it.
fn section_rule(word: &'static str) -> Div {
    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .gap(px(2.0))
                .font_weight(FontWeight::BOLD)
                .text_size(px(11.0))
                .text_color(rgb(ui_wc3_theme::GOLD))
                .children(word.chars().map(|letter| div().child(letter.to_string()))),
        )
        .child(div().flex_1().h(px(1.0)).bg(linear_gradient(
            90.0,
            gpui::linear_color_stop(rgba(0x8a6d_3bcc), 0.0),
            gpui::linear_color_stop(rgba(0x8a6d_3b00), 1.0),
        )))
}

/// One member: crest slot, name, rank, and a moss mark for anyone standing
/// in the hall's channel right now. The service does not report presence,
/// so nobody is shown as offline.
fn clan_row(
    index: usize,
    member: &superiority_core::games::wc3::ClanMember,
    you: bool,
    present: bool,
) -> Stateful<Div> {
    let lit = you || clan_rank_title(member.rank).is_some();
    let crest = div()
        .size(px(CREST))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(rgb(if lit {
            ui_wc3_theme::STONE_BRIGHT
        } else {
            CREST_EDGE_QUIET
        }))
        .bg(rgb(CREST_WELL))
        .text_size(px(12.0))
        .text_color(rgb(if lit {
            ui_wc3_theme::GOLD
        } else {
            ui_wc3_theme::GOLD_DIM
        }))
        // the crest slot: the game's rank icons can drop in here; until then
        // it carries the rank's number
        .child(member.rank.to_string());
    let mut name = div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_baseline()
        .gap(px(6.0))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(14.0))
                .when(you, |name| name.font_weight(FontWeight::BOLD))
                .text_color(rgb(if you {
                    ui_wc3_theme::PARCHMENT
                } else {
                    NAME_OTHER
                }))
                .child(strip_character_code(&member.name).to_owned()),
        );
    if you {
        name = name.child(
            div()
                .flex_shrink_0()
                .text_size(px(11.0))
                .text_color(rgb(ui_wc3_theme::MUTED))
                .child("— you"),
        );
    }
    let rank = div()
        .flex_shrink_0()
        .text_size(px(11.0))
        .text_color(rgb(if clan_rank_title(member.rank).is_some() {
            ui_wc3_theme::GOLD
        } else {
            ui_wc3_theme::GOLD_DIM
        }))
        .child(clan_rank_label(member.rank));
    let mut row = div()
        .id(("wc3-clan-member", index))
        .h(px(ROW_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(11.0))
        .px(px(10.0))
        .when(you, |row| row.bg(rgba(YOU_ROW)))
        .hover(|row| row.bg(rgba(ROW_HOVER)))
        .child(crest)
        .child(name)
        .child(rank);
    if present {
        row = row.child(
            div()
                .size(px(8.0))
                .flex_shrink_0()
                .rounded(px(4.0))
                .bg(rgb(ui_wc3_theme::MOSS))
                .shadow(vec![
                    gpui::BoxShadow::new(px(0.0), px(0.0), rgba(0x7cc4_6ae6).into())
                        .blur_radius(px(6.0)),
                ]),
        );
    }
    row
}

/// The hall with nobody else in it: a beginning, not a void. The design puts
/// an Invite here; it returns when the protocol has one.
fn empty_hall() -> Div {
    div()
        .flex_1()
        .min_h(px(80.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .border_1()
        .border_color(rgba(HALL_EDGE))
        .child(
            div()
                .italic()
                .text_size(px(13.0))
                .text_color(rgb(ui_wc3_theme::MUTED))
                .child("The hall stands empty."),
        )
        .child(
            div()
                .max_w(px(340.0))
                .text_center()
                .text_size(px(11.0))
                .text_color(rgb(HALL_NOTE))
                .child("No one else is sworn to this clan."),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use superiority_core::games::wc3::ClanMember;

    fn member(name: &str, rank: u32) -> ClanMember {
        ClanMember {
            account_id: 0,
            name: name.to_owned(),
            rank,
        }
    }

    #[test]
    fn only_the_founders_rank_has_a_title() {
        assert_eq!(clan_rank_title(1), Some("Chieftain"));
        assert_eq!(clan_rank_label(1), "Chieftain");
        assert_eq!(clan_rank_title(2), None);
        assert_eq!(clan_rank_label(4), "Rank 4");
    }

    #[test]
    fn the_signed_in_account_is_found_by_its_name_half() {
        assert!(is_local_clan_member(
            "NelsonTest91#1234",
            Some("NelsonTest91#1234")
        ));
        assert!(is_local_clan_member("nelsontest91", Some("NelsonTest91#9")));
        assert!(!is_local_clan_member(
            "Hozdar#22",
            Some("NelsonTest91#1234")
        ));
        assert!(!is_local_clan_member("Hozdar#22", None));
    }

    #[test]
    fn the_roster_leads_with_the_chieftain_then_whoever_is_in_the_hall() {
        let members = vec![
            member("Fumbles#1", 3),
            member("Hozdar#2", 2),
            member("Chief#3", 1),
            member("Absent#4", 2),
        ];
        let order = clan_roster_order(&members, |name| name.starts_with("Hozdar"));
        let names: Vec<&str> = order
            .into_iter()
            .map(|index| strip_character_code(&members[index].name))
            .collect();
        assert_eq!(names, vec!["Chief", "Hozdar", "Absent", "Fumbles"]);
    }
}
