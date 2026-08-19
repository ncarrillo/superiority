use super::*;

/// somebody the popup can offer, and the one line that says why they are in
/// the list at all.
#[derive(Clone)]
pub(in crate::app::client) struct PersonRow {
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) clan_tag: Option<String>,
    /// the tag is your own clan's, so it reads in bright gold.
    pub(in crate::app::client) own_clan: bool,
    pub(in crate::app::client) portrait: Option<Portrait>,
    pub(in crate::app::client) presence: PresenceKind,
    /// why this result ranks — `Friend · General`, `Clan · this channel`.
    pub(in crate::app::client) context: String,
    /// offline rows stay listed, because a whisper is delivered when they come
    /// back; they are dimmed rather than dropped.
    pub(in crate::app::client) offline: bool,
    pub(in crate::app::client) target: WhisperTarget,
}

impl PersonRow {
    pub(in crate::app::client) fn peer(&self) -> WhisperPeer {
        WhisperPeer {
            display: self.name.clone(),
            target: self.target.clone(),
        }
    }
}

/// who the composer is about to whisper: the name it shows and the handle it
/// sends to, which are not the same thing on this protocol.
#[derive(Clone)]
pub(in crate::app::client) struct WhisperPeer {
    pub(in crate::app::client) display: String,
    pub(in crate::app::client) target: WhisperTarget,
}

/// where we know somebody from. `/w` ranks in this order: the people you have a
/// standing relationship with, then the people in the room with you, then the
/// people you were talking to.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub(in crate::app::client) enum PersonSource {
    Friend,
    Clan,
    Channel,
    Whispered,
}

impl PersonSource {
    /// friends and clanmates share the top rank — one is a relationship you
    /// chose and the other one you joined, and neither outranks the other.
    pub(in crate::app::client) const fn rank(self) -> u8 {
        match self {
            Self::Friend | Self::Clan => 0,
            Self::Channel => 1,
            Self::Whispered => 2,
        }
    }

    const fn label(self) -> Option<&'static str> {
        match self {
            Self::Friend => Some("Friend"),
            Self::Clan => Some("Clan"),
            Self::Channel | Self::Whispered => None,
        }
    }
}

/// where somebody is, said the way the reader would say it.
#[derive(Clone, Copy)]
pub(in crate::app::client) enum PersonWhere<'a> {
    /// in the channel you are typing into.
    ThisChannel,
    /// in another channel you have open, which is worth naming.
    Channel(&'a str),
    /// not anywhere you can see, but a whisper still reaches them.
    Offline,
    Nowhere,
}

/// the sub-line under a name. it answers "why is this person in my list", which
/// is the only question a list of near-identical names raises.
pub(in crate::app::client) fn context_line(source: PersonSource, place: PersonWhere) -> String {
    let place = match place {
        PersonWhere::ThisChannel => Some("this channel".to_owned()),
        PersonWhere::Channel(channel) => Some(channel.to_owned()),
        // the promise matters more than the state: a whisper to someone offline
        // is not lost, and the row has to say so or it looks like a dead end
        PersonWhere::Offline => Some("Offline — will deliver".to_owned()),
        PersonWhere::Nowhere => None,
    };
    match (source.label(), place) {
        (Some(label), Some(place)) => format!("{label} · {place}"),
        (Some(label), None) => label.to_owned(),
        (None, Some(place)) => place,
        (None, None) => "Recent whisper".to_owned(),
    }
}

/// names match the way channel names do — anywhere in the name, ignoring case —
/// so a clan tag never has to be typed to find somebody behind it.
pub(in crate::app::client) fn matches_person(name: &str, query: &str) -> bool {
    query.trim().is_empty() || match_span(name, query).is_some()
}

/// one person, ranked. sorting is by where we know them from, then by whether
/// they are here to answer, then by name.
struct Ranked {
    row: PersonRow,
    source: PersonSource,
}

/// the people `/w` can reach: friends and clanmates first, then whoever is in
/// the rooms you have open, then whoever you were last talking to. offline
/// friends stay in the list — whispers are delivered when they come back.
pub(in crate::app::client) fn whisper_candidates(
    tabs: &[ChannelState],
    active: usize,
    friends: &[UiFriend],
    whispered: &BTreeMap<String, Vec<ConversationLine>>,
    query: &str,
    limit: usize,
) -> Vec<PersonRow> {
    let mut ranked: Vec<Ranked> = Vec::new();
    let mut seen = BTreeSet::new();

    for friend in friends
        .iter()
        .filter(|friend| matches_person(&friend.name, query))
    {
        if !seen.insert(friend.name.to_lowercase()) {
            continue;
        }
        let presence = presence_kind(friend.presence);
        let place = friend
            .is_online()
            .then(|| member_place(tabs, active, &friend.name))
            .flatten()
            .unwrap_or(if friend.is_online() {
                PersonWhere::Nowhere
            } else {
                PersonWhere::Offline
            });
        ranked.push(Ranked {
            row: PersonRow {
                name: friend.name.clone(),
                clan_tag: None,
                own_clan: false,
                portrait: friend
                    .portrait
                    .clone()
                    .map(|portrait| Portrait::Image(portrait.into())),
                presence,
                context: context_line(PersonSource::Friend, place),
                offline: !friend.is_online(),
                target: friend.target.clone(),
            },
            source: PersonSource::Friend,
        });
    }

    for (index, tab) in tabs.iter().enumerate() {
        for user in tab
            .users
            .iter()
            .filter(|user| matches_person(&user.name, query))
        {
            // the same person can be a friend, a clanmate, and someone you
            // whispered; they are one row, keyed on the person rather than on
            // the tag they happen to be wearing
            if !seen.insert(user.bare_name().to_lowercase()) {
                continue;
            }
            let source = if user.own_clan {
                PersonSource::Clan
            } else {
                PersonSource::Channel
            };
            let place = if index == active {
                PersonWhere::ThisChannel
            } else {
                PersonWhere::Channel(&tab.title)
            };
            ranked.push(Ranked {
                row: member_row(user, context_line(source, place)),
                source,
            });
        }
    }

    for peer in whispered.keys().filter(|peer| matches_person(peer, query)) {
        if !seen.insert(peer.to_lowercase()) {
            continue;
        }
        ranked.push(Ranked {
            row: PersonRow {
                name: peer.clone(),
                clan_tag: None,
                own_clan: false,
                portrait: None,
                presence: PresenceKind::Unknown,
                context: context_line(PersonSource::Whispered, PersonWhere::Nowhere),
                offline: false,
                target: WhisperTarget::Name(peer.clone()),
            },
            source: PersonSource::Whispered,
        });
    }

    rank(ranked, limit)
}

/// the people `@` can name: the room you are typing into, and nobody else. a
/// mention that nobody in the channel can see is a mention of nobody.
pub(in crate::app::client) fn mention_candidates(
    channel: Option<&ChannelState>,
    query: &str,
    limit: usize,
) -> Vec<PersonRow> {
    let Some(channel) = channel else {
        return Vec::new();
    };
    let ranked = channel
        .users
        .iter()
        .filter(|user| matches_person(&user.name, query))
        .map(|user| {
            let source = if user.own_clan {
                PersonSource::Clan
            } else {
                PersonSource::Channel
            };
            Ranked {
                row: member_row(user, context_line(source, PersonWhere::ThisChannel)),
                source,
            }
        })
        .collect();
    rank(ranked, limit)
}

/// a channel member as a result row. the name is taken bare — the tag is drawn
/// beside it, and a whisper is addressed to the person rather than to the clan
/// they are wearing.
fn member_row(user: &UiUser, context: String) -> PersonRow {
    PersonRow {
        name: user.bare_name().to_owned(),
        clan_tag: user.clan_tag.clone(),
        own_clan: user.own_clan,
        portrait: user
            .portrait
            .clone()
            .map(|portrait| Portrait::Image(portrait.into())),
        presence: user.presence,
        context,
        offline: matches!(user.presence, PresenceKind::Offline),
        target: member_target(user),
    }
}

/// how to reach somebody you are standing in a channel with: their presence
/// handle when the roster has one, and otherwise their name for the service to
/// resolve.
fn member_target(user: &UiUser) -> WhisperTarget {
    user.presence_id.map_or_else(
        || WhisperTarget::Name(user.bare_name().to_owned()),
        WhisperTarget::Presence,
    )
}

/// everybody `/w` could mean, as bare name-and-handle pairs. this is the pool
/// `whisper_candidates` ranks and dresses; splitting a typed line only needs to
/// know who is who.
fn whisper_pool(
    tabs: &[ChannelState],
    friends: &[UiFriend],
    whispered: &BTreeMap<String, Vec<ConversationLine>>,
) -> Vec<WhisperPeer> {
    let mut pool = Vec::new();
    let mut seen = BTreeSet::new();
    for friend in friends {
        if seen.insert(friend.name.to_lowercase()) {
            pool.push(WhisperPeer {
                display: friend.name.clone(),
                target: friend.target.clone(),
            });
        }
    }
    for user in tabs.iter().flat_map(|tab| tab.users.iter()) {
        if seen.insert(user.bare_name().to_lowercase()) {
            pool.push(WhisperPeer {
                display: user.bare_name().to_owned(),
                target: member_target(user),
            });
        }
    }
    for peer in whispered.keys() {
        if seen.insert(peer.to_lowercase()) {
            pool.push(WhisperPeer {
                display: peer.clone(),
                target: WhisperTarget::Name(peer.clone()),
            });
        }
    }
    pool
}

/// splits what was typed after `/w` into the person and the message they are
/// being sent. display names carry spaces — "Carlos Perez" is one person — so
/// the split is made at whoever the line actually names rather than at the
/// first gap: the longest prefix that is somebody's name wins, then the longest
/// that narrows to exactly one person, and failing both the first word is the
/// name and the service is left to resolve it.
pub(in crate::app::client) fn split_whisper(
    tabs: &[ChannelState],
    friends: &[UiFriend],
    whispered: &BTreeMap<String, Vec<ConversationLine>>,
    query: &str,
) -> Option<(WhisperPeer, String)> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    let pool = whisper_pool(tabs, friends, whispered);
    let splits = name_lengths(query);
    for &end in splits.iter().rev() {
        if let Some(peer) = pool
            .iter()
            .find(|peer| same_name(&peer.display, &query[..end]))
        {
            return Some((peer.clone(), query[end..].trim_start().to_owned()));
        }
    }
    for &end in splits.iter().rev() {
        let mut narrowed = pool
            .iter()
            .filter(|peer| matches_person(&peer.display, &query[..end]));
        if let (Some(peer), None) = (narrowed.next(), narrowed.next()) {
            return Some((peer.clone(), query[end..].trim_start().to_owned()));
        }
    }
    let (name, body) = query.split_once(char::is_whitespace).unwrap_or((query, ""));
    Some((
        WhisperPeer {
            display: name.to_owned(),
            target: WhisperTarget::Name(name.to_owned()),
        },
        body.trim_start().to_owned(),
    ))
}

/// every length the name could be: up to each gap in the line, and the whole
/// line. the query is trimmed, so none of them is empty.
fn name_lengths(query: &str) -> Vec<usize> {
    let mut lengths = query
        .char_indices()
        .filter(|(_, letter)| letter.is_whitespace())
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    lengths.push(query.len());
    lengths
}

fn same_name(left: &str, right: &str) -> bool {
    left.to_lowercase() == right.to_lowercase()
}

/// which open channel somebody is standing in, preferring the one you are
/// looking at.
fn member_place<'a>(
    tabs: &'a [ChannelState],
    active: usize,
    name: &str,
) -> Option<PersonWhere<'a>> {
    if tabs
        .get(active)
        .is_some_and(|tab| tab.users.iter().any(|user| user.bare_name() == name))
    {
        return Some(PersonWhere::ThisChannel);
    }
    tabs.iter()
        .find(|tab| tab.users.iter().any(|user| user.bare_name() == name))
        .map(|tab| PersonWhere::Channel(&tab.title))
}

fn rank(mut ranked: Vec<Ranked>, limit: usize) -> Vec<PersonRow> {
    ranked.sort_by(|left, right| {
        left.source
            .rank()
            .cmp(&right.source.rank())
            .then_with(|| left.row.offline.cmp(&right.row.offline))
            .then_with(|| {
                left.row
                    .name
                    .to_lowercase()
                    .cmp(&right.row.name.to_lowercase())
            })
    });
    ranked.truncate(limit);
    ranked.into_iter().map(|ranked| ranked.row).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::super::super::PresenceState;
    use super::{
        PersonSource, PersonWhere, UiFriend, WhisperTarget, context_line, matches_person,
        split_whisper,
    };

    #[test]
    fn the_sub_line_says_why_the_row_is_here() {
        // a list of near-identical names raises exactly one question, and this
        // line is the answer to it
        assert_eq!(
            context_line(PersonSource::Friend, PersonWhere::Channel("General")),
            "Friend · General"
        );
        assert_eq!(
            context_line(PersonSource::Clan, PersonWhere::ThisChannel),
            "Clan · this channel"
        );
        assert_eq!(
            context_line(PersonSource::Channel, PersonWhere::ThisChannel),
            "this channel"
        );
        // being unreachable is not the point — the promise that it still
        // arrives is, or the row reads as a dead end
        assert_eq!(
            context_line(PersonSource::Friend, PersonWhere::Offline),
            "Friend · Offline — will deliver"
        );
        assert_eq!(
            context_line(PersonSource::Whispered, PersonWhere::Nowhere),
            "Recent whisper"
        );
        assert_eq!(
            context_line(PersonSource::Friend, PersonWhere::Nowhere),
            "Friend"
        );
    }

    #[test]
    fn the_people_you_know_outrank_the_people_you_are_standing_next_to() {
        assert!(PersonSource::Friend.rank() < PersonSource::Channel.rank());
        assert!(PersonSource::Clan.rank() < PersonSource::Channel.rank());
        assert!(PersonSource::Channel.rank() < PersonSource::Whispered.rank());
        // a friend and a clanmate are both relationships; neither outranks the
        // other
        assert_eq!(PersonSource::Friend.rank(), PersonSource::Clan.rank());
    }

    #[test]
    fn a_name_matches_the_way_a_channel_name_does() {
        assert!(matches_person("NelsonTest91", "nel"));
        assert!(matches_person("NelsonTest91", "TEST"));
        assert!(matches_person("NelsonTest91", ""));
        assert!(!matches_person("NelsonTest91", "zerg"));
    }

    #[test]
    fn a_typed_whisper_splits_at_the_person_not_at_the_first_gap() {
        let friends = [friend("NelsonTest91"), friend("Carlos Perez")];
        assert_eq!(
            split(&friends, "NelsonTest91 hey"),
            named("NelsonTest91", "hey")
        );
        // a display name can carry a space, and reading it as a name plus a
        // message would send "Perez hello there" to somebody called Carlos
        assert_eq!(
            split(&friends, "Carlos Perez hello there"),
            named("Carlos Perez", "hello there")
        );
        // half a name is enough while it still means one person
        assert_eq!(split(&friends, "nelson hey"), named("NelsonTest91", "hey"));
        // naming somebody without saying anything is still just opening the
        // conversation
        assert_eq!(split(&friends, "NelsonTest91"), named("NelsonTest91", ""));
    }

    #[test]
    fn a_name_nobody_answers_to_is_still_the_first_word() {
        let friends = [friend("NelsonTest91")];
        assert_eq!(split(&friends, "stranger hi"), named("stranger", "hi"));
        // two people behind the same fragment is not a name, so it is left as
        // typed rather than guessed at
        let crowded = [friend("Nova"), friend("Novaa")];
        assert_eq!(split(&crowded, "nov hello"), named("nov", "hello"));
    }

    fn split(friends: &[UiFriend], query: &str) -> (String, String) {
        let (peer, body) = split_whisper(&[], friends, &BTreeMap::new(), query)
            .expect("a query with something in it names somebody");
        (peer.display, body)
    }

    fn friend(name: &str) -> UiFriend {
        UiFriend {
            name: name.to_owned(),
            presence: PresenceState::Available,
            portrait: None,
            target: WhisperTarget::Name(name.to_owned()),
        }
    }

    fn named(peer: &str, body: &str) -> (String, String) {
        (peer.to_owned(), body.to_owned())
    }
}
