use super::*;
use crate::app::client::ChannelState;
use crate::app::client::{roster::*, ui_roster};
use crate::chat::ChatChannel;
use superiority_ui::RosterUserTone;

#[test]
fn roster_filter_matches_display_names_case_insensitively() {
    let users = [user(10, "<SC2> Commander"), user(20, "Nova")];
    assert_eq!(filtered_roster_count(&users, "comm"), 1);
    assert_eq!(filtered_roster_count(&users, "NOVA"), 1);
    assert_eq!(filtered_roster_count(&users, "missing"), 0);
}

#[test]
fn roster_filters_are_scoped_to_channel_tabs() {
    let mut general = ChannelState::fixture_joined(1, "General".into());
    let mut arcade = ChannelState::fixture_joined(2, "Arcade".into());

    general.roster_filter = "nova".into();
    arcade.roster_filter = "commander".into();

    assert_eq!(general.roster_filter, "nova");
    assert_eq!(arcade.roster_filter, "commander");
}

#[test]
fn roster_segments_follow_clan_party_normal_away_precedence() {
    let mut general = ChannelState::fixture_joined(1, "General".into());
    general.channel = Some(ChatChannel::Public(1028));
    general.local_member_handle = Some(1);
    general.users = vec![
        roster_user(1, "Local", Some(10), Some("SC2"), PresenceKind::Available),
        roster_user(2, "Clanmate", Some(20), Some("SC2"), PresenceKind::Away),
        roster_user(3, "Party", Some(30), None, PresenceKind::Away),
        roster_user(4, "Online", Some(40), None, PresenceKind::Available),
        roster_user(5, "Away", Some(50), None, PresenceKind::Away),
    ];
    let mut party = ChannelState::pending_live(2, ChatChannel::Party);
    party.users = vec![
        roster_user(1, "Local", Some(10), Some("SC2"), PresenceKind::Available),
        roster_user(30, "Party", Some(30), None, PresenceKind::Away),
    ];
    let channels = vec![general.clone(), party];

    let presented = presented_roster_users(&channels, &general, "");
    assert_eq!(
        presented
            .iter()
            .map(|user| user.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Clanmate", "Local", "Party", "Online", "Away"]
    );
    assert_eq!(presented[0].tone, RosterUserTone::Clan);
    assert_eq!(presented[1].tone, RosterUserTone::Party);
    assert_eq!(presented[2].tone, RosterUserTone::Party);
    assert!(presented[1].segment_start);
    assert!(!presented[2].segment_start);
    assert!(presented[3].segment_start);
    assert!(presented[4].segment_start);
}

#[test]
fn party_roster_keeps_wire_order_without_segments() {
    let mut party = ChannelState::pending_live(1, ChatChannel::Party);
    party.users = vec![
        roster_user(2, "Zulu", Some(20), None, PresenceKind::Away),
        roster_user(1, "Alpha", Some(10), Some("SC2"), PresenceKind::Available),
    ];
    let presented = presented_roster_users(&[party.clone()], &party, "");
    assert_eq!(
        presented
            .iter()
            .map(|user| user.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Zulu", "Alpha"]
    );
    assert!(
        presented
            .iter()
            .all(|user| user.tone == RosterUserTone::Party)
    );
    assert!(presented.iter().all(|user| !user.segment_start));
}

fn roster_user(
    handle: u32,
    name: &str,
    presence_id: Option<u32>,
    clan_tag: Option<&str>,
    presence: PresenceKind,
) -> UiUser {
    UiUser {
        handle,
        name: name.into(),
        presence_id,
        clan_tag: clan_tag.map(ToOwned::to_owned),
        presence,
        portrait: None,
        tone: RosterUserTone::Normal,
        segment_start: false,
    }
}

#[test]
fn roster_diff_tracks_stable_insertions_and_removals() {
    let previous = vec![user(10, "Ten"), user(20, "Twenty"), user(40, "Forty")];
    let next = vec![user(10, "Ten"), user(30, "Thirty"), user(40, "Forty")];
    let transition = ui_roster::Transition::new(previous, &next, |user| user.handle)
        .expect("stable relative order");
    assert!(!transition.is_full_reveal());

    let reordered = ui_roster::Transition::new(
        vec![user(10, "Ten"), user(20, "Twenty")],
        &[user(20, "Twenty"), user(10, "Ten")],
        |user| user.handle,
    )
    .expect("reordering uses a full reveal");
    assert!(reordered.is_full_reveal());
}

#[test]
fn animated_roster_merge_keeps_removed_and_inserted_rows_in_place() {
    let next = vec![user(10, "Ten"), user(30, "Thirty"), user(40, "Forty")];
    let animation = ui_roster::Transition::new(
        vec![user(10, "Ten"), user(20, "Twenty"), user(40, "Forty")],
        &next,
        |user| user.handle,
    )
    .expect("roster changed");
    let rows = animation.rows(&next, |user| user.handle);
    assert_eq!(
        rows.iter().map(|(user, _)| user.handle).collect::<Vec<_>>(),
        vec![10, 20, 30, 40]
    );
    assert_eq!(rows[1].1, ui_roster::RowMotion::Removed);
    assert_eq!(rows[2].1, ui_roster::RowMotion::Inserted);
}
