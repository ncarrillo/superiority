use super::{ChatLine, PresenceKind, UiUser};

mod channel;
mod chrome;
mod join;
mod navigation;
mod roster;

fn user(handle: u32, name: &str) -> UiUser {
    UiUser {
        handle,
        name: name.to_owned(),
        presence_id: None,
        clan_tag: None,
        presence: PresenceKind::Available,
        portrait: None,
        tone: superiority_ui::RosterUserTone::Normal,
        segment_start: false,
    }
}
