use super::{ChatLine, PresenceKind, UiUser};

mod channel;
mod chrome;
mod composer;
mod join;
mod navigation;
mod roster;
mod social;
mod transcript;

fn user(handle: u32, name: &str) -> UiUser {
    UiUser {
        handle,
        name: name.to_owned(),
        presence_id: None,
        clan_tag: None,
        presence: PresenceKind::Available,
        portrait: None,
        tone: superiority_ui::RosterUserTone::Normal,
        segment: superiority_ui::RosterSegment::Everyone,
        own_clan: false,
    }
}
