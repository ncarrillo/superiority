//! Shared StarCraft II presentation for desktop and Live.

pub mod assets;
pub mod components;
pub mod model;
pub mod theme;

pub use assets::Sc2Assets;
pub use model::{
    DigestEvent, MembershipEvent, MembershipKind, Portrait, PresenceKind, RosterChannelKind,
    RosterPresentation, RosterRelationship, RosterSegment, RosterUser, RosterUserTone,
    TranscriptLine,
};
