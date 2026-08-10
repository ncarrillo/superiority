use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use crate::{Error, Result};

use super::model::{
    ImageTableEntry, PresenceField, PresenceFields, PresenceUpdate, ProfileAddress, ToonHandle,
};

pub const FIELD_AVATAR: u32 = 65_555;
pub const FIELD_TOON_PROFILE: u32 = 65_556;
pub const FIELD_TOON_HANDLE: u32 = 65_561;
const FIELD_ACCOUNT_ID: u32 = 0x0001_0005;
const FIELD_SOCIAL_ACCOUNT_ID: u32 = 0x0001_001b;
pub const FIELD_CLAN_TAG: u32 = 0x50004;
pub const FIELD_IN_GAME: u32 = 0x50002;
pub const FIELD_AWAY: u32 = 0x10020;
pub const FIELD_AWAY_FALLBACK: u32 = 0x10010;
pub const FIELD_BUSY: u32 = 0x10022;
pub const FIELD_BUSY_FALLBACK: u32 = 0x10011;
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
