//! shared StarCraft: Remastered presentation for desktop and Live.

use std::time::Duration;

pub mod components;
pub mod model;
pub mod presenter;
pub mod theme;

pub use model::{
    AccountIdentity, RosterPresence, RosterUser, TranscriptLine, TranscriptMessageKind,
    TranscriptNoticeKind, TranscriptSpeaker,
};
pub use presenter::{Console, ConsoleHost};

/// how long a freshly landed transcript line takes to reveal itself.
pub const CHAT_ENTRY_REVEAL: Duration = Duration::from_millis(350);
