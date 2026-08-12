use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    ops::Range,
};

use gpui::UniformListScrollHandle;
use superiority_ui::{
    PresenceKind, TranscriptLine,
    components::{roster, workspace},
};
use wasm_bindgen::JsValue;

use crate::api::{ChannelSummary, RosterMember, Sender, StreamItem};

#[derive(Clone, Default)]
pub(super) struct ChannelData {
    pub(super) items: Vec<StreamItem>,
    pub(super) roster: Vec<RosterMember>,
    pub(super) message_cursor: Option<u64>,
    pub(super) stream_loaded: bool,
    pub(super) loading: bool,
    pub(super) refreshed_at: Option<f64>,
}

pub(super) struct ChannelTransitionSnapshot {
    pub(super) outgoing: String,
    pub(super) outgoing_data: ChannelData,
    pub(super) outgoing_roster_scroll: UniformListScrollHandle,
    pub(super) incoming: String,
}

pub(super) type ChannelTransition = workspace::ChannelTransition<ChannelTransitionSnapshot, f64>;

pub(super) fn reconcile_channel_order(
    current: &mut Vec<ChannelSummary>,
    incoming: Vec<ChannelSummary>,
) {
    let mut by_key = incoming
        .into_iter()
        .map(|channel| (channel.key.clone(), channel))
        .collect::<HashMap<_, _>>();
    let mut merged = Vec::with_capacity(by_key.len());
    for channel in current.iter() {
        if let Some(updated) = by_key.remove(&channel.key) {
            merged.push(updated);
        }
    }
    let mut added = by_key.into_values().collect::<Vec<_>>();
    added.sort_by(|left, right| {
        left.first_seen_at
            .cmp(&right.first_seen_at)
            .then_with(|| left.key.cmp(&right.key))
    });
    merged.extend(added);
    *current = merged;
}

pub(super) fn filtered_roster_range(
    members: &[RosterMember],
    filter: &str,
    range: Range<usize>,
) -> Vec<RosterMember> {
    roster::filtered_range(members, filter, range, |member, filter| {
        roster_member_matches(member, filter)
    })
}

pub(super) fn roster_member_matches(member: &RosterMember, filter: &str) -> bool {
    let name = Sender {
        handle: member.handle,
        name: member.name.clone(),
        clan_tag: member.clan_tag.clone(),
    }
    .display_name();
    roster::filter_matches(&name, filter)
}

pub(super) fn transcript_scope(channel: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    channel.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn stream_line(item: &StreamItem) -> TranscriptLine {
    match item {
        StreamItem::Message(message) => TranscriptLine::Message {
            time: stamp_label(message.ts),
            sender: message.sender.display_name(),
            text: message.body.clone(),
        },
    }
}

fn stamp_label(timestamp: u64) -> String {
    let date = js_sys::Date::new(&JsValue::from_f64(timestamp as f64));
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    let suffix = if hours < 12 { "AM" } else { "PM" };
    let hour = match hours % 12 {
        0 => 12,
        hour => hour,
    };
    format!("{hour}:{minutes:02} {suffix}")
}

pub(super) fn presence(value: &str) -> PresenceKind {
    match value {
        "available" | "online" => PresenceKind::Available,
        "away" => PresenceKind::Away,
        "busy" => PresenceKind::Busy,
        "in_game" => PresenceKind::InGame,
        _ => PresenceKind::Offline,
    }
}

pub(super) fn compact_error(error: &str) -> String {
    let compact = error.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 56 {
        compact
    } else {
        format!("{}…", compact.chars().take(55).collect::<String>())
    }
}
