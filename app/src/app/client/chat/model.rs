use super::*;

#[derive(Clone)]
pub(in crate::app::client) enum ChatLine {
    Notice {
        time: String,
        text: String,
    },
    Membership {
        time: String,
        text: String,
    },
    Message {
        time: String,
        sender: UiUser,
        text: String,
    },
    Error {
        time: String,
        text: String,
    },
}

pub(in crate::app::client) struct ChatEntryReveal {
    pub(in crate::app::client) tab_id: u64,
    pub(in crate::app::client) line_index: usize,
    pub(in crate::app::client) started: Instant,
}

impl From<&FixtureLine> for ChatLine {
    fn from(line: &FixtureLine) -> Self {
        match line {
            FixtureLine::Notice { time, text } => Self::Notice {
                time: (*time).to_owned(),
                text: (*text).to_owned(),
            },
            FixtureLine::Membership { time, text } => Self::Membership {
                time: (*time).to_owned(),
                text: (*text).to_owned(),
            },
            FixtureLine::Message { time, user, text } => Self::Message {
                time: (*time).to_owned(),
                sender: UiUser::fixture(*user),
                text: (*text).to_owned(),
            },
        }
    }
}

pub(in crate::app::client) fn shared_transcript_line(line: &ChatLine) -> TranscriptLine {
    match line {
        ChatLine::Notice { time, text } => TranscriptLine::Notice {
            time: time.clone(),
            text: text.clone(),
        },
        ChatLine::Membership { time, text } => TranscriptLine::Membership {
            time: time.clone(),
            text: text.clone(),
        },
        ChatLine::Message { time, sender, text } => TranscriptLine::Message {
            time: time.clone(),
            sender: sender.name.clone(),
            text: text.clone(),
        },
        ChatLine::Error { time, text } => TranscriptLine::Error {
            time: time.clone(),
            text: text.clone(),
        },
    }
}
