use super::*;

#[derive(Clone)]
pub(in crate::app::client) struct ChannelState {
    pub(in crate::app::client) id: u64,
    pub(in crate::app::client) title: String,
    pub(in crate::app::client) channel: Option<ChatChannel>,
    pub(in crate::app::client) channel_index: Option<u8>,
    pub(in crate::app::client) shard_index: Option<u16>,
    pub(in crate::app::client) local_member_handle: Option<u32>,
    pub(in crate::app::client) transcript: Vec<ChatLine>,
    pub(in crate::app::client) users: Vec<UiUser>,
    pub(in crate::app::client) roster_complete: bool,
    pub(in crate::app::client) roster_filter: String,
    pub(in crate::app::client) unread: bool,
}

pub(in crate::app::client) struct ChannelTransitionSnapshot {
    pub(in crate::app::client) outgoing: Option<ChannelState>,
    pub(in crate::app::client) outgoing_selected_user: Option<u32>,
}

pub(in crate::app::client) type ChannelTransition =
    ui_workspace::ChannelTransition<ChannelTransitionSnapshot, Instant>;

impl ChannelState {
    pub(in crate::app::client) fn fixture() -> Self {
        Self {
            id: 0,
            title: preview::CHANNEL.to_owned(),
            channel: None,
            channel_index: None,
            shard_index: None,
            local_member_handle: Some(0),
            transcript: preview::TRANSCRIPT.iter().map(ChatLine::from).collect(),
            users: (0..preview::USERS.len()).map(UiUser::fixture).collect(),
            roster_complete: true,
            roster_filter: String::new(),
            unread: false,
        }
    }

    pub(in crate::app::client) fn fixture_joined(id: u64, title: String) -> Self {
        Self {
            id,
            transcript: vec![ChatLine::Notice {
                time: "7:35 PM".to_owned(),
                text: format!("Welcome to {title}."),
            }],
            title,
            channel: None,
            channel_index: None,
            shard_index: None,
            local_member_handle: Some(0),
            users: (0..preview::USERS.len()).map(UiUser::fixture).collect(),
            roster_complete: true,
            roster_filter: String::new(),
            unread: false,
        }
    }

    pub(in crate::app::client) fn pending_live(id: u64, channel: ChatChannel) -> Self {
        Self {
            id,
            title: channel_title(&channel),
            channel: Some(channel),
            channel_index: None,
            shard_index: None,
            local_member_handle: None,
            transcript: Vec::new(),
            users: Vec::new(),
            roster_complete: false,
            roster_filter: String::new(),
            unread: false,
        }
    }
}

pub(in crate::app::client) type TabDragPayload = ui_navigation::TabDragPayload;

pub(in crate::app::client) struct TabCloseAnimation {
    pub(in crate::app::client) index: usize,
    pub(in crate::app::client) started: Option<Instant>,
}

pub(in crate::app::client) fn retitle_notices(transcript: &mut [ChatLine], from: &str, to: &str) {
    for line in transcript {
        if let ChatLine::Notice { text, .. } = line
            && text.contains(from)
        {
            *text = text.replace(from, to);
        }
    }
}

pub(in crate::app::client) fn join_rejection_notice(channel: &str, reason: Option<u16>) -> String {
    let Some(code) = reason else {
        return format!("Could not join {channel}.");
    };
    match code {
        10001 | 11001 => format!("{channel} no longer exists."),
        10000 | 11000 => format!("{channel} is full."),
        10015 | 11015 | 10008 | 11008 => {
            format!("{channel} is private — it takes an invitation.")
        }
        10014 | 11014 | 305 => {
            format!("You do not have permission to create or join {channel}.")
        }
        301 | 10017 | 11017 => "You are in as many channels as Battle.net allows.".to_owned(),
        10003 | 11003 => format!("You are already in {channel}."),
        _ => match crate::native::errors::error_name(code) {
            Some(name) => format!("Could not join {channel} ({name})."),
            None => format!("Could not join {channel} (Battle.net reason {code})."),
        },
    }
}
