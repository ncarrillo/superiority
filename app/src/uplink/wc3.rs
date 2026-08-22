//! Projecting a Reforged hall into Live wire events.
//!
//! Like the Remastered projector, the hall's membership is described whole and
//! re-sent, so it diffs snapshots into a [`Roster`] or [`RosterDelta`]. Chat is
//! senderless — the recovered callback does not name a speaker yet — and the
//! join/leave and server lines become [`Notice`]s, matching what the desktop
//! hall renders.
//!
//! [`Roster`]: super::model::EventKind::Roster
//! [`RosterDelta`]: super::model::EventKind::RosterDelta
//! [`Notice`]: super::model::EventKind::Notice

use std::collections::BTreeMap;

use superiority_core::games::wc3::{ChatChannel, ChatEvent, ChatMember, ChatPresence};

use crate::chat::strip_character_code;

use super::classic::{fnv1a32, roster_events, truncate};
use super::model::{ChannelRef, EventKind, NoticeSubkind, UserRef};

pub(super) struct Wc3Projector {
    channels: BTreeMap<u8, ProjectedChannel>,
}

struct ProjectedChannel {
    key: String,
    roster: BTreeMap<u32, UserRef>,
    roster_sent: bool,
}

impl Wc3Projector {
    pub(super) fn new() -> Self {
        Self {
            channels: BTreeMap::new(),
        }
    }

    pub(super) fn channel_events(&mut self, channel: &ChatChannel) -> Vec<EventKind> {
        let key = channel_key(&channel.name);
        let mut events = Vec::new();
        if !self.channels.contains_key(&channel.id) {
            events.push(EventKind::Joined {
                channel: ChannelRef {
                    key: key.clone(),
                    name: Some(channel.name.clone()),
                },
            });
            self.channels.insert(
                channel.id,
                ProjectedChannel {
                    key: key.clone(),
                    roster: BTreeMap::new(),
                    roster_sent: false,
                },
            );
        }
        let projected = self
            .channels
            .get_mut(&channel.id)
            .expect("WC3 projected channel was just inserted");
        projected.key.clone_from(&key);
        let members = channel
            .members
            .iter()
            .map(|member| {
                let user = user_ref(member);
                (user.handle, user)
            })
            .collect::<BTreeMap<_, _>>();
        let channel_ref = ChannelRef {
            key,
            name: Some(channel.name.clone()),
        };
        events.extend(roster_events(
            channel_ref,
            members,
            &mut projected.roster,
            &mut projected.roster_sent,
        ));
        events
    }

    /// A hall event as wire events. Structural (leave) and transcript lines are
    /// both possible, so this returns a list; membership itself rides the
    /// channel snapshot, so joins and leaves are rendered as notices, exactly
    /// as the desktop hall renders them.
    pub(super) fn event_kinds(&mut self, event: &ChatEvent) -> Vec<EventKind> {
        match event {
            ChatEvent::ChannelLeft { channel_id } => {
                let Some(channel) = self.channels.remove(channel_id) else {
                    return Vec::new();
                };
                vec![EventKind::Left {
                    channel: ChannelRef {
                        key: channel.key,
                        name: None,
                    },
                }]
            }
            // the join is carried by the channel snapshot that follows it
            ChatEvent::ChannelJoined { .. } => Vec::new(),
            ChatEvent::Message { channel_id, body } => self
                .channel_ref(*channel_id)
                .map(|channel| EventKind::Message {
                    channel,
                    subkind: None,
                    sender: None,
                    body: truncate(body.clone()),
                })
                .into_iter()
                .collect(),
            // Private conversations belong only to the local Social surface.
            ChatEvent::Whisper { .. } => Vec::new(),
            // Subscription notices are session-wide and have no authoritative
            // room identity, so the multi-channel uplink does not assign one.
            ChatEvent::Notice { .. } => Vec::new(),
            ChatEvent::MemberJoined { channel_id, name } => self.notice(
                *channel_id,
                format!("{} entered the channel.", strip_character_code(name)),
            ),
            ChatEvent::MemberLeft { channel_id, name } => self.notice(
                *channel_id,
                name.as_ref().map_or_else(
                    || "A player left the channel.".to_owned(),
                    |name| format!("{} left the channel.", strip_character_code(name)),
                ),
            ),
        }
    }

    pub(super) fn resend(&self) -> Vec<EventKind> {
        self.channels
            .values()
            .filter(|channel| channel.roster_sent)
            .map(|channel| {
                let users = channel.roster.values().cloned().collect::<Vec<_>>();
                EventKind::Roster {
                    channel: ChannelRef {
                        key: channel.key.clone(),
                        name: None,
                    },
                    complete: true,
                    count: u32::try_from(users.len()).unwrap_or(u32::MAX),
                    users,
                }
            })
            .collect()
    }

    pub(super) fn reset_roster(&mut self) {
        for channel in self.channels.values_mut() {
            channel.roster_sent = false;
        }
    }

    fn channel_ref(&self, channel_id: u8) -> Option<ChannelRef> {
        self.channels.get(&channel_id).map(|channel| ChannelRef {
            key: channel.key.clone(),
            name: None,
        })
    }

    fn notice(&self, channel_id: u8, body: String) -> Vec<EventKind> {
        self.channel_ref(channel_id)
            .map(|channel| EventKind::Notice {
                channel,
                subkind: NoticeSubkind::Information,
                body,
            })
            .into_iter()
            .collect()
    }
}

fn channel_key(name: &str) -> String {
    format!("private:{name}")
}

fn user_ref(member: &ChatMember) -> UserRef {
    UserRef {
        handle: fnv1a32(&member.handle.to_le_bytes()),
        name: Some(strip_character_code(&member.name).to_owned()),
        clan_tag: member.clan_abbreviation.clone(),
        presence: Some(presence(member.presence)),
        portrait: None,
        is_local: None,
        joined_order: None,
        avatar: member.avatar_id.clone(),
        is_operator: None,
    }
}

const fn presence(presence: ChatPresence) -> &'static str {
    match presence {
        ChatPresence::Offline => "offline",
        ChatPresence::Online => "available",
        ChatPresence::Away => "away",
        ChatPresence::Busy => "busy",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(id: u8, name: &str) -> ChatChannel {
        ChatChannel {
            id,
            name: name.into(),
            members: Vec::new(),
        }
    }

    #[test]
    fn simultaneous_halls_remain_joined_and_route_by_channel_id() {
        let mut projector = Wc3Projector::new();
        let general = projector.channel_events(&channel(0, "General"));
        let clan = projector.channel_events(&channel(1, "Clan"));

        assert!(matches!(general.first(), Some(EventKind::Joined { .. })));
        assert!(matches!(clan.first(), Some(EventKind::Joined { .. })));
        assert_eq!(projector.resend().len(), 2);
        assert!(projector.channel_events(&channel(0, "General")).is_empty());

        let message = projector.event_kinds(&ChatEvent::Message {
            channel_id: 1,
            body: "Lok'tar".into(),
        });
        assert!(matches!(
            message.as_slice(),
            [EventKind::Message {
                channel: ChannelRef { key, .. },
                body,
                ..
            }] if key == "private:Clan" && body == "Lok'tar"
        ));

        let left = projector.event_kinds(&ChatEvent::ChannelLeft { channel_id: 0 });
        assert!(matches!(left.as_slice(), [EventKind::Left { .. }]));
        assert_eq!(projector.resend().len(), 1);
    }
}
