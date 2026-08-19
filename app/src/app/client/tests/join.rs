use std::collections::BTreeMap;

use crate::{
    app::client::{
        InvitationKind, UiInvitation,
        join::{JoinRow, JoinSource, count_color, offers_channel, ranked, target_for_query},
    },
    chat::ChatChannel,
};

#[test]
fn invitation_labels_match_product_fallbacks() {
    let group = UiInvitation {
        id: 1,
        kind: InvitationKind::Group { club_id: 535_220 },
        inviter: None,
        destination: None,
        closing: false,
    };
    assert_eq!(group.inviter_label(), "A player");
    assert_eq!(group.destination_label(), "Group 535220");
}

#[test]
fn exact_catalog_name_resolves_to_the_public_channel() {
    let channels = BTreeMap::from([(1028, "General".to_owned()), (1030, "Arcade".to_owned())]);

    assert_eq!(
        target_for_query("General", &channels),
        ChatChannel::Public(1028)
    );
    assert_eq!(
        target_for_query("general", &channels),
        ChatChannel::Public(1028)
    );
    assert_eq!(
        target_for_query("private room", &channels),
        ChatChannel::Private("private room".to_owned())
    );
}

#[test]
fn the_list_is_one_ranked_set_with_groups_first() {
    // there are no section headers, so the order has to carry the grouping:
    // groups above channels, then the busiest rooms first
    let rows = ranked(
        &[
            channel("Looking for Cooperative Team", Some(3)),
            channel("Protoss Strategy", Some(231)),
            group("cecw", Some(14)),
            channel("Practice League", Some(64)),
            channel("Nobody Knows", None),
        ],
        "",
    );

    assert_eq!(
        rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
        vec![
            "cecw",
            "Protoss Strategy",
            "Practice League",
            "Looking for Cooperative Team",
            "Nobody Knows",
        ]
    );
}

#[test]
fn your_own_groups_stay_in_the_list_while_you_type() {
    // typing the first letters of a clan you are in is the fastest way back
    // into it, so your own groups are filtered like everything else — never
    // dropped for being yours — and they sit above the ones you merely found
    let rows = ranked(
        &[
            channel("Midnight Ladder", Some(120)),
            group("Mid City Gamblers", Some(300)),
            member("Midigation", Some(4)),
            member("Blood Nation", Some(90)),
        ],
        "mid",
    );

    assert_eq!(
        rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
        vec!["Midigation", "Mid City Gamblers", "Midnight Ladder"]
    );
}

#[test]
fn an_empty_room_sinks_under_the_ones_with_people_in_them() {
    // a dead channel is still offered — joining is how it stops being empty —
    // but it never reads first, however early its name sorts
    let rows = ranked(
        &[
            channel("Aardvark Alley", Some(0)),
            channel("Zerg Practice", Some(12)),
            channel("Nobody Knows", None),
        ],
        "",
    );

    assert_eq!(
        rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
        vec!["Zerg Practice", "Nobody Knows", "Aardvark Alley"]
    );
    // a room we have no count for is unknown, not dead
    assert!(rows[2].dead());
    assert!(!rows[1].dead());
}

#[test]
fn a_quiet_room_warns_instead_of_inviting() {
    assert_eq!(count_color(231), 0x0047_d184);
    assert_eq!(count_color(10), 0x0047_d184);
    assert_eq!(count_color(3), 0x007d_8fa8);
}

#[test]
fn channels_with_no_live_conference_are_not_offered() {
    // the catalogue names 35 channels, but only a handful exist as rooms; the
    // rest cannot be joined, so the list must not pretend otherwise
    let live = BTreeMap::from([(1028_u16, vec![129_215_u32]), (1033, vec![102_214])]);
    let general = ChatChannel::Public(1028);
    let practice = ChatChannel::Public(1005);

    assert!(offers_channel(true, &live, &general));
    assert!(!offers_channel(true, &live, &practice));

    // before the directory lands nothing is pruned — an empty set would
    // otherwise empty the whole list
    assert!(offers_channel(false, &BTreeMap::new(), &practice));
    // and a private or group target is never a catalogue channel
    assert!(offers_channel(
        true,
        &live,
        &ChatChannel::Private("clan chat".to_owned())
    ));
}

fn channel(name: &str, count: Option<usize>) -> JoinRow {
    JoinRow {
        name: name.to_owned(),
        note: None,
        source: JoinSource::Public,
        target: ChatChannel::Private(name.to_owned()),
        icon: "images/icons/channel.png",
        count,
    }
}

fn member(name: &str, count: Option<usize>) -> JoinRow {
    JoinRow {
        source: JoinSource::Group,
        note: Some("Clan".to_owned()),
        ..channel(name, count)
    }
}

fn group(name: &str, count: Option<usize>) -> JoinRow {
    JoinRow {
        source: JoinSource::Community,
        note: Some("Community".to_owned()),
        ..channel(name, count)
    }
}
