use crate::app::client::{InvitationKind, UiInvitation};

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
