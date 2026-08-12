use super::*;
use crate::app::client::{roster::*, ui_roster};

#[test]
fn roster_filter_matches_display_names_case_insensitively() {
    let users = [user(10, "<SC2> Commander"), user(20, "Nova")];
    assert_eq!(filtered_roster_users(&users, "comm")[0].handle, 10);
    assert_eq!(filtered_roster_users(&users, "NOVA")[0].handle, 20);
    assert!(filtered_roster_users(&users, "missing").is_empty());
}

#[test]
fn roster_range_materializes_only_requested_rows() {
    let users = [
        user(10, "Alpha"),
        user(20, "Bravo"),
        user(30, "Charlie"),
        user(40, "Delta"),
    ];
    assert_eq!(filtered_roster_count(&users, "a"), 4);
    assert_eq!(
        filtered_roster_range(&users, "a", 1..3)
            .iter()
            .map(|user| user.handle)
            .collect::<Vec<_>>(),
        vec![20, 30]
    );
    assert_eq!(
        filtered_roster_range(&users, "", 2..8)
            .iter()
            .map(|user| user.handle)
            .collect::<Vec<_>>(),
        vec![30, 40]
    );
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
