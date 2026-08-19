use super::*;
use crate::app::client::ChannelState;
use crate::app::client::UiFriend;
use crate::app::client::{roster::*, ui_roster};
use crate::chat::ChatChannel;
use crate::native::{PresenceState, WhisperTarget};
use superiority_ui::{RosterSegment, RosterUserTone};

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
fn roster_segments_band_clan_party_friends_and_everyone() {
    let mut general = ChannelState::fixture_joined(1, "General".into());
    general.channel = Some(ChatChannel::Public(1028));
    general.local_member_handle = Some(1);
    general.users = vec![
        roster_user(1, "Local", Some(10), Some("SC2"), PresenceKind::Available),
        roster_user(2, "Clanmate", Some(20), Some("SC2"), PresenceKind::Away),
        roster_user(3, "Party", Some(30), None, PresenceKind::Away),
        roster_user(4, "Online", Some(40), None, PresenceKind::Available),
        roster_user(5, "Away", Some(50), None, PresenceKind::Away),
        roster_user(6, "Buddy", Some(60), None, PresenceKind::Available),
    ];
    let mut party = ChannelState::pending_live(2, ChatChannel::Party);
    party.users = vec![
        roster_user(1, "Local", Some(10), Some("SC2"), PresenceKind::Available),
        roster_user(30, "Party", Some(30), None, PresenceKind::Away),
    ];
    let channels = vec![general.clone(), party];
    let friends = vec![friend("Buddy")];

    let presented = presented_roster_users(&channels, &friends, &general, "");
    assert_eq!(
        presented
            .iter()
            .map(|user| user.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Local", "Clanmate", "Party", "Buddy", "Online", "Away"]
    );
    assert_eq!(
        presented
            .iter()
            .map(|user| user.segment)
            .collect::<Vec<_>>(),
        vec![
            RosterSegment::Clan,
            RosterSegment::Clan,
            RosterSegment::Party,
            RosterSegment::Friends,
            RosterSegment::Everyone,
            RosterSegment::Everyone,
        ]
    );
    // you stand with your own clan even though your own name is never tinted
    assert_eq!(presented[0].tone, RosterUserTone::Party);
    assert_eq!(presented[1].tone, RosterUserTone::Clan);
    // present before away inside a band, then alphabetical
    assert_eq!(presented[4].name, "Online");
    assert_eq!(presented[5].name, "Away");
    // your own tag and a clanmate's both read as your clan; a stranger's does not
    assert!(presented[0].own_clan);
    assert!(presented[1].own_clan);
    assert!(!presented[4].own_clan);
}

#[test]
fn roster_entries_carry_one_counted_header_per_band() {
    let mut general = ChannelState::fixture_joined(1, "General".into());
    general.channel = Some(ChatChannel::Public(1028));
    general.local_member_handle = Some(1);
    general.users = vec![
        roster_user(1, "Local", Some(10), Some("SC2"), PresenceKind::Available),
        roster_user(
            2,
            "Clanmate",
            Some(20),
            Some("SC2"),
            PresenceKind::Available,
        ),
        roster_user(4, "Online", Some(40), None, PresenceKind::Available),
        roster_user(5, "Zulu", Some(50), None, PresenceKind::Available),
    ];
    let entries = presented_roster_entries(&[general.clone()], &[], &general, "");
    let headers = entries
        .iter()
        .filter_map(|entry| match entry {
            RosterEntry::Segment { segment, count } => Some((*segment, *count)),
            RosterEntry::User(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        headers,
        vec![(RosterSegment::Clan, 2), (RosterSegment::Everyone, 2)]
    );
    assert_eq!(entries.len(), 6);
    // a header is never selectable, so it never lands under the keyboard cursor
    assert!(matches!(entries[0], RosterEntry::Segment { .. }));
    assert_eq!(
        entries[1].user().map(|user| user.name.as_str()),
        Some("Clanmate")
    );
}

#[test]
fn one_band_needs_no_header() {
    let mut general = ChannelState::fixture_joined(1, "General".into());
    general.channel = Some(ChatChannel::Public(1028));
    general.users = vec![
        roster_user(4, "Online", Some(40), None, PresenceKind::Available),
        roster_user(5, "Zulu", Some(50), None, PresenceKind::Available),
    ];
    let entries = presented_roster_entries(&[general.clone()], &[], &general, "");
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|entry| entry.user().is_some()));
}

#[test]
fn party_roster_keeps_wire_order_without_segments() {
    let mut party = ChannelState::pending_live(1, ChatChannel::Party);
    party.users = vec![
        roster_user(2, "Zulu", Some(20), None, PresenceKind::Away),
        roster_user(1, "Alpha", Some(10), Some("SC2"), PresenceKind::Available),
    ];
    let presented = presented_roster_users(&[party.clone()], &[], &party, "");
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
    let entries = presented_roster_entries(&[party.clone()], &[], &party, "");
    assert!(entries.iter().all(|entry| entry.user().is_some()));
}

fn friend(name: &str) -> UiFriend {
    UiFriend {
        name: name.to_owned(),
        presence: PresenceState::Available,
        portrait: None,
        target: WhisperTarget::Name(name.to_owned()),
    }
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
        segment: RosterSegment::Everyone,
        own_clan: false,
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

#[test]
fn a_member_is_addressed_by_their_name_not_by_their_clan() {
    // `name` carries the tag because that is how the transcript says it, but a
    // whisper, a mention, and the tag chip beside a row all want the person
    let tagged = roster_user(
        1,
        "<BNU> Tagban",
        None,
        Some("BNU"),
        PresenceKind::Available,
    );
    assert_eq!(tagged.bare_name(), "Tagban");
    let plain = roster_user(2, "Nova", None, None, PresenceKind::Available);
    assert_eq!(plain.bare_name(), "Nova");
    // a tag that is not actually the prefix leaves the name alone
    let mismatched = roster_user(3, "Zeratul", None, Some("BNU"), PresenceKind::Available);
    assert_eq!(mismatched.bare_name(), "Zeratul");
}
