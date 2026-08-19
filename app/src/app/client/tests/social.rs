use super::super::{PresenceState, UiFriend, WhisperTarget, friend_order, online_summary};

#[test]
fn friends_sort_online_first_then_alphabetical() {
    let friends = [
        friend("Zeratul", PresenceState::Available),
        friend("carlos", PresenceState::Offline),
        friend("Artanis", PresenceState::InGame),
        friend("Echoes", PresenceState::Unknown),
    ];
    let order = friend_order(&friends, &[]);
    let names = order
        .iter()
        .map(|friend| friend.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["Artanis", "Zeratul", "carlos", "Echoes"]);
}

#[test]
fn friends_listed_under_whispers_are_not_listed_again() {
    let friends = [
        friend("Zeratul", PresenceState::Available),
        friend("Nova", PresenceState::Available),
    ];
    let order = friend_order(&friends, &["Nova".to_owned()]);
    assert_eq!(order.len(), 1);
    assert_eq!(order[0].name, "Zeratul");
}

#[test]
fn friends_header_counts_who_is_around() {
    let friends = [
        friend("Zeratul", PresenceState::Available),
        friend("carlos", PresenceState::Offline),
        friend("Echoes", PresenceState::Offline),
    ];
    let order = friend_order(&friends, &[]);
    assert_eq!(online_summary(&order), "1 of 3 online");
}

fn friend(name: &str, presence: PresenceState) -> UiFriend {
    UiFriend {
        name: name.to_owned(),
        presence,
        portrait: None,
        target: WhisperTarget::Name(name.to_owned()),
    }
}
