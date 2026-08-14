use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    ops::Range,
};

use gpui::UniformListScrollHandle;
use superiority_ui::{
    PresenceKind, RosterChannelKind, RosterPresentation, RosterRelationship, TranscriptLine,
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
    channels: &[ChannelSummary],
    channel_data: &HashMap<String, ChannelData>,
    channel: &str,
    members: &[RosterMember],
    filter: &str,
    range: Range<usize>,
) -> Vec<RosterMember> {
    presented_roster_members(channels, channel_data, channel, members, filter)
        .into_iter()
        .skip(range.start)
        .take(range.len())
        .collect()
}

pub(super) fn presented_roster_members(
    channels: &[ChannelSummary],
    channel_data: &HashMap<String, ChannelData>,
    channel: &str,
    members: &[RosterMember],
    filter: &str,
) -> Vec<RosterMember> {
    let kind = channels
        .iter()
        .find(|summary| summary.key == channel)
        .map_or(RosterChannelKind::Standard, |summary| {
            match summary.kind.as_str() {
                "club" => RosterChannelKind::Group,
                "party" => RosterChannelKind::Party,
                _ => RosterChannelKind::Standard,
            }
        });
    let local_clan = channels
        .iter()
        .filter_map(|channel| channel_data.get(&channel.key))
        .flat_map(|data| &data.roster)
        .find(|member| member.is_local)
        .and_then(|member| member.clan_tag.as_deref())
        .map(str::to_lowercase);
    let shared_party = membership_keys(channels, channel_data, "party");
    let mut members = members
        .iter()
        .filter(|member| roster_member_matches(member, filter))
        .cloned()
        .map(|mut member| {
            let key = member_identity(&member);
            let is_local = member.is_local;
            let shares_clan = !is_local
                && member.clan_tag.as_ref().is_some_and(|clan_tag| {
                    local_clan
                        .as_ref()
                        .is_some_and(|local_clan| clan_tag.to_lowercase() == *local_clan)
                });
            let in_party = shared_party.contains(&key);
            let presentation = RosterPresentation::resolve(RosterRelationship {
                shared_clan: shares_clan,
                shared_party: in_party,
                away: member.presence == "away",
            });
            member.tone = presentation.tone;
            (member, presentation)
        })
        .collect::<Vec<_>>();
    if kind != RosterChannelKind::Party {
        members.sort_by_key(|(member, presentation)| {
            (presentation.rank, member.joined_order.unwrap_or(u64::MAX))
        });
        let mut previous = None;
        for (member, presentation) in &mut members {
            member.segment_start = previous.is_some_and(|previous| previous != presentation.rank);
            previous = Some(presentation.rank);
        }
    } else {
        for (member, _) in &mut members {
            member.segment_start = false;
        }
    }
    members.into_iter().map(|(member, _)| member).collect()
}

fn membership_keys(
    channels: &[ChannelSummary],
    channel_data: &HashMap<String, ChannelData>,
    kind: &str,
) -> std::collections::BTreeSet<String> {
    channels
        .iter()
        .filter(|channel| channel.kind == kind)
        .filter_map(|channel| channel_data.get(&channel.key))
        .flat_map(|data| data.roster.iter().map(member_identity))
        .collect()
}

fn member_identity(member: &RosterMember) -> String {
    member
        .name
        .as_deref()
        .map_or_else(|| format!("handle:{}", member.handle), str::to_lowercase)
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
