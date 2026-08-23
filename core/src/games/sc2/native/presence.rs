use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use crate::{Error, Result, bsn::bits::BitReader};

use super::model::{
    AdvertHandle, ImageTableEntry, PresenceField, PresenceFields, PresenceUpdate, ProfileAddress,
    ToonHandle,
};

pub const FIELD_AVATAR: u32 = 65_555;
pub const FIELD_TOON_PROFILE: u32 = 65_556;
pub const FIELD_TOON_HANDLE: u32 = 65_561;
const FIELD_ACCOUNT_ID: u32 = 0x0001_0005;
const FIELD_SOCIAL_ACCOUNT_ID: u32 = 0x0001_001b;
pub const FIELD_CLAN_TAG: u32 = 0x50004;
pub const FIELD_IN_GAME: u32 = 0x50002;
/// the game somebody is advertising, as `Presence::S2GameInfo`. `FIELD_IN_GAME`
/// only says *that* they are playing; this says *which* lobby, and is the one
/// place a whole `AdvertHandle` crosses the wire — chat links carry a third of
/// one. the client reads the same field: `get_field(out, presence, 0x60001)`
/// then `tag == 21` (`s2GameInfoval`) at `0x100c06330` in the 97563 image.
pub const FIELD_GAME_INFO: u32 = 0x60001;
pub const FIELD_AWAY: u32 = 0x10020;
pub const FIELD_AWAY_FALLBACK: u32 = 0x10010;
pub const FIELD_BUSY: u32 = 0x10022;
pub const FIELD_BUSY_FALLBACK: u32 = 0x10011;
/// the `PresenceFieldSpec` type that carries an account id, from a capture of
/// field `0x10015`. Note this is *not* the `Battlenet::Presence::FieldVal`
/// choice tag, where 21 is `s2GameInfoval` — the spec's type byte and the
/// value's choice tag are separate enumerations, and only the latter matches
/// the metadata.
const TYPE_ACCOUNT_INFO: u8 = 21;

const STANDING_ORDER: [(u32, PresenceState); 4] = [
    (FIELD_BUSY_FALLBACK, PresenceState::Busy),
    (FIELD_AWAY_FALLBACK, PresenceState::Away),
    (FIELD_BUSY, PresenceState::Busy),
    (FIELD_AWAY, PresenceState::Away),
];

fn is_standing(handle: u32) -> bool {
    handle == FIELD_IN_GAME || STANDING_ORDER.iter().any(|(field, _)| *field == handle)
}

const SESSION_MARKS: [u32; 2] = [0x0001_0003, 0x0001_0009];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PresenceState {
    Available,
    Away,
    Busy,
    InGame,
    Offline,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PresenceIdentity {
    pub account_id: Option<u32>,
    pub profile: Option<ProfileAddress>,
    pub toon_handle: Option<ToonHandle>,
    pub avatar: Option<ImageTableEntry>,
    pub clan_tag: Option<String>,
    pub state: PresenceState,
}

#[derive(Clone, Debug, Default)]
pub struct PresenceDirectory {
    fields: BTreeMap<u32, PresenceField>,
    values: BTreeMap<u32, BTreeMap<u32, Vec<u8>>>,
    aliases: BTreeMap<u32, u32>,
    local_presence: BTreeMap<u32, u32>,
    account_presence: BTreeMap<u32, u32>,
    profile_presence: BTreeMap<ProfileAddress, u32>,
    toon_presence: BTreeMap<ToonHandle, u32>,
    online: BTreeMap<u32, bool>,
    seen: BTreeMap<u32, u64>,
    standing: BTreeMap<u32, BTreeMap<u32, u64>>,
    sessioned: BTreeSet<u32>,
    clock: u64,
}

impl PresenceDirectory {
    pub fn announce(&mut self, fields: &PresenceFields) {
        self.fields
            .extend(fields.entries.iter().map(|field| (field.handle, *field)));
    }

    pub fn apply(&mut self, update: &PresenceUpdate) -> Result<PresenceIdentity> {
        let canonical = self.canonical_id(update.local_presence_id, update.master_presence_id);
        if canonical == 0 {
            return Err(presence_error("presence update has no identity"));
        }
        let mut offset = 0_usize;
        let mut variable_index = 0_usize;
        let mut decoded = Vec::with_capacity(update.handles.len());
        let mut unread = false;
        for handle in &update.handles {
            let Some(field) = self.fields.get(handle) else {
                unread = true;
                break;
            };
            let size = if let Some(size) = field.fixed_size {
                usize::from(size)
            } else {
                let size = update
                    .variable_sizes
                    .get(variable_index)
                    .ok_or_else(|| presence_error("presence update omits a variable field size"))?;
                variable_index += 1;
                usize::from(*size)
            };
            let end = offset
                .checked_add(size)
                .ok_or_else(|| presence_error("presence field size overflows"))?;
            let bytes = update.field_data.get(offset..end).ok_or_else(|| {
                presence_error(format!(
                    "presence update field data is shorter than announced: data_bytes={}, \
                     offset={offset}, field_handle={handle}, field_size={size}, \
                     handles={}, variable_sizes={}",
                    update.field_data.len(),
                    update.handles.len(),
                    update.variable_sizes.len()
                ))
            })?;
            decoded.push((*handle, bytes.to_vec()));
            offset = end;
        }
        if !unread
            && (offset != update.field_data.len() || variable_index != update.variable_sizes.len())
        {
            return Err(presence_error(format!(
                "presence update field sizes do not cover the payload: data_bytes={}, \
                 consumed_bytes={offset}, variable_sizes={}, consumed_variable_sizes={variable_index}",
                update.field_data.len(),
                update.variable_sizes.len()
            )));
        }
        self.merge_alias(update.local_presence_id, canonical);
        self.merge_alias(update.master_presence_id, canonical);
        for id in [update.local_presence_id, update.master_presence_id] {
            if id != 0 {
                self.aliases.insert(id, canonical);
            }
        }
        if update.local_presence_id != 0 {
            self.local_presence
                .insert(canonical, update.local_presence_id);
        }
        self.clock = self.clock.wrapping_add(1);
        let clock = self.clock;
        let told = decoded
            .iter()
            .map(|(handle, _)| *handle)
            .filter(|handle| is_standing(*handle))
            .collect::<Vec<_>>();
        let values = self.values.entry(canonical).or_default();
        for handle in &update.cleared_handles {
            values.remove(handle);
        }
        for (handle, bytes) in decoded {
            if handle == FIELD_GAME_INFO {
                trace_game_info(canonical, &bytes);
            }
            values.insert(handle, bytes);
        }
        let standing = self.standing.entry(canonical).or_default();
        for handle in &update.cleared_handles {
            standing.remove(handle);
        }
        for handle in told {
            standing.insert(handle, clock);
        }
        let values = &self.values[&canonical];
        let held = SESSION_MARKS
            .iter()
            .filter(|mark| values.contains_key(mark))
            .count();
        if held > 0 {
            self.sessioned.insert(canonical);
        }
        let gone = held == 0 && self.sessioned.contains(&canonical);
        self.online.insert(canonical, update.online && !gone);
        self.seen.insert(canonical, clock);
        self.rebuild_identity_indexes();
        let canonical = self.canonical_presence_id(canonical);
        Ok(self.identity(canonical))
    }

    fn rebuild_identity_indexes(&mut self) {
        self.account_presence.clear();
        self.profile_presence.clear();
        self.toon_presence.clear();
        let mut account_candidates = BTreeMap::<u32, (u8, u32)>::new();
        let mut owners = BTreeMap::<u32, u32>::new();
        let mut duplicates = Vec::new();

        for (canonical, values) in &self.values {
            let Some(local_presence_id) = self.local_presence.get(canonical).copied() else {
                continue;
            };
            let identity = identity_from_values(
                values,
                &self.fields,
                self.online.get(canonical).copied(),
                self.standing.get(canonical).unwrap_or(&BTreeMap::new()),
            );
            if let Some(account_id) = identity.account_id {
                match owners.entry(account_id) {
                    Entry::Vacant(slot) => {
                        slot.insert(*canonical);
                    }
                    Entry::Occupied(mut slot) => {
                        let held = *slot.get();
                        let (keep, folded) = if self.seen.get(canonical) >= self.seen.get(&held) {
                            (*canonical, held)
                        } else {
                            (held, *canonical)
                        };
                        slot.insert(keep);
                        duplicates.push((folded, keep));
                    }
                }
                let priority = account_field_priority(values, &self.fields);
                let candidate = account_candidates
                    .entry(account_id)
                    .or_insert((priority, local_presence_id));
                if priority > candidate.0 {
                    *candidate = (priority, local_presence_id);
                }
            }
            if let Some(profile) = identity.profile {
                self.profile_presence.insert(profile, local_presence_id);
            }
            if let Some(toon_handle) = identity.toon_handle {
                self.toon_presence.insert(toon_handle, local_presence_id);
            }
        }

        if !duplicates.is_empty() {
            for (folded, keep) in duplicates {
                let keep = self.canonical_presence_id(keep);
                if folded != keep && self.values.contains_key(&folded) {
                    self.fold_identity(folded, keep);
                }
            }
            self.rebuild_identity_indexes();
            return;
        }

        self.account_presence.extend(
            account_candidates
                .into_iter()
                .map(|(account_id, (_, presence_id))| (account_id, presence_id)),
        );
    }

    fn fold_identity(&mut self, folded: u32, keep: u32) {
        if let Some(values) = self.values.remove(&folded) {
            let kept = self.values.entry(keep).or_default();
            for (handle, value) in values {
                kept.entry(handle).or_insert(value);
            }
        }
        if let Some(standing) = self.standing.remove(&folded) {
            let kept = self.standing.entry(keep).or_default();
            for (handle, written) in standing {
                kept.entry(handle).or_insert(written);
            }
        }
        if let Some(online) = self.online.remove(&folded) {
            self.online.entry(keep).or_insert(online);
        }
        if let Some(local) = self.local_presence.remove(&folded) {
            self.local_presence.entry(keep).or_insert(local);
        }
        if let Some(seen) = self.seen.remove(&folded) {
            let kept = self.seen.entry(keep).or_insert(seen);
            *kept = (*kept).max(seen);
        }
        if self.sessioned.remove(&folded) {
            self.sessioned.insert(keep);
        }
        for canonical in self.aliases.values_mut() {
            if *canonical == folded {
                *canonical = keep;
            }
        }
        self.aliases.insert(folded, keep);
    }

    fn canonical_id(&self, local: u32, master: u32) -> u32 {
        [master, local]
            .into_iter()
            .filter(|id| *id != 0)
            .find_map(|id| self.aliases.get(&id).copied())
            .unwrap_or(if master != 0 { master } else { local })
    }

    fn merge_alias(&mut self, id: u32, canonical: u32) {
        let Some(previous) = self.aliases.get(&id).copied() else {
            return;
        };
        if previous == canonical {
            return;
        }
        if let Some(previous_values) = self.values.remove(&previous) {
            let values = self.values.entry(canonical).or_default();
            for (handle, value) in previous_values {
                values.entry(handle).or_insert(value);
            }
        }
        if let Some(previous_online) = self.online.remove(&previous) {
            self.online.entry(canonical).or_insert(previous_online);
        }
        if let Some(previous_local) = self.local_presence.remove(&previous) {
            self.local_presence
                .entry(canonical)
                .or_insert(previous_local);
        }
        for alias in self.aliases.values_mut() {
            if *alias == previous {
                *alias = canonical;
            }
        }
    }

    #[must_use]
    pub fn identity(&self, presence_id: u32) -> PresenceIdentity {
        let canonical = self.canonical_presence_id(presence_id);
        self.values.get(&canonical).map_or_else(
            || PresenceIdentity {
                state: state_from_values(
                    &BTreeMap::new(),
                    self.online.get(&canonical).copied(),
                    &BTreeMap::new(),
                ),
                ..PresenceIdentity::default()
            },
            |values| {
                identity_from_values(
                    values,
                    &self.fields,
                    self.online.get(&canonical).copied(),
                    self.standing.get(&canonical).unwrap_or(&BTreeMap::new()),
                )
            },
        )
    }

    #[must_use]
    pub fn presence_for_account(&self, account_id: u32) -> Option<u32> {
        self.account_presence.get(&account_id).copied()
    }

    #[must_use]
    pub fn presence_for_profile(&self, profile: ProfileAddress) -> Option<u32> {
        self.profile_presence.get(&profile).copied()
    }

    #[must_use]
    pub fn presence_for_toon_handle(&self, toon_handle: ToonHandle) -> Option<u32> {
        self.toon_presence.get(&toon_handle).copied()
    }

    #[must_use]
    pub fn local_presence_id(&self, presence_id: u32) -> u32 {
        let canonical = self.canonical_presence_id(presence_id);
        self.local_presence
            .get(&canonical)
            .copied()
            .unwrap_or(presence_id)
    }

    /// every advert handle presence is showing, one per person in a joinable
    /// game.
    ///
    /// A value that reads more than one way is settled by the company it keeps:
    /// people are spread across a handful of game servers, so the alignment
    /// that names a `(label, epoch)` other presences also name is the real one,
    /// and the rest are bit-shifted noise that names a server nobody is on.
    /// How many presences read unambiguously as being on each game server. An
    /// ambiguous value cannot vote, because it would vote for its own noise
    /// alongside its real reading.
    fn attested_servers(&self) -> BTreeMap<(u32, i32), usize> {
        let mut attested = BTreeMap::new();
        for values in self.values.values() {
            if let Some(value) = values.get(&FIELD_GAME_INFO)
                && let [advert] = game_advert_candidates(value).as_slice()
            {
                *attested
                    .entry((advert.server_label, advert.server_epoch))
                    .or_default() += 1;
            }
        }
        attested
    }

    #[must_use]
    pub fn adverts(&self) -> Vec<AdvertHandle> {
        let attested = self.attested_servers();
        let mut adverts = self
            .values
            .values()
            .filter_map(|values| resolve_advert(values.get(&FIELD_GAME_INFO)?, &attested))
            .collect::<Vec<_>>();
        // a newer server epoch is a server that came up later, which is the
        // best ordering available without a clock of our own.
        adverts.sort_by_key(|advert| std::cmp::Reverse(advert.server_epoch));
        adverts.dedup();
        adverts
    }

    /// the game server everybody visible is playing on, when they agree on
    /// one — the two thirds of an `AdvertHandle` a chat link throws away.
    ///
    /// Walks every presence and bit-decodes each game info value, so sample it
    /// when presence changes rather than per frame.
    #[must_use]
    pub fn advert_server(&self) -> Option<(u32, i32)> {
        self.attested_servers()
            .into_iter()
            .max_by_key(|(_, votes)| *votes)
            .map(|(server, _)| server)
    }

    /// completes an advert id lifted out of a `lobbyLink(...)` in chat.
    ///
    /// The link carries only `m_advertId`; the server label and epoch are
    /// dropped when it is written, so they are borrowed from a handle we have
    /// seen whole. Presence is the only source of one. An exact match wins —
    /// that lobby is somebody's current game, so all three fields are already
    /// known — and otherwise the server everybody is on stands in.
    ///
    /// Same cost as [`PresenceDirectory::advert_server`]. Call it for a link
    /// somebody is acting on, not for every link on screen; to resolve many,
    /// take the server once and build the handles from it.
    #[must_use]
    pub fn resolve_advert_id(&self, advert_id: u32) -> Option<AdvertHandle> {
        let adverts = self.adverts();
        adverts
            .iter()
            .find(|advert| advert.advert_id == advert_id)
            .copied()
            .or_else(|| {
                let (server_label, server_epoch) = self.advert_server()?;
                Some(AdvertHandle {
                    server_label,
                    server_epoch,
                    advert_id,
                })
            })
    }

    #[must_use]
    pub fn canonical_presence_id(&self, presence_id: u32) -> u32 {
        self.aliases
            .get(&presence_id)
            .copied()
            .unwrap_or(presence_id)
    }
}

fn account_field_priority(
    values: &BTreeMap<u32, Vec<u8>>,
    fields: &BTreeMap<u32, PresenceField>,
) -> u8 {
    if values.contains_key(&FIELD_SOCIAL_ACCOUNT_ID) {
        2
    } else {
        u8::from(
            values.contains_key(&FIELD_ACCOUNT_ID)
                || values.keys().any(|handle| {
                    fields
                        .get(handle)
                        .is_some_and(|field| field.identifier == TYPE_ACCOUNT_INFO)
                }),
        )
    }
}

fn identity_from_values(
    values: &BTreeMap<u32, Vec<u8>>,
    fields: &BTreeMap<u32, PresenceField>,
    online: Option<bool>,
    written: &BTreeMap<u32, u64>,
) -> PresenceIdentity {
    PresenceIdentity {
        account_id: [FIELD_SOCIAL_ACCOUNT_ID, FIELD_ACCOUNT_ID]
            .into_iter()
            .find_map(|handle| {
                values
                    .get(&handle)
                    .and_then(|value| decode_account_id(value))
            })
            .or_else(|| {
                values.iter().find_map(|(handle, value)| {
                    (fields.get(handle)?.identifier == TYPE_ACCOUNT_INFO)
                        .then(|| decode_account_id(value))
                        .flatten()
                })
            }),
        profile: values
            .get(&FIELD_TOON_PROFILE)
            .and_then(|value| decode_profile_address(value)),
        toon_handle: values
            .get(&FIELD_TOON_HANDLE)
            .and_then(|value| decode_toon_handle(value)),
        avatar: values
            .get(&FIELD_AVATAR)
            .and_then(|value| decode_image_table_entry(value)),
        clan_tag: values
            .get(&FIELD_CLAN_TAG)
            .and_then(|value| decode_string_literal(value)),
        state: state_from_values(values, online, written),
    }
}

fn decode_account_id(value: &[u8]) -> Option<u32> {
    let account_id = u32::from_be_bytes(value.get(..4)?.try_into().ok()?);
    (account_id != 0).then_some(account_id)
}

fn state_from_values(
    values: &BTreeMap<u32, Vec<u8>>,
    online: Option<bool>,
    written: &BTreeMap<u32, u64>,
) -> PresenceState {
    if online == Some(false) {
        return PresenceState::Offline;
    }
    if bool_field(values, FIELD_IN_GAME) {
        return PresenceState::InGame;
    }
    let latest = STANDING_ORDER
        .into_iter()
        .filter(|(handle, _)| values.contains_key(handle))
        .max_by_key(|(handle, _)| written.get(handle).copied().unwrap_or_default());
    if let Some((handle, state)) = latest
        && bool_field(values, handle)
    {
        return state;
    }
    match online {
        Some(true) => PresenceState::Available,
        Some(false) => PresenceState::Offline,
        None => PresenceState::Unknown,
    }
}

fn bool_field(values: &BTreeMap<u32, Vec<u8>>, handle: u32) -> bool {
    values
        .get(&handle)
        .is_some_and(|value| value.as_slice() == [1])
}

fn decode_string_literal(value: &[u8]) -> Option<String> {
    let pair_count = usize::from(*value.first()?);
    let trailing_byte = usize::from(*value.get(1)?);
    if trailing_byte > 1 {
        return None;
    }
    let byte_count = pair_count.checked_mul(2)?.checked_add(trailing_byte)?;
    let encoded = value.get(2..)?;
    if encoded.len() != byte_count {
        return None;
    }
    let decoded = std::str::from_utf8(encoded).ok()?.trim();
    (!decoded.is_empty()).then(|| decoded.to_owned())
}

fn decode_toon_handle(value: &[u8]) -> Option<ToonHandle> {
    Some(ToonHandle {
        region: *value.first()?,
        program_id: u32::from_be_bytes(value.get(1..5)?.try_into().ok()?),
        realm: u32::from_be_bytes(value.get(5..9)?.try_into().ok()?),
        id: u64::from_be_bytes(value.get(9..17)?.try_into().ok()?),
    })
}

/// the one field whose wire layout is inferred rather than captured, so it
/// prints itself the first time anybody is seen in a game. Enable with
/// `SUPERIORITY_PRESENCE_TRACE=1`; the hex is what confirms — or corrects —
/// [`decode_game_advert`].
fn trace_game_info(presence_id: u32, bytes: &[u8]) {
    if std::env::var_os("SUPERIORITY_PRESENCE_TRACE").is_none() {
        return;
    }
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    let candidates = game_advert_candidates(bytes);
    let decoded = match candidates.as_slice() {
        [] => "not joinable".to_owned(),
        [advert] => format!(
            "label={} epoch={} advert={}",
            advert.server_label, advert.server_epoch, advert.advert_id
        ),
        // several alignments read as a handle; which one is real is settled
        // against the other presences, not against this value alone
        many => format!(
            "{} alignments, adverts {:?}",
            many.len(),
            many.iter()
                .map(|advert| advert.advert_id)
                .collect::<Vec<_>>()
        ),
    };
    eprintln!(
        "superiority: presence {presence_id} game info {} bytes {hex} -> {decoded}",
        bytes.len()
    );
}

/// the advert out of a `S2GameInfo` presence value.
///
/// `S2GameInfo` is `{m_shortLink, m_advert}` bit-packed, and the short link in
/// front is variable length — 109 bits in the shortest captures, 177 in the
/// next, more again with further map entries — so the advert cannot be found by
/// counting forward without decoding the link. It is read from the back
/// instead: `m_advert` is a 1-bit choice whose `joinable` arm (tag 0) is the
/// whole 96-bit handle and whose `nonJoinable` arm is void, and the value is
/// padded to a byte, so a joinable value ends with
/// `[tag=0][96-bit handle][<8 zero bits]`.
///
/// How many of those trailing bits are padding is the one thing the value does
/// not say, so every alignment is tried and the implausible ones dropped: the
/// tag must be zero, the padding must be zero, and `m_serverEpoch` — a
/// `Time::Seconds`, so 32 raw bits biased by `-i32::MIN` — must be a real wall
/// clock time. What survives is usually one candidate; see
/// [`PresenceDirectory::adverts`] for the rest.
#[must_use]
pub fn game_advert_candidates(value: &[u8]) -> Vec<AdvertHandle> {
    // a server that came up before StarCraft II ran on this protocol, or a
    // decade after now, is a misalignment rather than a lobby.
    const EPOCH_FLOOR: i64 = 1_400_000_000;
    const EPOCH_CEILING: i64 = 2_000_000_000;
    const HANDLE_BITS: usize = 96;

    let total = value.len() * 8;
    (0..8)
        .filter_map(|padding| {
            let start = total.checked_sub(HANDLE_BITS + padding)?;
            let mut reader = BitReader::new(value, None).ok()?;
            // the choice tag sits immediately in front of the arm it selects
            reader.set_position(start.checked_sub(1)?).ok()?;
            if reader.read(1).ok()? != 0 {
                return None;
            }
            let server_label = u32::try_from(reader.read(32).ok()?).ok()?;
            let epoch = i64::from(i32::MIN) + i64::try_from(reader.read(32).ok()?).ok()?;
            let advert_id = u32::try_from(reader.read(32).ok()?).ok()?;
            if !(EPOCH_FLOOR..=EPOCH_CEILING).contains(&epoch) {
                return None;
            }
            (reader.read(padding).ok()? == 0).then_some(AdvertHandle {
                server_label,
                server_epoch: i32::try_from(epoch).ok()?,
                advert_id,
            })
        })
        .collect()
}

/// the advert a value carries, settled against the servers other presences are
/// known to be on. A value that reads several ways is common — roughly half of
/// them — and the reading that names a server somebody else is unambiguously
/// on is the real one; the rest name servers nobody is on.
#[must_use]
pub fn resolve_advert(
    value: &[u8],
    attested: &BTreeMap<(u32, i32), usize>,
) -> Option<AdvertHandle> {
    let candidates = game_advert_candidates(value);
    // one reading needs nothing to settle it, and is what votes for the rest
    if let [advert] = candidates.as_slice() {
        return Some(*advert);
    }
    // otherwise only a server somebody is known to be on can be believed.
    // early in a session nothing is attested yet, and answering anyway would
    // just be picking whichever alignment happened to sort last.
    candidates
        .into_iter()
        .filter_map(|advert| {
            let votes = attested.get(&(advert.server_label, advert.server_epoch))?;
            (*votes > 0).then_some((*votes, advert))
        })
        .max_by_key(|(votes, _)| *votes)
        .map(|(_, advert)| advert)
}

/// the advert when the value only reads one way, for a blob with no other
/// presences to settle it against.
#[must_use]
pub fn decode_game_advert(value: &[u8]) -> Option<AdvertHandle> {
    match game_advert_candidates(value).as_slice() {
        [advert] => Some(*advert),
        _ => None,
    }
}

fn decode_profile_address(value: &[u8]) -> Option<ProfileAddress> {
    let label = u32::from_be_bytes(value.get(..4)?.try_into().ok()?);
    let id = u64::from_be_bytes(value.get(4..12)?.try_into().ok()?);
    Some(ProfileAddress { label, id })
}

fn decode_image_table_entry(value: &[u8]) -> Option<ImageTableEntry> {
    Some(ImageTableEntry {
        table_id: u16::from_be_bytes(value.get(..2)?.try_into().ok()?),
        offset: u16::from_be_bytes(value.get(2..4)?.try_into().ok()?),
    })
}

fn presence_error(message: impl Into<String>) -> Error {
    Error::Native(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::model::PresenceFieldFlags;

    /// `FIELD_GAME_INFO` values captured off a live session, 2026-08-19. Nine
    /// people in games on one server, three on another, and sixteen playing
    /// something nobody can join.
    const CAPTURED_GAME_INFO: &[&str] = &[
        "411985cd320101eb1200068002c018ab3bf9a911342101775405",
        "401985cd320102d1b800000032c018ab3bf9a91134210177023e",
        "401985cd320102056c0100001220",
        "411985cd720002d1020100000400002056140000050003",
        "451985cd3201023c67000001a8c018ab3bf9a911342101774e19",
        "401985cd32010223320100887cc018ab3bf9a911342101775a29",
        "401985cd320102c9590100803ec018ab3bf9a911342101775537",
        "401985cd32010249d00000000621",
        "411985cd32010196c70101003020",
        "451985cd32010201fb00078005c118ab3bf9a911342101774017",
        "421985cd72000260da0000002301002056140000050003",
        "401985cd32010243c90101000cc018ab3bf9a911342101775131",
        "401985cd320102056e0000001121",
        "401985cd320102a4e501008113c118ab3bf9a91134210177572c",
        "411985cd720002e30f0000000200002056140000050003",
        "411985cd720002b2ce000000080000217b0800280002c58ab3bee99113480117740d00",
        "421985cd7200018b930000800700002056140000050003",
        "411985cd720002e30f00000002000020561400000500c58ab3bee9911348011775b000",
        "401985cdb200020a7300000010000025d20e000000040102176600000004020022bc160000000503",
        "401985cd320102197c0000001021",
        "431985cd72000251b50000002f01002056140000050003",
        "411985cd720002e00d0000000200002056140000050003",
        "411985cd720002e0fe0100000200002056140000050003",
        "411985cd720002df090100000200002056140000050003",
        "431985cd720000001400008006000020561400000500c58ab3bee9911348011775c000",
        "401985cd320102056d0100001020",
        "441985cd720001f23f0000800700002056140000050003",
        "471985cd32010196c701010030c018ab3bf9a911342101775b32",
    ];

    /// a second, larger capture off the same session. Half of these read more
    /// than one way on their own, which is what [`resolve_advert`] is for.
    const CAPTURED_GAME_INFO_BUSY: &[&str] = &[
        "411985cd32010196c70101003020",
        "411985cd720002de140100000001002056140000050003",
        "411985cd720002df090000000200002056140000050003",
        "401985cd3201027d970100008fc018ab3bf9a911342101776c21",
        "401985cd320100343701008031c118ab3bf9a911342101777a02",
        "401985cd32010196c701010030c018ab3bf9a911342101778338",
        "401985cd32010239860100801cc118ab3bf9a911342101777c28",
        "401985cd320102cf670100802bc118ab3bf9a91134210177811c",
        "401985cd320102452c01070002c118ab3bf9a911342101778336",
        "4339bdc96d01021b6e00000047010029830e000800000102056a00000050c218ab3bf9a911342101778239",
        "411985cd32010196c701010030c018ab3bf9a911342101778403",
        "401985cd32010220380100000d21",
        "401985cd32010278b3010100c9c118ab3bf9a911342101777b3a",
        "411985cd320101ef5c0101000fc118ab3bf9a911342101778011",
        "401985cd720001d54101008000000020561400000500c58ab3bee99113480117779a03",
        "401985cd320101eb1200068002c018ab3bf9a911342101778033",
        "401985cd320102a87f00008028c018ab3bf9a911342101777537",
        "401985cd720002afd3000080000000207c1300100000c58ab3bee99113480117783b02",
        "401985cd320102bbe40101800cc018ab3bf9a91134210177750f",
        "401985cd320102cf670100802bc118ab3bf9a911342101775d19",
        "401985cd32010172c60002800dc018ab3bf9a911342101778029",
        "471985cd32010196c701010030c018ab3bf9a911342101778125",
        "401985cd720002948201008000010025260e0010020cc58ab3bee99113480117771600",
        "4d1985cd720002948201008000010028e50e00000510c58ab3bee99113480117782e00",
        "4a1985cd32010196c70101003020",
        "421985cd7201023e630101800201001bda080008000c0102056a00000050c218ab3bf9a911342101777d32",
        "401985cd320101ed6c01008008c018ab3bf9a911342101776b38",
        "431985cd720002286c0000004700002056140000050003",
        "401985cd32010239860100801c21",
        "401985cd320102a88a01010007c018ab3bf9a91134210177640d",
        "401985cd3201024b580000003dc118ab3bf9a911342101773c05",
        "401985cd3201025bef01010020c118ab3bf9a911342101777f21",
        "401985cd320102804501000038c118ab3bf9a911342101777213",
        "411985cd320102970600000015c018ab3bf9a91134210177722c",
        "421985cd320101eb1200068002c018ab3bf9a911342101778333",
        "441985cd720002c4f70100000800002056140000050003",
        "401985cd320102817000008001c118ab3bf9a911342101778413",
        "401985cd32010034370100803121",
        "431985cd720000002001008008010020561400000500c58ab3bee99113480117784501",
        "411985cd720002d10400000004000020561400000500c58ab3bee99113480117784600",
        "441985cd720001654501008000000020561400000500c58ab3bee99113480117760403",
        "401985cd320102056c0100001220",
        "421985cd720002baf20000000900002056140000050003",
        "441985cd720002c4f40100000701002056140000050003",
        "401985cd320102452c0107000221",
        "401985cd320102056d0100001020",
        "401985cd320102056e0000001121",
        "421985cd7200018b930000800700002056140000050003",
        "431985cd720002594601008005010028e50e00000510c58ab3bee9911348011777dd03",
        "401985cd320102a4e50100811321",
        "411985cd720002e0ff0000000200002056140000050003",
        "401985cd32010249d00000000621",
        "491985cd72000196f300010000000020561400000500c58ab3bee99113480117784702",
        "4a1985cd32010196c701010030c018ab3bf9a91134210177841f",
        "401985cd320102197c0000001021",
        "411985cd720002e3100000000200002056140000050003",
        "411985cd720002e0fe0100000200002056140000050003",
        "401985cd3201020a730000001020",
        "431985cd72000265610000001d00002056140000050003",
        "401985cd32010281700000800121",
    ];

    fn captured() -> Vec<Vec<u8>> {
        CAPTURED_GAME_INFO
            .iter()
            .map(|value| hex::decode(value).expect("captured value is hex"))
            .collect()
    }

    /// one presence per captured value. Each carries a distinct profile as
    /// well, because presences with no identity at all fold into each other.
    fn directory_of(values: &[Vec<u8>]) -> PresenceDirectory {
        let mut directory = PresenceDirectory::default();
        directory.announce(&PresenceFields {
            entries: vec![
                PresenceField {
                    handle: FIELD_TOON_PROFILE,
                    identifier: 19,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(12),
                },
                PresenceField {
                    // whatever the spec calls this type, it is not the account
                    // info one — that is 21, and claiming it here would have
                    // every value read as an account id
                    handle: FIELD_GAME_INFO,
                    identifier: 0,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: None,
                },
            ],
        });
        for (index, value) in values.iter().enumerate() {
            let presence_id = 100 + u32::try_from(index).expect("index fits");
            let mut profile = 0xcafe_babe_u32.to_be_bytes().to_vec();
            profile.extend(u64::from(presence_id).to_be_bytes());
            let mut field_data = profile;
            field_data.extend_from_slice(value);
            directory
                .apply(&PresenceUpdate {
                    local_presence_id: presence_id,
                    master_presence_id: presence_id,
                    field_data,
                    cleared_handles: Vec::new(),
                    handles: vec![FIELD_TOON_PROFILE, FIELD_GAME_INFO],
                    variable_sizes: vec![u16::try_from(value.len()).expect("length fits")],
                    online: true,
                })
                .expect("presence update applies");
        }
        directory
    }

    #[test]
    fn a_joinable_game_is_the_values_that_end_in_a_handle() {
        // the twelve longer captures carry an advert; the sixteen shorter ones
        // are the same games with the non-joinable arm, and must not decode to
        // a handle just because 96 bits can be read off the end of anything
        let joinable = captured()
            .iter()
            .filter(|value| !game_advert_candidates(value).is_empty())
            .count();
        assert_eq!(joinable, 12);
        assert_eq!(
            captured()
                .iter()
                .filter(|value| value.len() == 26 || value.len() == 35)
                .count(),
            12
        );
    }

    #[test]
    fn the_company_a_value_keeps_settles_how_it_reads() {
        let adverts = directory_of(&captured()).adverts();
        assert_eq!(adverts.len(), 12);
        // everybody is on one game server, and the alignments that claimed
        // otherwise were noise
        let servers = adverts
            .iter()
            .map(|advert| (advert.server_label, advert.server_epoch))
            .collect::<BTreeSet<_>>();
        assert_eq!(servers, BTreeSet::from([(3_324_694_265, 1_782_861_089)]));
        // and each of them is in their own lobby
        assert_eq!(
            adverts
                .iter()
                .map(|advert| advert.advert_id)
                .collect::<BTreeSet<_>>()
                .len(),
            12
        );
    }

    #[test]
    fn a_chat_advert_id_borrows_the_server_it_is_missing() {
        let directory = directory_of(&captured());
        let known = directory.adverts()[0];
        // a lobby somebody is standing in comes back whole
        assert_eq!(
            directory.resolve_advert_id(known.advert_id),
            Some(known),
            "an advert we can already see should not be guessed at"
        );
        // one nobody is in borrows the server, which is the whole point: the
        // chat link never carried it
        assert_eq!(
            directory.resolve_advert_id(4_242_424),
            Some(AdvertHandle {
                server_label: 3_324_694_265,
                server_epoch: 1_782_861_089,
                advert_id: 4_242_424,
            })
        );
    }

    #[test]
    fn a_value_that_reads_several_ways_is_settled_by_the_others() {
        let busy = CAPTURED_GAME_INFO_BUSY
            .iter()
            .map(|value| hex::decode(value).expect("captured value is hex"))
            .collect::<Vec<_>>();
        // half of a busy channel's values are ambiguous on their own
        let ambiguous = busy
            .iter()
            .filter(|value| game_advert_candidates(value).len() > 1)
            .count();
        assert_eq!(ambiguous, 20);

        let adverts = directory_of(&busy).adverts();
        // every joinable value resolves, ambiguous ones included
        assert_eq!(adverts.len(), 35);
        // onto one game server, because that is where everybody is
        assert_eq!(
            adverts
                .iter()
                .map(|advert| (advert.server_label, advert.server_epoch))
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([(3_324_694_265, 1_782_861_089)])
        );
        // each in their own lobby, with ids allocated in one tight run — which
        // is what a wrong alignment would not produce
        let ids = adverts
            .iter()
            .map(|advert| advert.advert_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), 35);
        let (first, last) = (
            *ids.iter().next().expect("an id"),
            *ids.iter().next_back().expect("an id"),
        );
        assert!(last - first < 10_000, "{first}..{last} is not one run");
    }

    #[test]
    fn the_server_is_the_one_everybody_is_on() {
        let busy = CAPTURED_GAME_INFO_BUSY
            .iter()
            .map(|value| hex::decode(value).expect("captured value is hex"))
            .collect::<Vec<_>>();
        assert_eq!(
            directory_of(&busy).advert_server(),
            Some((3_324_694_265, 1_782_861_089))
        );
        // and with nobody in a game there is no server to borrow
        assert_eq!(PresenceDirectory::default().advert_server(), None);
    }

    #[test]
    fn an_unsettled_value_is_left_unread_rather_than_guessed() {
        // "471985cd…779010" reads two ways: advert 6153232 on the server
        // everybody is on, and 539947536 on a server nobody is on. with nobody
        // else seen yet there is nothing to tell them apart, and answering
        // would just be taking whichever alignment sorted last
        let value =
            hex::decode("471985cd32010196c701010030c018ab3bf9a911342101779010").expect("hex");
        assert_eq!(game_advert_candidates(&value).len(), 2);
        assert_eq!(resolve_advert(&value, &BTreeMap::new()), None);
        // one person seen unambiguously on that server is enough to settle it
        let attested = BTreeMap::from([((3_324_694_265, 1_782_861_089), 1)]);
        assert_eq!(
            resolve_advert(&value, &attested).map(|advert| advert.advert_id),
            Some(6_153_232)
        );
    }

    #[test]
    fn without_a_whole_handle_there_is_nothing_to_borrow_from() {
        assert_eq!(PresenceDirectory::default().resolve_advert_id(12345), None);
    }

    #[test]
    fn reconstructs_profile_and_avatar_fields() {
        let mut directory = PresenceDirectory::default();
        directory.announce(&PresenceFields {
            entries: vec![
                PresenceField {
                    handle: FIELD_TOON_PROFILE,
                    identifier: 5,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(12),
                },
                PresenceField {
                    handle: FIELD_AVATAR,
                    identifier: 1,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(4),
                },
                PresenceField {
                    handle: FIELD_TOON_HANDLE,
                    identifier: 22,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(17),
                },
            ],
        });
        let mut data = hex::decode("cafebabedc82c80e00000000").unwrap();
        data.extend(hex::decode("00020051").unwrap());
        data.extend(hex::decode("01000053320000000100000000008440c1").unwrap());
        let identity = directory
            .apply(&PresenceUpdate {
                local_presence_id: 41,
                master_presence_id: 91,
                field_data: data,
                cleared_handles: Vec::new(),
                handles: vec![FIELD_TOON_PROFILE, FIELD_AVATAR, FIELD_TOON_HANDLE],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();

        assert_eq!(
            identity.profile,
            Some(ProfileAddress {
                label: 0xcafe_babe,
                id: 0xdc82_c80e_0000_0000,
            })
        );
        assert_eq!(
            identity.avatar,
            Some(ImageTableEntry {
                table_id: 2,
                offset: 81,
            })
        );
        assert_eq!(
            identity.toon_handle,
            Some(ToonHandle {
                region: 1,
                program_id: crate::bgs::fourcc("S2"),
                realm: 1,
                id: 0x84_40c1,
            })
        );
        assert_eq!(identity.state, PresenceState::Available);
        assert_eq!(directory.identity(41), identity);
        assert_eq!(directory.identity(91), identity);
        assert_eq!(directory.local_presence_id(41), 41);
        assert_eq!(directory.local_presence_id(91), 41);
        assert_eq!(
            directory.presence_for_profile(ProfileAddress {
                label: 0xcafe_babe,
                id: 0xdc82_c80e_0000_0000,
            }),
            Some(41)
        );
    }

    #[test]
    fn tracks_online_and_offline_presence_without_profile_fields() {
        let mut directory = PresenceDirectory::default();
        let online = directory
            .apply(&PresenceUpdate {
                local_presence_id: 41,
                master_presence_id: 91,
                field_data: Vec::new(),
                cleared_handles: Vec::new(),
                handles: Vec::new(),
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();
        assert_eq!(online.state, PresenceState::Available);
        assert_eq!(directory.identity(41).state, PresenceState::Available);

        let offline = directory
            .apply(&PresenceUpdate {
                local_presence_id: 41,
                master_presence_id: 91,
                field_data: Vec::new(),
                cleared_handles: Vec::new(),
                handles: Vec::new(),
                variable_sizes: Vec::new(),
                online: false,
            })
            .unwrap();
        assert_eq!(offline.state, PresenceState::Offline);
        assert_eq!(directory.identity(91).state, PresenceState::Offline);
    }

    #[test]
    fn maps_account_info_to_the_connection_local_presence_id() {
        const ACCOUNT_INFO: u32 = 0x0001_0015;
        let mut directory = PresenceDirectory::default();
        directory.announce(&PresenceFields {
            entries: vec![PresenceField {
                handle: ACCOUNT_INFO,
                identifier: TYPE_ACCOUNT_INFO,
                flags: PresenceFieldFlags::default(),
                fixed_size: Some(7),
            }],
        });

        let identity = directory
            .apply(&PresenceUpdate {
                local_presence_id: 34_222_470,
                master_presence_id: 91,
                field_data: hex::decode("12629863010000").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![ACCOUNT_INFO],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();

        assert_eq!(identity.account_id, Some(308_451_427));
        assert_eq!(
            directory.presence_for_account(308_451_427),
            Some(34_222_470)
        );
    }

    #[test]
    fn maps_retail_social_account_field_to_the_whisper_presence_id() {
        let mut directory = PresenceDirectory::default();
        directory.announce(&PresenceFields {
            entries: vec![
                PresenceField {
                    handle: FIELD_ACCOUNT_ID,
                    identifier: 5,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(4),
                },
                PresenceField {
                    handle: FIELD_SOCIAL_ACCOUNT_ID,
                    identifier: 5,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(4),
                },
            ],
        });

        directory
            .apply(&PresenceUpdate {
                local_presence_id: 38_137_964,
                master_presence_id: 38_137_964,
                field_data: hex::decode("3b1b6652").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_ACCOUNT_ID],
                variable_sizes: Vec::new(),
                online: false,
            })
            .unwrap();

        let identity = directory
            .apply(&PresenceUpdate {
                local_presence_id: 38_137_963,
                master_presence_id: 38_137_963,
                field_data: hex::decode("3b1b6652").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_SOCIAL_ACCOUNT_ID],
                variable_sizes: Vec::new(),
                online: false,
            })
            .unwrap();

        assert_eq!(identity.account_id, Some(991_651_410));
        assert_eq!(
            directory.presence_for_account(991_651_410),
            Some(38_137_963)
        );
    }

    #[test]
    fn links_identity_when_the_local_id_arrives_before_account_info() {
        const ACCOUNT_INFO: u32 = 0x0001_0015;
        let mut directory = PresenceDirectory::default();
        directory.announce(&PresenceFields {
            entries: vec![PresenceField {
                handle: ACCOUNT_INFO,
                identifier: TYPE_ACCOUNT_INFO,
                flags: PresenceFieldFlags::default(),
                fixed_size: Some(7),
            }],
        });

        directory
            .apply(&PresenceUpdate {
                local_presence_id: 34_222_470,
                master_presence_id: 91,
                field_data: Vec::new(),
                cleared_handles: Vec::new(),
                handles: Vec::new(),
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();
        directory
            .apply(&PresenceUpdate {
                local_presence_id: 0,
                master_presence_id: 91,
                field_data: hex::decode("12629863010000").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![ACCOUNT_INFO],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();

        assert_eq!(
            directory.presence_for_account(308_451_427),
            Some(34_222_470)
        );
    }

    #[test]
    fn links_identity_when_account_info_arrives_before_the_local_id() {
        const ACCOUNT_INFO: u32 = 0x0001_0015;
        let mut directory = PresenceDirectory::default();
        directory.announce(&PresenceFields {
            entries: vec![PresenceField {
                handle: ACCOUNT_INFO,
                identifier: TYPE_ACCOUNT_INFO,
                flags: PresenceFieldFlags::default(),
                fixed_size: Some(7),
            }],
        });

        directory
            .apply(&PresenceUpdate {
                local_presence_id: 0,
                master_presence_id: 91,
                field_data: hex::decode("12629863010000").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![ACCOUNT_INFO],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();
        directory
            .apply(&PresenceUpdate {
                local_presence_id: 34_222_470,
                master_presence_id: 91,
                field_data: Vec::new(),
                cleared_handles: Vec::new(),
                handles: Vec::new(),
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();

        assert_eq!(
            directory.presence_for_account(308_451_427),
            Some(34_222_470)
        );
    }

    #[test]
    fn retains_fields_when_followup_updates_omit_the_master_id() {
        let mut directory = PresenceDirectory::default();
        directory.announce(&PresenceFields {
            entries: vec![PresenceField {
                handle: FIELD_IN_GAME,
                identifier: 1,
                flags: PresenceFieldFlags::default(),
                fixed_size: Some(1),
            }],
        });

        let in_game = directory
            .apply(&PresenceUpdate {
                local_presence_id: 41,
                master_presence_id: 91,
                field_data: vec![1],
                cleared_handles: Vec::new(),
                handles: vec![FIELD_IN_GAME],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();
        assert_eq!(in_game.state, PresenceState::InGame);

        let followup = directory
            .apply(&PresenceUpdate {
                local_presence_id: 41,
                master_presence_id: 0,
                field_data: Vec::new(),
                cleared_handles: Vec::new(),
                handles: Vec::new(),
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();
        assert_eq!(followup.state, PresenceState::InGame);
        assert_eq!(directory.identity(41), followup);
        assert_eq!(directory.identity(91), followup);
    }

    #[test]
    fn applies_retail_presence_priority() {
        let told = BTreeMap::new();
        let mut values = BTreeMap::new();
        assert_eq!(
            state_from_values(&values, None, &told),
            PresenceState::Unknown
        );
        assert_eq!(
            state_from_values(&values, Some(false), &told),
            PresenceState::Offline
        );
        assert_eq!(
            state_from_values(&values, Some(true), &told),
            PresenceState::Available
        );

        values.insert(FIELD_BUSY_FALLBACK, vec![1]);
        assert_eq!(
            state_from_values(&values, Some(true), &told),
            PresenceState::Busy
        );
        values.insert(FIELD_AWAY_FALLBACK, vec![1]);
        assert_eq!(
            state_from_values(&values, Some(true), &told),
            PresenceState::Away
        );
        values.insert(FIELD_IN_GAME, vec![1]);
        assert_eq!(
            state_from_values(&values, Some(true), &told),
            PresenceState::InGame
        );
    }

    #[test]
    fn reconstructs_retail_clan_tag_field() {
        let mut directory = PresenceDirectory::default();
        directory.announce(&PresenceFields {
            entries: vec![PresenceField {
                handle: FIELD_CLAN_TAG,
                identifier: 13,
                flags: PresenceFieldFlags::default(),
                fixed_size: None,
            }],
        });

        let identity = directory
            .apply(&PresenceUpdate {
                local_presence_id: 44_449_582,
                master_presence_id: 44_430_591,
                field_data: hex::decode("03004255524e494e").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_CLAN_TAG],
                variable_sizes: vec![8],
                online: true,
            })
            .unwrap();

        assert_eq!(identity.clan_tag.as_deref(), Some("BURNIN"));
        assert_eq!(directory.canonical_presence_id(44_449_582), 44_430_591);
        assert_eq!(directory.canonical_presence_id(44_430_591), 44_430_591);
        assert_eq!(directory.identity(44_449_582), identity);
        assert_eq!(directory.identity(44_430_591), identity);
    }

    #[test]
    fn rejects_malformed_presence_string_literals() {
        assert_eq!(decode_string_literal(&[3, 0, b'B', b'N', b'U']), None);
        assert_eq!(decode_string_literal(&[1, 2, b'B', b'N', b'U']), None);
    }

    fn standing_fields() -> PresenceFields {
        PresenceFields {
            entries: vec![
                PresenceField {
                    handle: FIELD_SOCIAL_ACCOUNT_ID,
                    identifier: 5,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(4),
                },
                PresenceField {
                    handle: FIELD_ACCOUNT_ID,
                    identifier: 5,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(4),
                },
                PresenceField {
                    handle: FIELD_AWAY,
                    identifier: 6,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(1),
                },
                PresenceField {
                    handle: FIELD_IN_GAME,
                    identifier: 6,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(1),
                },
                PresenceField {
                    handle: FIELD_BUSY,
                    identifier: 6,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(1),
                },
                PresenceField {
                    handle: FIELD_AWAY_FALLBACK,
                    identifier: 6,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(1),
                },
                PresenceField {
                    handle: FIELD_BUSY_FALLBACK,
                    identifier: 6,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(1),
                },
                PresenceField {
                    handle: SESSION_MARKS[0],
                    identifier: 7,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(4),
                },
                PresenceField {
                    handle: SESSION_MARKS[1],
                    identifier: 7,
                    flags: PresenceFieldFlags::default(),
                    fixed_size: Some(4),
                },
            ],
        }
    }

    #[test]
    fn one_account_under_two_presence_ids_is_one_person() {
        const ACCOUNT: &str = "12629863";
        let mut directory = PresenceDirectory::default();
        directory.announce(&standing_fields());

        directory
            .apply(&PresenceUpdate {
                local_presence_id: 46_902_359,
                master_presence_id: 46_902_359,
                field_data: hex::decode(ACCOUNT).unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_SOCIAL_ACCOUNT_ID],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();
        directory
            .apply(&PresenceUpdate {
                local_presence_id: 46_902_360,
                master_presence_id: 46_902_360,
                field_data: hex::decode(ACCOUNT).unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_SOCIAL_ACCOUNT_ID],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();

        let account_id = directory.identity(46_902_360).account_id.unwrap();
        let bound = directory.presence_for_account(account_id).unwrap();

        directory
            .apply(&PresenceUpdate {
                local_presence_id: 46_902_359,
                master_presence_id: 46_902_359,
                field_data: hex::decode(format!("{ACCOUNT}01")).unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_SOCIAL_ACCOUNT_ID, FIELD_AWAY],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();

        assert_eq!(directory.identity(bound).state, PresenceState::Away);
        assert_eq!(directory.identity(46_902_359).state, PresenceState::Away);
        assert_eq!(directory.identity(46_902_360).state, PresenceState::Away);
    }

    #[test]
    fn a_standing_granted_after_another_is_the_one_that_holds() {
        let mut directory = PresenceDirectory::default();
        directory.announce(&standing_fields());
        let mut set = |handle: u32| {
            directory
                .apply(&PresenceUpdate {
                    local_presence_id: 46_925_812,
                    master_presence_id: 46_925_812,
                    field_data: hex::decode("01").unwrap(),
                    cleared_handles: Vec::new(),
                    handles: vec![handle],
                    variable_sizes: Vec::new(),
                    online: true,
                })
                .unwrap()
                .state
        };

        assert_eq!(set(FIELD_AWAY), PresenceState::Away);
        assert_eq!(set(FIELD_BUSY), PresenceState::Busy);
        assert_eq!(set(FIELD_AWAY), PresenceState::Away);
    }

    #[test]
    fn a_retail_status_cycle_reads_the_way_it_was_set() {
        const ACCOUNT: &str = "12629863";
        let mut directory = PresenceDirectory::default();
        directory.announce(&standing_fields());
        let mut told = |id: u32, handles: Vec<u32>, cleared: Vec<u32>, data: &str, online: bool| {
            directory
                .apply(&PresenceUpdate {
                    local_presence_id: id,
                    master_presence_id: id,
                    field_data: hex::decode(data).unwrap(),
                    cleared_handles: cleared,
                    handles,
                    variable_sizes: Vec::new(),
                    online,
                })
                .unwrap()
                .state
        };

        assert_eq!(
            told(
                46_925_812,
                vec![FIELD_ACCOUNT_ID],
                Vec::new(),
                ACCOUNT,
                false
            ),
            PresenceState::Offline
        );
        assert_eq!(
            told(
                46_925_813,
                vec![FIELD_SOCIAL_ACCOUNT_ID],
                Vec::new(),
                ACCOUNT,
                false
            ),
            PresenceState::Offline
        );
        assert_eq!(
            told(
                46_925_812,
                SESSION_MARKS.to_vec(),
                Vec::new(),
                "ea7797f3ea7797f2",
                true
            ),
            PresenceState::Available
        );
        assert_eq!(
            told(
                46_925_813,
                vec![FIELD_AWAY_FALLBACK, FIELD_BUSY_FALLBACK],
                Vec::new(),
                "0000",
                true
            ),
            PresenceState::Available
        );
        assert_eq!(
            told(46_925_812, vec![FIELD_AWAY], Vec::new(), "01", true),
            PresenceState::Away
        );
        assert_eq!(
            told(46_925_812, vec![FIELD_BUSY], Vec::new(), "01", true),
            PresenceState::Busy
        );
        assert_eq!(
            told(46_925_812, vec![FIELD_AWAY], Vec::new(), "01", true),
            PresenceState::Away
        );
        assert_eq!(
            told(46_925_812, Vec::new(), vec![SESSION_MARKS[1]], "", true),
            PresenceState::Away
        );
        assert_eq!(
            told(46_925_813, Vec::new(), vec![SESSION_MARKS[0]], "", true),
            PresenceState::Offline
        );

        let bound = directory.presence_for_account(308_451_427).unwrap();
        assert_eq!(directory.identity(bound).state, PresenceState::Offline);
    }

    #[test]
    fn leaving_is_announced_by_taking_the_session_marks_back() {
        let mut directory = PresenceDirectory::default();
        directory.announce(&standing_fields());
        let mut apply = |handles: Vec<u32>, cleared: Vec<u32>, data: &str| {
            directory
                .apply(&PresenceUpdate {
                    local_presence_id: 46_920_343,
                    master_presence_id: 46_920_343,
                    field_data: hex::decode(data).unwrap(),
                    cleared_handles: cleared,
                    handles,
                    variable_sizes: Vec::new(),
                    online: true,
                })
                .unwrap()
                .state
        };

        assert_eq!(
            apply(SESSION_MARKS.to_vec(), Vec::new(), "ea7797f3ea7797f2"),
            PresenceState::Available
        );
        assert_eq!(
            apply(Vec::new(), vec![SESSION_MARKS[1]], ""),
            PresenceState::Available,
            "one mark going is not leaving"
        );
        assert_eq!(
            apply(Vec::new(), vec![SESSION_MARKS[0]], ""),
            PresenceState::Offline
        );
    }

    #[test]
    fn somebody_who_never_carried_a_session_mark_is_not_taken_as_gone() {
        let mut directory = PresenceDirectory::default();
        directory.announce(&standing_fields());
        let state = directory
            .apply(&PresenceUpdate {
                local_presence_id: 77,
                master_presence_id: 77,
                field_data: hex::decode("01").unwrap(),
                cleared_handles: vec![FIELD_BUSY],
                handles: vec![FIELD_AWAY],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap()
            .state;

        assert_eq!(state, PresenceState::Away);
    }

    #[test]
    fn coming_back_is_announced_by_taking_both_standings_back() {
        let mut directory = PresenceDirectory::default();
        directory.announce(&standing_fields());
        directory
            .apply(&PresenceUpdate {
                local_presence_id: 46_925_812,
                master_presence_id: 46_925_812,
                field_data: hex::decode("01").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_AWAY],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();

        let back = directory
            .apply(&PresenceUpdate {
                local_presence_id: 46_925_812,
                master_presence_id: 46_925_812,
                field_data: hex::decode("0000").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_AWAY_FALLBACK, FIELD_BUSY_FALLBACK],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();

        assert_eq!(back.state, PresenceState::Available);
    }

    #[test]
    fn leaving_outranks_what_an_identity_was_last_doing() {
        let mut directory = PresenceDirectory::default();
        directory.announce(&standing_fields());
        directory
            .apply(&PresenceUpdate {
                local_presence_id: 41,
                master_presence_id: 41,
                field_data: hex::decode("01").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_IN_GAME],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();
        assert_eq!(directory.identity(41).state, PresenceState::InGame);

        let gone = directory
            .apply(&PresenceUpdate {
                local_presence_id: 41,
                master_presence_id: 41,
                field_data: Vec::new(),
                cleared_handles: Vec::new(),
                handles: Vec::new(),
                variable_sizes: Vec::new(),
                online: false,
            })
            .unwrap();

        assert_eq!(gone.state, PresenceState::Offline);
    }

    #[test]
    fn an_unannounced_field_does_not_hide_a_change_of_standing() {
        let mut directory = PresenceDirectory::default();
        directory.announce(&PresenceFields {
            entries: vec![PresenceField {
                handle: FIELD_TOON_PROFILE,
                identifier: 5,
                flags: PresenceFieldFlags::default(),
                fixed_size: Some(12),
            }],
        });
        let online = directory
            .apply(&PresenceUpdate {
                local_presence_id: 41,
                master_presence_id: 91,
                field_data: hex::decode("cafebabedc82c80e00000000").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_TOON_PROFILE],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();
        assert_eq!(online.state, PresenceState::Available);

        let offline = directory
            .apply(&PresenceUpdate {
                local_presence_id: 41,
                master_presence_id: 91,
                field_data: hex::decode("cafebabedc82c80e00000000ff").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_TOON_PROFILE, 0x0009_0003],
                variable_sizes: Vec::new(),
                online: false,
            })
            .unwrap();

        assert_eq!(offline.state, PresenceState::Offline);
        assert_eq!(offline.profile, online.profile);
    }

    #[test]
    fn rejected_update_does_not_mutate_presence_state() {
        let mut directory = PresenceDirectory::default();
        directory.announce(&PresenceFields {
            entries: vec![PresenceField {
                handle: FIELD_TOON_PROFILE,
                identifier: 5,
                flags: PresenceFieldFlags::default(),
                fixed_size: Some(12),
            }],
        });
        let original = directory
            .apply(&PresenceUpdate {
                local_presence_id: 41,
                master_presence_id: 91,
                field_data: hex::decode("cafebabedc82c80e00000000").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![FIELD_TOON_PROFILE],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();

        let error = directory
            .apply(&PresenceUpdate {
                local_presence_id: 42,
                master_presence_id: 91,
                field_data: vec![0],
                cleared_handles: vec![FIELD_TOON_PROFILE],
                handles: vec![FIELD_TOON_PROFILE],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("presence update field data is shorter than announced")
        );
        assert_eq!(directory.identity(91), original);
        assert_eq!(directory.identity(42), PresenceIdentity::default());
    }
}
