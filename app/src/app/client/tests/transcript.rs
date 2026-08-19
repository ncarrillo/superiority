use super::*;
use crate::app::client::session::adopt_identity;

fn with_portrait(mut user: UiUser) -> UiUser {
    user.portrait = Some(std::sync::Arc::new(gpui::RenderImage::new(vec![
        image::Frame::new(image::RgbaImage::new(1, 1)),
    ])));
    user
}

#[test]
fn a_member_adopts_the_avatar_the_join_event_could_not_carry() {
    let mut member = user(7, "NelsonTest91");
    assert!(member.portrait.is_none());

    let roster = vec![with_portrait(user(7, "NelsonTest91"))];
    adopt_identity(&mut member, &roster);

    assert!(member.portrait.is_some());
}

#[test]
fn a_member_adopts_a_clan_tag_that_resolves_after_the_join() {
    let mut member = user(7, "werlap");
    let mut tagged = user(7, "<MDGTN> werlap");
    tagged.clan_tag = Some("MDGTN".to_owned());

    adopt_identity(&mut member, &[tagged]);

    assert_eq!(member.clan_tag.as_deref(), Some("MDGTN"));
    assert_eq!(member.name, "<MDGTN> werlap");
}

#[test]
fn a_resolved_avatar_survives_the_member_leaving_the_roster() {
    let mut member = with_portrait(user(7, "NelsonTest91"));

    // they have churned out of a busy channel; nothing to adopt from.
    let settled = adopt_identity(&mut member, &[]);

    assert!(
        member.portrait.is_some(),
        "must not decay to the placeholder"
    );
    assert!(
        settled,
        "a departed member is never coming back to be filled in"
    );
}

#[test]
fn an_unresolvable_member_stops_being_asked_about() {
    let mut member = user(7, "Ghost");

    assert!(adopt_identity(&mut member, &[]));
}
