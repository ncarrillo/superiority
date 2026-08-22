//! Projecting a Remastered classic session into Live wire events.
//!
//! The classic channel is described whole and re-sent on every roster move, so
//! this diffs successive snapshots the way the `StarCraft II` [`Projector`] does
//! — a full [`Roster`] the first time and whenever the member set changes, a
//! [`RosterDelta`] when only presence or avatars move. Chat lines become
//! [`Message`]s and [`Notice`]s; whispers and local errors never leave the
//! machine, matching the SC2 privacy line.
//!
//! [`Projector`]: super::model::Projector
//! [`Roster`]: super::model::EventKind::Roster
//! [`RosterDelta`]: super::model::EventKind::RosterDelta
//! [`Message`]: super::model::EventKind::Message
//! [`Notice`]: super::model::EventKind::Notice

use std::collections::BTreeMap;

use superiority_core::games::scr::chat::{ChatChannel, ChatEvent, ChatUser, EventKind as ScrEvent};

use super::classic::{fnv1a32, roster_events, truncate};
use super::model::{ChannelRef, EventKind, MessageSubkind, NoticeSubkind, UserRef};

pub(super) struct ScrProjector {
    /// The signed-in account's BattleTag, lowercased, so the local member can
    /// be marked. The classic roster never says which entry is you.
    local_identity: Option<String>,
    /// The channel this session is in: its numeric id and the wire key.
    channel: Option<(u32, String)>,
    roster: BTreeMap<u32, UserRef>,
    roster_sent: bool,
}

impl ScrProjector {
    pub(super) fn new(local_identity: Option<String>) -> Self {
        Self {
            local_identity: local_identity.map(|tag| tag.trim().to_lowercase()),
            channel: None,
            roster: BTreeMap::new(),
            roster_sent: false,
        }
    }

    /// Turns a whole-channel snapshot into the events that carry the change:
    /// a leave and a join when the channel itself changed, then the roster.
    pub(super) fn channel_events(&mut self, channel: &ChatChannel) -> Vec<EventKind> {
        let key = channel_key(channel);
        let name = channel.label().to_owned();
        let mut events = Vec::new();
        let is_new = self.channel.as_ref().map(|(id, _)| *id) != Some(channel.channel_id);
        if is_new {
            if let Some((_, previous)) = self.channel.take() {
                events.push(EventKind::Left {
                    channel: ChannelRef {
                        key: previous,
                        name: None,
                    },
                });
            }
            self.channel = Some((channel.channel_id, key.clone()));
            self.roster.clear();
            self.roster_sent = false;
            events.push(EventKind::Joined {
                channel: ChannelRef {
                    key: key.clone(),
                    name: Some(name.clone()),
                },
            });
        }
        let members = channel
            .users
            .iter()
            .map(|user| {
                let handle = fnv1a32(user.name.to_lowercase().as_bytes());
                (handle, self.user_ref(handle, user))
            })
            .collect::<BTreeMap<_, _>>();
        let channel_ref = ChannelRef {
            key,
            name: Some(name),
        };
        events.extend(roster_events(
            channel_ref,
            members,
            &mut self.roster,
            &mut self.roster_sent,
        ));
        events
    }

    /// One chat line, or `None` when it must not leave the machine.
    pub(super) fn message_event(&self, event: &ChatEvent) -> Option<EventKind> {
        let (_, key) = self.channel.as_ref()?;
        let channel = ChannelRef {
            key: key.clone(),
            name: None,
        };
        let body = || truncate(event.text.clone().unwrap_or_default());
        match event.kind {
            ScrEvent::Talk | ScrEvent::Emote => {
                let sender_name = event.sender.clone()?;
                let handle = fnv1a32(sender_name.to_lowercase().as_bytes());
                Some(EventKind::Message {
                    channel,
                    subkind: (event.kind == ScrEvent::Emote).then_some(MessageSubkind::Emote),
                    sender: Some(UserRef {
                        handle,
                        name: Some(sender_name),
                        clan_tag: None,
                        presence: None,
                        portrait: None,
                        is_local: None,
                        joined_order: None,
                        avatar: None,
                        is_operator: None,
                    }),
                    body: body(),
                })
            }
            ScrEvent::Broadcast => Some(EventKind::Notice {
                channel,
                subkind: NoticeSubkind::Broadcast,
                body: body(),
            }),
            ScrEvent::Information => Some(EventKind::Notice {
                channel,
                subkind: NoticeSubkind::Information,
                body: body(),
            }),
            // a whisper is private; an error is a local UI concern
            ScrEvent::Whisper | ScrEvent::Error => None,
        }
    }

    /// The current roster as a complete snapshot, for re-announcing after Live
    /// was toggled back on.
    pub(super) fn resend(&self) -> Vec<EventKind> {
        let Some((_, key)) = self.channel.as_ref() else {
            return Vec::new();
        };
        if !self.roster_sent {
            return Vec::new();
        }
        let users = self.roster.values().cloned().collect::<Vec<_>>();
        vec![EventKind::Roster {
            channel: ChannelRef {
                key: key.clone(),
                name: None,
            },
            complete: true,
            count: u32::try_from(users.len()).unwrap_or(u32::MAX),
            users,
        }]
    }

    /// The next snapshot resends the whole roster — used when the session was
    /// re-announced and the viewer needs a fresh, complete member list.
    pub(super) fn reset_roster(&mut self) {
        self.roster_sent = false;
    }

    fn user_ref(&self, handle: u32, user: &ChatUser) -> UserRef {
        let is_local = self.local_identity.as_ref().is_some_and(|identity| {
            user.battle_tag()
                .is_some_and(|tag| tag.trim().to_lowercase() == *identity)
        });
        UserRef {
            handle,
            name: Some(user.name.clone()),
            clan_tag: None,
            presence: Some(user.presence().slug()),
            portrait: None,
            is_local: Some(is_local),
            joined_order: None,
            avatar: avatar_id(user),
            is_operator: Some(user.is_operator),
        }
    }
}

/// The wire key for a classic channel: public channels by their numeric id,
/// custom channels by name. The product dimension keeps this from colliding
/// with `StarCraft II`'s `public:<id>`.
fn channel_key(channel: &ChatChannel) -> String {
    if channel.is_public {
        format!("public:{}", channel.channel_id)
    } else {
        format!("private:{}", channel.name)
    }
}

/// The member's avatar as a bare profile id the viewer resolves to its own
/// asset — `avatar_terran_marine`, or a full URL if the service gave one.
fn avatar_id(user: &ChatUser) -> Option<String> {
    let raw = user
        .avatar
        .as_ref()
        .and_then(|avatar| avatar.id.clone().or_else(|| avatar.image_url.clone()))
        .or_else(|| {
            [
                "avatar_url",
                "avatar_id",
                "avatar",
                "profile_avatar",
                "toon_profile_avatar",
            ]
            .into_iter()
            .find_map(|name| user.attribute(name).map(str::to_owned))
        })?;
    Some(normalize_avatar_id(&raw))
}

fn normalize_avatar_id(value: &str) -> String {
    let value = value.trim();
    if value.starts_with("http://") || value.starts_with("https://") {
        return value.to_owned();
    }
    let id = value.split_once('?').map_or(value, |(id, _)| id);
    id.strip_suffix(".jpg")
        .or_else(|| id.strip_suffix(".png"))
        .unwrap_or(id)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use superiority_core::games::scr::{chat::ChatAttribute, profile::Avatar};

    fn user(name: &str, operator: bool, attrs: &[(&str, &str)], avatar: Option<&str>) -> ChatUser {
        ChatUser {
            name: name.into(),
            flags: Some(0),
            is_operator: operator,
            avatar: avatar.map(|id| Avatar {
                image_url: None,
                id: Some(id.into()),
            }),
            attributes: attrs
                .iter()
                .map(|(name, value)| ChatAttribute {
                    name: (*name).into(),
                    value: (*value).into(),
                })
                .collect(),
        }
    }

    fn channel(id: u32, users: Vec<ChatUser>) -> ChatChannel {
        ChatChannel {
            channel_id: id,
            name: "Public Chat 1".into(),
            display_name: Some("Public Chat 1".into()),
            is_public: true,
            users,
        }
    }

    #[test]
    fn first_snapshot_is_a_join_and_a_complete_roster() {
        let mut projector = ScrProjector::new(Some("Commander#1234".into()));
        let events = projector.channel_events(&channel(
            9,
            vec![
                user(
                    "Commander",
                    true,
                    &[("battle_tag", "Commander#1234")],
                    Some("avatar_terran_marine.jpg"),
                ),
                user("Darko", false, &[], None),
            ],
        ));
        assert!(matches!(events[0], EventKind::Joined { .. }));
        let EventKind::Roster {
            users, complete, ..
        } = &events[1]
        else {
            panic!("expected a complete roster, got {:?}", events[1]);
        };
        assert!(complete);
        assert_eq!(users.len(), 2);
        let local = users
            .iter()
            .find(|user| user.is_local == Some(true))
            .expect("a local member");
        assert_eq!(local.name.as_deref(), Some("Commander"));
        assert_eq!(local.is_operator, Some(true));
        assert_eq!(local.avatar.as_deref(), Some("avatar_terran_marine"));
    }

    #[test]
    fn a_presence_change_is_a_delta_not_a_full_roster() {
        let mut projector = ScrProjector::new(None);
        let quiet = || user("Darko", false, &[], None);
        let away = || user("Darko", false, &[("status", "away")], None);
        projector.channel_events(&channel(9, vec![quiet()]));
        assert!(
            projector
                .channel_events(&channel(9, vec![quiet()]))
                .is_empty()
        );
        match projector
            .channel_events(&channel(9, vec![away()]))
            .as_slice()
        {
            [EventKind::RosterDelta { users, .. }] => {
                assert_eq!(users.len(), 1);
                assert_eq!(users[0].presence, Some("away"));
            }
            other => panic!("expected a single delta, got {other:?}"),
        }
    }

    #[test]
    fn a_departure_resends_the_whole_roster_so_the_leaver_is_dropped() {
        let mut projector = ScrProjector::new(None);
        projector.channel_events(&channel(
            9,
            vec![
                user("Darko", false, &[], None),
                user("Kerrigan", false, &[], None),
            ],
        ));
        match projector
            .channel_events(&channel(9, vec![user("Darko", false, &[], None)]))
            .as_slice()
        {
            [
                EventKind::Roster {
                    users, complete, ..
                },
            ] => {
                assert!(complete);
                assert_eq!(users.len(), 1);
                assert_eq!(users[0].name.as_deref(), Some("Darko"));
            }
            other => panic!("expected a complete roster, got {other:?}"),
        }
    }

    #[test]
    fn talk_emote_broadcast_and_information_project_but_whisper_does_not() {
        let mut projector = ScrProjector::new(None);
        projector.channel_events(&channel(9, vec![user("Darko", false, &[], None)]));
        let event = |kind, text: &str, sender: Option<&str>| ChatEvent {
            kind,
            channel_id: Some(9),
            sender: sender.map(str::to_owned),
            text: Some(text.into()),
            aurora_whisper: None,
        };
        assert!(matches!(
            projector.message_event(&event(ScrEvent::Talk, "hi", Some("Darko"))),
            Some(EventKind::Message {
                subkind: None,
                sender: Some(_),
                ..
            })
        ));
        assert!(matches!(
            projector.message_event(&event(ScrEvent::Emote, "waves", Some("Darko"))),
            Some(EventKind::Message {
                subkind: Some(MessageSubkind::Emote),
                ..
            })
        ));
        assert!(matches!(
            projector.message_event(&event(ScrEvent::Broadcast, "maintenance", None)),
            Some(EventKind::Notice {
                subkind: NoticeSubkind::Broadcast,
                ..
            })
        ));
        assert!(matches!(
            projector.message_event(&event(ScrEvent::Information, "welcome", None)),
            Some(EventKind::Notice {
                subkind: NoticeSubkind::Information,
                ..
            })
        ));
        assert!(
            projector
                .message_event(&event(ScrEvent::Whisper, "psst", Some("Darko")))
                .is_none()
        );
    }
}
