use std::collections::BTreeMap;

use crate::{
    app::client::{InvitationKind, UiInvitation, join::target_for_query},
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
