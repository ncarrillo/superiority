use std::collections::BTreeMap;

use crate::{
    Error, Result,
    bsn::{
        bits::BitReader,
        codec::DecodedField,
        value::{BsnStruct, BsnValue},
    },
    metadata::{TypeKind, TypeShape},
    native::model::{
        AccountBlockEntry, AccountBlockPage, CacheStreamItem, CacheStreamItems, ChannelDescriptor,
        ChannelList, ChatInvite, ChatJoin, ChatMembership, ChatMessage, ChatWhisper,
        ClubInviteAction, ClubSummary, ConferenceDescription, ConferenceDescriptions,
        ConferenceMemberCount, ConferenceMemberCounts,
        ConnectionMessageFrame, FriendEntry, FriendIdentity, FriendToon, FriendToonPage,
        FriendUpdate, FriendsPage, MemberClanTag, MembershipChange, MembershipKind,
        PartyMemberStatus, Payload, PresenceField, PresenceFieldFlags, PresenceFields,
        PresenceUpdate, ProfileAddress, ProfileReadResponse, ProfileReadResult, SocialOperation,
        StartupSummary, ToonDisplay, ToonFullName, ToonHandle, ToonList, ToonNameResolved,
        ToonSelected,
    },
    native::protocol::Protocol,
};

pub(crate) struct DecodedPayload {
    pub payload: Payload,
    pub provenance: Vec<DecodedField>,
}

impl DecodedPayload {
    fn new(payload: Payload, provenance: Vec<DecodedField>) -> Self {
        Self {
            payload,
            provenance,
        }
    }
}

fn reflected_payload(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
    project: impl FnOnce(BsnValue) -> Result<Payload>,
) -> Result<DecodedPayload> {
    let decoded = protocol
        .codec()
        .decode_reflected_traced_from(reader, type_id, "value", 0)?;
    Ok(DecodedPayload::new(project(decoded.value)?, decoded.fields))
}

fn traced_reflected_field(
    protocol: &Protocol,
    struct_type: u32,
    field_name: &str,
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
    provenance: &mut Vec<DecodedField>,
) -> Result<BsnValue> {
    let field_type = protocol.member_type(struct_type, field_name)?;
    let decoded = protocol
        .codec()
        .decode_reflected_traced_from(reader, field_type, path, depth)?;
    provenance.extend(decoded.fields);
    Ok(decoded.value)
}

fn custom_payload(
    payload: Payload,
    mut provenance: Vec<DecodedField>,
    kind: &'static str,
    summary: impl Into<String>,
    start_bit: usize,
    end_bit: usize,
) -> DecodedPayload {
    provenance.push(decoded_field("value", kind, summary, start_bit, end_bit, 0));
    sort_provenance(&mut provenance);
    DecodedPayload::new(payload, provenance)
}

pub(crate) fn message_frame(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    Ok(message_frame_with_provenance(protocol, type_id, reader)?.payload)
}

pub(crate) fn message_frame_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let mut provenance = Vec::new();
    let payload = expect_bytes(
        traced_reflected_field(
            protocol,
            type_id,
            "m_payload",
            reader,
            "value.m_payload",
            1,
            &mut provenance,
        )?,
        "message frame payload",
    )?;
    let frame_type = expect_integer(
        traced_reflected_field(
            protocol,
            type_id,
            "m_frameType",
            reader,
            "value.m_frameType",
            1,
            &mut provenance,
        )?,
        "message frame type",
    )?;
    let headers = match traced_reflected_field(
        protocol,
        type_id,
        "m_headers",
        reader,
        "value.m_headers",
        1,
        &mut provenance,
    )? {
        BsnValue::Array(values) => values
            .into_iter()
            .map(|value| match value {
                BsnValue::Struct(value) => Ok(value),
                _ => Err(native_error("message frame header is not a struct")),
            })
            .collect::<Result<Vec<_>>>()?,
        _ => return Err(native_error("message frame headers are not an array")),
    };
    Ok(custom_payload(
        Payload::MessageFrame(ConnectionMessageFrame {
            frame_type,
            headers,
            payload,
        }),
        provenance,
        "MessageFrame",
        "3 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn game_site_info(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let external = protocol.member_type(type_id, "m_externalIp4Addr")?;
    let sites = protocol.member_type(type_id, "m_siteData")?;
    protocol.codec().decode_reflected_from(reader, external)?;
    let site_data = protocol.codec().decode_reflected_from(reader, sites)?;
    let item_count = match site_data {
        BsnValue::Array(values) => values.len(),
        _ => 0,
    };
    Ok(Payload::StartupSummary(StartupSummary {
        kind: "game-site-info",
        item_count,
        complete: None,
    }))
}

pub(crate) fn game_site_info_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let mut provenance = Vec::new();
    traced_reflected_field(
        protocol,
        type_id,
        "m_externalIp4Addr",
        reader,
        "value.m_externalIp4Addr",
        1,
        &mut provenance,
    )?;
    let site_data = traced_reflected_field(
        protocol,
        type_id,
        "m_siteData",
        reader,
        "value.m_siteData",
        1,
        &mut provenance,
    )?;
    let item_count = match site_data {
        BsnValue::Array(values) => values.len(),
        _ => 0,
    };
    Ok(custom_payload(
        Payload::StartupSummary(StartupSummary {
            kind: "game-site-info",
            item_count,
            complete: None,
        }),
        provenance,
        "GameSiteInfo",
        "2 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn profile_settings(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    for name in ["m_type", "m_path", "m_address"] {
        let field_type = protocol.member_type(type_id, name)?;
        protocol.codec().decode_reflected_from(reader, field_type)?;
    }
    Ok(Payload::StartupSummary(StartupSummary {
        kind: "profile-settings",
        item_count: 1,
        complete: None,
    }))
}

pub(crate) fn profile_settings_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let mut provenance = Vec::new();
    for name in ["m_type", "m_path", "m_address"] {
        traced_reflected_field(
            protocol,
            type_id,
            name,
            reader,
            &format!("value.{name}"),
            1,
            &mut provenance,
        )?;
    }
    Ok(custom_payload(
        Payload::StartupSummary(StartupSummary {
            kind: "profile-settings",
            item_count: 1,
            complete: None,
        }),
        provenance,
        "ProfileSettings",
        "3 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn profile_read(
    protocol: &Protocol,
    result_type: u32,
    token_type: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    Ok(profile_read_with_provenance(protocol, result_type, token_type, reader)?.payload)
}

pub(crate) fn profile_read_with_provenance(
    protocol: &Protocol,
    result_type: u32,
    token_type: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let decoded_result =
        protocol
            .codec()
            .decode_reflected_traced_from(reader, result_type, "value.result", 1)?;
    let request =
        protocol
            .codec()
            .decode_reflected_traced_from(reader, token_type, "value.request_id", 1)?;
    let request_id = u32::try_from(expect_integer(request.value, "profile read request id")?)
        .map_err(|_| native_error("profile read request id is outside u32"))?;
    let BsnValue::Choice { index, value } = decoded_result.value else {
        return Err(native_error("profile read result is not a choice"));
    };
    let result = match index {
        0 => {
            let BsnValue::Struct(value) = *value else {
                return Err(native_error("profile read start is not a struct"));
            };
            let packet_count = u32::try_from(expect_integer(
                value
                    .get("m_numPackets")
                    .ok_or_else(|| native_error("profile read start omits packet count"))?
                    .clone(),
                "profile read packet count",
            )?)
            .map_err(|_| native_error("profile read packet count is outside u32"))?;
            let record_type = expect_integer(
                value
                    .get("m_type")
                    .ok_or_else(|| native_error("profile read start omits record type"))?
                    .clone(),
                "profile read record type",
            )?;
            ProfileReadResult::Start {
                packet_count,
                record_type,
            }
        }
        1 => ProfileReadResult::Block(expect_bytes(*value, "profile read block")?),
        2 => ProfileReadResult::Failure(
            u16::try_from(expect_integer(*value, "profile read failure")?)
                .map_err(|_| native_error("profile read failure is outside u16"))?,
        ),
        3 => ProfileReadResult::Cache,
        _ => return Err(native_error("profile read result has an unknown choice")),
    };
    let mut provenance = decoded_result.fields;
    provenance.extend(request.fields);
    Ok(custom_payload(
        Payload::ProfileRead(ProfileReadResponse { request_id, result }),
        provenance,
        "ProfileRead",
        "2 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn toon_name_resolved(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let value = protocol.codec().decode_reflected_from(reader, type_id)?;
    let root = value
        .as_struct()
        .ok_or_else(|| native_error("toon-name response is not a struct"))?;
    let name = toon_full_name_from_value(
        root.get("m_name")
            .ok_or_else(|| native_error("toon-name response omits its name"))?,
    )?;
    let result = u16::try_from(expect_integer(
        root.get("m_result")
            .ok_or_else(|| native_error("toon-name response omits its result"))?
            .clone(),
        "toon-name result",
    )?)
    .map_err(|_| native_error("toon-name result is outside u16"))?;
    let handle = toon_handle_from_value(
        root.get("m_handle")
            .ok_or_else(|| native_error("toon-name response omits its handle field"))?,
    )?;
    Ok(Payload::ToonNameResolved(ToonNameResolved {
        name,
        result,
        handle,
    }))
}

pub(crate) fn toon_name_resolved_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    reflected_payload(protocol, type_id, reader, |value| {
        let root = value
            .as_struct()
            .ok_or_else(|| native_error("toon-name response is not a struct"))?;
        let name = toon_full_name_from_value(
            root.get("m_name")
                .ok_or_else(|| native_error("toon-name response omits its name"))?,
        )?;
        let result = u16::try_from(expect_integer(
            root.get("m_result")
                .ok_or_else(|| native_error("toon-name response omits its result"))?
                .clone(),
            "toon-name result",
        )?)
        .map_err(|_| native_error("toon-name result is outside u16"))?;
        let handle = toon_handle_from_value(
            root.get("m_handle")
                .ok_or_else(|| native_error("toon-name response omits its handle field"))?,
        )?;
        Ok(Payload::ToonNameResolved(ToonNameResolved {
            name,
            result,
            handle,
        }))
    })
}

pub(crate) fn member_clan_tag(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let value = protocol.codec().decode_reflected_from(reader, type_id)?;
    let root = value
        .as_struct()
        .ok_or_else(|| native_error("member-clan-tag response is not a struct"))?;
    let request = root
        .get("GetMemberClanTags")
        .and_then(BsnValue::as_struct)
        .ok_or_else(|| native_error("member-clan-tag response omits its request token"))?;
    let token = expect_u32(
        request
            .get("m_token")
            .ok_or_else(|| native_error("member-clan-tag response omits its token"))?
            .clone(),
        "member-clan-tag token",
    )?;
    let result = u16::try_from(expect_integer(
        root.get("m_result")
            .ok_or_else(|| native_error("member-clan-tag response omits its result"))?
            .clone(),
        "member-clan-tag result",
    )?)
    .map_err(|_| native_error("member-clan-tag result is outside u16"))?;
    let club_id = root
        .get("m_clubId")
        .and_then(unwrap_optional)
        .map(|value| expect_u32(value.clone(), "member clan id"))
        .transpose()?;
    let tag = root
        .get("m_clubTag")
        .and_then(unwrap_optional)
        .and_then(bsn_string)
        .map(str::to_owned)
        .filter(|tag| !tag.is_empty());
    Ok(Payload::MemberClanTag(MemberClanTag {
        token,
        result,
        club_id,
        tag,
    }))
}

pub(crate) fn member_clan_tag_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    reflected_payload(protocol, type_id, reader, |value| {
        let root = value
            .as_struct()
            .ok_or_else(|| native_error("member-clan-tag response is not a struct"))?;
        let request = root
            .get("GetMemberClanTags")
            .and_then(BsnValue::as_struct)
            .ok_or_else(|| native_error("member-clan-tag response omits its request token"))?;
        let token = expect_u32(
            request
                .get("m_token")
                .ok_or_else(|| native_error("member-clan-tag response omits its token"))?
                .clone(),
            "member-clan-tag token",
        )?;
        let result = u16::try_from(expect_integer(
            root.get("m_result")
                .ok_or_else(|| native_error("member-clan-tag response omits its result"))?
                .clone(),
            "member-clan-tag result",
        )?)
        .map_err(|_| native_error("member-clan-tag result is outside u16"))?;
        let club_id = root
            .get("m_clubId")
            .and_then(unwrap_optional)
            .map(|value| expect_u32(value.clone(), "member clan id"))
            .transpose()?;
        let tag = root
            .get("m_clubTag")
            .and_then(unwrap_optional)
            .and_then(bsn_string)
            .map(str::to_owned)
            .filter(|tag| !tag.is_empty());
        Ok(Payload::MemberClanTag(MemberClanTag {
            token,
            result,
            club_id,
            tag,
        }))
    })
}

pub(crate) fn toon_list(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    Ok(toon_list_with_provenance(protocol, type_id, reader)?.payload)
}

pub(crate) fn toon_list_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let displays_field = protocol.member_type(type_id, "m_toonDisplays")?;
    let display_type = protocol.array_element(displays_field)?;
    let profile_type = protocol.member_type(display_type, "m_profile")?;
    let realm_type = protocol.member_type(display_type, "m_realm")?;
    let payload_start = reader.position();
    let count = read_spanned_usize(reader, 6)?;
    if count.value > 50 {
        return Err(native_error("native ToonList contains too many displays"));
    }
    let mut displays = Vec::with_capacity(count.value);
    let mut provenance = vec![decoded_field(
        "value.displays.count",
        "array length",
        count.value.to_string(),
        count.start_bit,
        count.end_bit,
        2,
    )];
    for index in 0..count.value {
        let path = format!("value.displays[{index}]");
        let display_start = reader.position();
        let name = decode_spanned_generated_utf8(reader, 7, 2, 100, 25)?;
        let last_online = read_spanned_i32(reader)?;
        let wire_layout = read_spanned_u64(reader, 3)?;
        let flags = read_spanned_u32(reader, 32)?;
        let profile = protocol.codec().decode_reflected_traced_from(
            reader,
            profile_type,
            &format!("{path}.profile"),
            3,
        )?;
        let realm = protocol.codec().decode_reflected_traced_from(
            reader,
            realm_type,
            &format!("{path}.realm"),
            3,
        )?;
        let realm_value = u32::try_from(expect_integer(realm.value, "toon realm")?)
            .map_err(|_| native_error("toon realm is outside u32"))?;
        let display_end = reader.position();
        displays.push(ToonDisplay {
            name: name.value.clone(),
            realm: realm_value,
        });
        provenance.extend([
            decoded_field(
                path.clone(),
                "Toon.Display",
                "5 fields",
                display_start,
                display_end,
                2,
            ),
            spanned_field(format!("{path}.name"), "string", &name, 3),
            spanned_field(format!("{path}.last_online"), "int32", &last_online, 3),
            spanned_field(
                format!("{path}.wire_layout_selector"),
                "obfuscation selector",
                &wire_layout,
                3,
            ),
            spanned_field(format!("{path}.flags"), "uint32", &flags, 3),
        ]);
        provenance.extend(profile.fields);
        provenance.extend(realm.fields);
    }
    let payload_end = reader.position();
    provenance.push(decoded_field(
        "value.displays",
        "array",
        format!("{} items", displays.len()),
        count.start_bit,
        payload_end,
        1,
    ));
    provenance.push(decoded_field(
        "value",
        "ToonList",
        "1 field",
        payload_start,
        payload_end,
        0,
    ));
    sort_provenance(&mut provenance);
    Ok(DecodedPayload::new(
        Payload::ToonList(ToonList { displays }),
        provenance,
    ))
}

pub(crate) fn toon_selected(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    Ok(toon_selected_with_provenance(protocol, type_id, reader)?.payload)
}

pub(crate) fn toon_selected_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let record_address = protocol.member_type(type_id, "m_recordAddress")?;
    let handle_type = protocol.member_type(type_id, "m_toonHandle")?;
    let realm_type = protocol.member_type(type_id, "m_realm")?;
    let last_logon_type = protocol.member_type(type_id, "m_lastLogon")?;
    let start_bit = reader.position();
    let record = protocol.codec().decode_reflected_traced_from(
        reader,
        record_address,
        "value.record_address",
        1,
    )?;
    let mut fields = [0_u64; 4];
    let handle_start = reader.position();
    let mut provenance = record.fields;
    for (slot, name) in fields
        .iter_mut()
        .zip(["m_programId", "m_region", "m_realm", "m_id"])
    {
        let field_type = protocol.member_type(handle_type, name)?;
        let decoded = protocol.codec().decode_reflected_traced_from(
            reader,
            field_type,
            &format!("value.handle.{name}"),
            2,
        )?;
        provenance.extend(decoded.fields);
        *slot = match decoded.value {
            BsnValue::FourCc(code) => u64::from(code),
            other => u64::try_from(expect_integer(other, "selected toon handle")?)
                .map_err(|_| native_error("selected toon handle field is outside u64"))?,
        };
    }
    provenance.push(decoded_field(
        "value.handle",
        "ToonHandle",
        "4 fields",
        handle_start,
        reader.position(),
        1,
    ));
    let handle = ToonHandle {
        program_id: u32::try_from(fields[0])
            .map_err(|_| native_error("selected toon program id is outside u32"))?,
        region: u8::try_from(fields[1])
            .map_err(|_| native_error("selected toon region is outside u8"))?,
        realm: u32::try_from(fields[2])
            .map_err(|_| native_error("selected toon realm is outside u32"))?,
        id: fields[3],
    };
    let realm_decoded =
        protocol
            .codec()
            .decode_reflected_traced_from(reader, realm_type, "value.realm", 1)?;
    let realm = u32::try_from(expect_integer(realm_decoded.value, "selected toon realm")?)
        .map_err(|_| native_error("selected toon realm is outside u32"))?;
    provenance.extend(realm_decoded.fields);
    let last_logon = protocol.codec().decode_reflected_traced_from(
        reader,
        last_logon_type,
        "value.last_logon",
        1,
    )?;
    provenance.extend(last_logon.fields);
    let name = decode_spanned_generated_utf8(reader, 7, 2, 100, 25)?;
    provenance.push(spanned_field("value.name", "string", &name, 1));
    Ok(custom_payload(
        Payload::ToonSelected(ToonSelected {
            name: name.value,
            realm,
            handle,
        }),
        provenance,
        "ToonSelected",
        "5 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn toon_welcome(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    Ok(toon_welcome_with_provenance(protocol, type_id, reader)?.payload)
}

#[expect(
    clippy::too_many_lines,
    reason = "the retail welcome wire order is explicit"
)]
pub(crate) fn toon_welcome_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let mut provenance = Vec::new();
    traced_reflected_field(
        protocol,
        type_id,
        "m_depotRegion",
        reader,
        "value.depot_region",
        1,
        &mut provenance,
    )?;

    let achievements = protocol.member_type(type_id, "m_achievementHandles")?;
    let achievement_type = protocol.array_element(achievements)?;
    let achievement_count = read_spanned_usize(reader, 4)?;
    provenance.push(spanned_field(
        "value.achievements.count",
        "array length",
        &achievement_count,
        2,
    ));
    for index in 0..achievement_count.value {
        let path = format!("value.achievements[{index}]");
        let item_start = reader.position();
        for name in ["m_handle", "m_program"] {
            traced_reflected_field(
                protocol,
                achievement_type,
                name,
                reader,
                &format!("{path}.{name}"),
                3,
                &mut provenance,
            )?;
        }
        provenance.push(decoded_field(
            path,
            "AchievementHandle",
            "2 fields",
            item_start,
            reader.position(),
            2,
        ));
    }
    provenance.push(decoded_field(
        "value.achievements",
        "array",
        format!("{} items", achievement_count.value),
        achievement_count.start_bit,
        reader.position(),
        1,
    ));
    for (name, path) in [
        ("m_isPlayingFromIGR", "value.is_playing_from_igr"),
        ("m_defaultPortrait", "value.default_portrait"),
    ] {
        traced_reflected_field(protocol, type_id, name, reader, path, 1, &mut provenance)?;
    }
    let portrait_selector = read_spanned_u64(reader, 31)?;
    provenance.push(spanned_field(
        "value.portrait_wire_selector",
        "obfuscation selector",
        &portrait_selector,
        1,
    ));
    for name in [
        "m_maxGameServerConnectTimeoutMS",
        "m_programName",
        "m_programFlags",
    ] {
        traced_reflected_field(
            protocol,
            type_id,
            name,
            reader,
            &format!("value.{name}"),
            1,
            &mut provenance,
        )?;
    }

    let realms = protocol.member_type(type_id, "m_realmMapList")?;
    let realm_type = protocol.array_element(realms)?;
    let handle_type = protocol.member_type(realm_type, "m_to")?;
    let from_list = protocol.member_type(realm_type, "m_fromList")?;
    let from_type = protocol.array_element(from_list)?;
    let realm_count = read_spanned_usize(reader, 3)?;
    if realm_count.value > 4 {
        return Err(native_error("Welcome has too many realm mappings"));
    }
    provenance.push(spanned_field(
        "value.realm_map.count",
        "array length",
        &realm_count,
        2,
    ));
    for index in 0..realm_count.value {
        let path = format!("value.realm_map[{index}]");
        let item_start = reader.position();
        for name in ["m_id", "m_programId"] {
            traced_reflected_field(
                protocol,
                handle_type,
                name,
                reader,
                &format!("{path}.to.{name}"),
                3,
                &mut provenance,
            )?;
        }
        let handle_flag = read_spanned_u64(reader, 1)?;
        provenance.push(spanned_field(
            format!("{path}.to.flag"),
            "wire flag",
            &handle_flag,
            3,
        ));
        traced_reflected_field(
            protocol,
            handle_type,
            "m_region",
            reader,
            &format!("{path}.to.m_region"),
            3,
            &mut provenance,
        )?;
        let from_count = read_spanned_usize(reader, 5)?;
        if from_count.value > 16 {
            return Err(native_error("Welcome realm mapping is too large"));
        }
        provenance.push(spanned_field(
            format!("{path}.from.count"),
            "array length",
            &from_count,
            4,
        ));
        for from_index in 0..from_count.value {
            let decoded = protocol.codec().decode_reflected_traced_from(
                reader,
                from_type,
                &format!("{path}.from[{from_index}]"),
                4,
            )?;
            provenance.extend(decoded.fields);
        }
        provenance.push(decoded_field(
            path,
            "RealmMap",
            "2 fields",
            item_start,
            reader.position(),
            2,
        ));
    }
    provenance.push(decoded_field(
        "value.realm_map",
        "array",
        format!("{} items", realm_count.value),
        realm_count.start_bit,
        reader.position(),
        1,
    ));

    let unlockables = protocol.member_type(type_id, "m_unlockablesFiles")?;
    let unlockable_type = protocol.array_element(unlockables)?;
    let handle = protocol.member_type(unlockable_type, "m_unlockDefinitionFileCacheHandle")?;
    let unlockable_count = read_spanned_usize(reader, 8)?;
    if unlockable_count.value > 128 {
        return Err(native_error("Welcome has too many unlockable files"));
    }
    provenance.push(spanned_field(
        "value.unlockables.count",
        "array length",
        &unlockable_count,
        2,
    ));
    for index in 0..unlockable_count.value {
        let path = format!("value.unlockables[{index}]");
        let item_start = reader.position();
        let selector = read_spanned_u64(reader, 4)?;
        provenance.push(spanned_field(
            format!("{path}.wire_layout_selector"),
            "obfuscation selector",
            &selector,
            3,
        ));
        let decoded = protocol.codec().decode_reflected_traced_from(
            reader,
            handle,
            &format!("{path}.handle"),
            3,
        )?;
        provenance.extend(decoded.fields);
        provenance.push(decoded_field(
            path,
            "UnlockableFile",
            "2 fields",
            item_start,
            reader.position(),
            2,
        ));
    }
    provenance.push(decoded_field(
        "value.unlockables",
        "array",
        format!("{} items", unlockable_count.value),
        unlockable_count.start_bit,
        reader.position(),
        1,
    ));
    let trailing_selector = read_spanned_u64(reader, 3)?;
    provenance.push(spanned_field(
        "value.trailing_wire_selector",
        "obfuscation selector",
        &trailing_selector,
        1,
    ));
    traced_reflected_field(
        protocol,
        type_id,
        "m_maxMapFavorites",
        reader,
        "value.max_map_favorites",
        1,
        &mut provenance,
    )?;
    let intermediate = decode_spanned_generated_utf8(reader, 13, 0, 4096, 1024)?;
    let final_restriction = decode_spanned_generated_utf8(reader, 13, 0, 4096, 1024)?;
    provenance.extend([
        spanned_field(
            "value.intermediate_name_restriction",
            "string",
            &intermediate,
            1,
        ),
        spanned_field(
            "value.final_name_restriction",
            "string",
            &final_restriction,
            1,
        ),
    ]);
    traced_reflected_field(
        protocol,
        type_id,
        "m_currentTime",
        reader,
        "value.current_time",
        1,
        &mut provenance,
    )?;
    Ok(custom_payload(
        Payload::StartupSummary(StartupSummary {
            kind: "toon-welcome",
            item_count: achievement_count.value + realm_count.value + unlockable_count.value,
            complete: None,
        }),
        provenance,
        "ToonWelcome",
        "13 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn chat_message(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    Ok(chat_message_with_provenance(protocol, type_id, reader)?.payload)
}

pub(crate) fn chat_message_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let mut provenance = Vec::new();
    traced_reflected_field(
        protocol,
        type_id,
        "Message",
        reader,
        "value.message",
        1,
        &mut provenance,
    )?;
    let member_handle = u32::try_from(expect_integer(
        traced_reflected_field(
            protocol,
            type_id,
            "m_memberHandle",
            reader,
            "value.member_handle",
            1,
            &mut provenance,
        )?,
        "chat member handle",
    )?)
    .map_err(|_| native_error("chat member handle is outside u32"))?;
    let body = decode_spanned_generated_utf8(reader, 10, 0, 1020, 255)?;
    provenance.push(spanned_field("value.m_body", "string", &body, 1));
    let channel_index = u8::try_from(expect_integer(
        traced_reflected_field(
            protocol,
            type_id,
            "m_channelIndex",
            reader,
            "value.channel_index",
            1,
            &mut provenance,
        )?,
        "chat channel index",
    )?)
    .map_err(|_| native_error("chat channel index is outside u8"))?;
    Ok(custom_payload(
        Payload::ChatMessage(ChatMessage {
            channel_index,
            member_handle,
            body: body.value,
        }),
        provenance,
        "ChatMessage",
        "4 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn chat_invite(reader: &mut BitReader<'_>) -> Result<Payload> {
    let channel_type = read_u8(reader, 4)?;
    let inviter_presence = read_u32(reader, 32)?;
    let channel_index = read_u8(reader, 3)?;
    Ok(Payload::ChatInvite(ChatInvite {
        channel_type,
        channel_index,
        inviter_presence,
    }))
}

pub(crate) fn chat_invite_with_provenance(reader: &mut BitReader<'_>) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let channel_type = read_spanned_u8(reader, 4)?;
    let inviter_presence = read_spanned_u32(reader, 32)?;
    let channel_index = read_spanned_u8(reader, 3)?;
    Ok(custom_payload(
        Payload::ChatInvite(ChatInvite {
            channel_type: channel_type.value,
            channel_index: channel_index.value,
            inviter_presence: inviter_presence.value,
        }),
        vec![
            spanned_field("value.m_channelType", "ChatChannelType2", &channel_type, 1),
            spanned_field("value.m_inviterPresence", "uint32", &inviter_presence, 1),
            spanned_field("value.m_channelIndex", "uint3", &channel_index, 1),
        ],
        "ChatInvite",
        "3 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn chat_whisper(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
    echo: bool,
) -> Result<Payload> {
    Ok(chat_whisper_with_provenance(protocol, type_id, reader, echo)?.payload)
}

pub(crate) fn chat_whisper_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
    echo: bool,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let mut provenance = Vec::new();
    traced_reflected_field(
        protocol,
        type_id,
        "Whisper",
        reader,
        "value.whisper",
        1,
        &mut provenance,
    )?;
    let peer_start = reader.position();
    let peer = trace_chat_display_name(reader, "value.peer", 2, &mut provenance)?;
    provenance.push(decoded_field(
        "value.peer",
        "ToonFullName",
        "4 fields",
        peer_start,
        reader.position(),
        1,
    ));
    let body = decode_spanned_generated_utf8(reader, 10, 0, 1020, 255)?;
    provenance.push(spanned_field("value.body", "string", &body, 1));
    let whisper = ChatWhisper {
        peer,
        body: body.value,
    };
    Ok(custom_payload(
        if echo {
            Payload::ChatWhisperEcho(whisper)
        } else {
            Payload::ChatWhisper(whisper)
        },
        provenance,
        if echo { "WhisperEcho" } else { "WhisperRecv" },
        "3 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn chat_membership(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    Ok(chat_membership_with_provenance(protocol, type_id, reader)?.payload)
}

#[expect(
    clippy::too_many_lines,
    reason = "one sequential step per field in the recovered wire layout"
)]
pub(crate) fn chat_membership_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let end_of_initial = read_spanned_u64(reader, 1)?;
    let channel_index = read_spanned_u8(reader, 3)?;
    if usize::from(channel_index.value) >= crate::native::protocol::CHANNEL_INDEX_COUNT {
        return Err(native_error("chat membership channel index is invalid"));
    }
    let change_array = protocol.member_type(type_id, "m_changes")?;
    let change_choice = protocol.peel_alias(protocol.array_element(change_array)?)?;
    let change_shape = protocol.codec().schema().shape(change_choice)?;
    if change_shape.kind != TypeKind::Choice {
        return Err(native_error("chat membership change type is not a choice"));
    }
    let join_type = choice_variant(&change_shape, 1)?;
    let statuses = protocol.member_type(join_type, "m_memberStatus")?;
    let status_choice = protocol.peel_alias(protocol.array_element(statuses)?)?;
    let status_shape = protocol.codec().schema().shape(status_choice)?;
    if status_shape.kind != TypeKind::Choice {
        return Err(native_error("chat member status type is not a choice"));
    }

    let count = read_spanned_usize(reader, 6)?;
    let change_count = count.value + 1;
    let mut provenance = vec![
        spanned_field("value.end_of_initial", "bool", &end_of_initial, 1),
        spanned_field("value.channel_index", "uint3", &channel_index, 1),
        decoded_field(
            "value.changes.count",
            "array length minus one",
            change_count.to_string(),
            count.start_bit,
            count.end_bit,
            2,
        ),
    ];
    let mut changes = Vec::with_capacity(change_count);
    for index in 0..change_count {
        let path = format!("value.changes[{index}]");
        let item_start = reader.position();
        let choice = read_spanned_u64(reader, 2)?;
        provenance.push(spanned_field(
            format!("{path}.kind"),
            "choice selector",
            &choice,
            3,
        ));
        match choice.value {
            0 => {
                let member_handle = read_spanned_u32(reader, 32)?;
                let reason = read_spanned_u64(reader, 16)?;
                provenance.extend([
                    spanned_field(format!("{path}.member_handle"), "uint32", &member_handle, 3),
                    spanned_field(format!("{path}.reason"), "uint16", &reason, 3),
                ]);
                changes.push(MembershipChange {
                    kind: MembershipKind::Leave,
                    member_handle: member_handle.value,
                    presence_id: None,
                    display_name: None,
                    toon_name: None,
                    party_status: None,
                    reason: Some(u16::try_from(reason.value).expect("16-bit field fits in u16")),
                });
            }
            1 => {
                let member_handle = read_spanned_u32(reader, 32)?;
                let presence_id = read_spanned_u32(reader, 32)?;
                let status_count = read_spanned_usize(reader, 3)?;
                provenance.extend([
                    spanned_field(format!("{path}.member_handle"), "uint32", &member_handle, 3),
                    spanned_field(format!("{path}.presence_id"), "uint32", &presence_id, 3),
                    spanned_field(
                        format!("{path}.statuses.count"),
                        "array length",
                        &status_count,
                        4,
                    ),
                ]);
                let mut display_name = None;
                let mut toon_name = None;
                let mut party_status = None;
                for status_index in 0..status_count.value {
                    let status = trace_member_status(
                        protocol,
                        &status_shape,
                        reader,
                        &format!("{path}.statuses[{status_index}]"),
                        4,
                        &mut provenance,
                    )?;
                    if let Some(name) = status.toon_name {
                        display_name = Some(name.name.clone());
                        toon_name = Some(name);
                    }
                    party_status = status.party_status.or(party_status);
                }
                changes.push(MembershipChange {
                    kind: MembershipKind::Join,
                    member_handle: member_handle.value,
                    presence_id: Some(presence_id.value),
                    display_name,
                    toon_name,
                    party_status,
                    reason: None,
                });
            }
            2 => {
                let member_handle = read_spanned_u32(reader, 32)?;
                provenance.push(spanned_field(
                    format!("{path}.member_handle"),
                    "uint32",
                    &member_handle,
                    3,
                ));
                let status = trace_member_status(
                    protocol,
                    &status_shape,
                    reader,
                    &format!("{path}.status"),
                    3,
                    &mut provenance,
                )?;
                changes.push(MembershipChange {
                    kind: MembershipKind::Status,
                    member_handle: member_handle.value,
                    presence_id: None,
                    display_name: status.toon_name.as_ref().map(|name| name.name.clone()),
                    toon_name: status.toon_name,
                    party_status: status.party_status,
                    reason: None,
                });
            }
            _ => return Err(native_error("chat membership change has an unknown choice")),
        }
        provenance.push(decoded_field(
            path,
            "MembershipChange",
            match choice.value {
                0 => "Leave",
                1 => "Join",
                2 => "Status",
                _ => unreachable!(),
            },
            item_start,
            reader.position(),
            2,
        ));
    }
    provenance.push(decoded_field(
        "value.changes",
        "array",
        format!("{} items", changes.len()),
        count.start_bit,
        reader.position(),
        1,
    ));
    Ok(custom_payload(
        Payload::ChatMembership(ChatMembership {
            channel_index: channel_index.value,
            end_of_initial: end_of_initial.value != 0,
            changes,
        }),
        provenance,
        "ChatMembership",
        "3 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn friends_list(protocol: &Protocol, reader: &mut BitReader<'_>) -> Result<Payload> {
    Ok(friends_list_with_provenance(protocol, reader)?.payload)
}

#[expect(
    clippy::too_many_lines,
    reason = "one sequential step per field in the recovered wire layout"
)]
pub(crate) fn friends_list_with_provenance(
    protocol: &Protocol,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let complete_start = reader.position();
    let complete = if reader.read(1)? == 0 {
        None
    } else {
        Some(reader.read(1)? != 0)
    };
    let complete_end = reader.position();
    let count = read_spanned_usize(reader, 7)?;
    if count.value > 64 {
        return Err(native_error("friends snapshot has too many updates"));
    }
    let mut provenance = vec![
        decoded_field(
            "value.complete",
            "optional bool",
            complete.map_or_else(|| "none".to_owned(), |value| value.to_string()),
            complete_start,
            complete_end,
            1,
        ),
        spanned_field("value.updates.count", "array length", &count, 2),
    ];
    let mut updates = Vec::with_capacity(count.value);
    for index in 0..count.value {
        let path = format!("value.updates[{index}]");
        let update_start = reader.position();
        let operation_start = reader.position();
        let operation_index = reader.read(2)?;
        let operation_end = reader.position();
        let operation = match operation_index {
            0 => SocialOperation::Add,
            1 => {
                let identity_start = reader.position();
                let identity = if reader.read(1)? == 0 {
                    FriendIdentity::Account(read_u32(reader, 32)?)
                } else {
                    trace_toon_handle(
                        protocol,
                        reader,
                        &format!("{path}.entry.identity"),
                        3,
                        &mut provenance,
                    )?
                };
                let identity_end = reader.position();
                provenance.extend([
                    decoded_field(
                        format!("{path}.operation"),
                        "choice selector",
                        "Remove",
                        operation_start,
                        operation_end,
                        2,
                    ),
                    decoded_field(
                        format!("{path}.entry.identity"),
                        "identity",
                        format!("{identity:?}"),
                        identity_start,
                        identity_end,
                        3,
                    ),
                    decoded_field(
                        format!("{path}.entry"),
                        "FriendEntry",
                        "identity only",
                        identity_start,
                        identity_end,
                        2,
                    ),
                    decoded_field(
                        path,
                        "FriendUpdate",
                        "Remove",
                        update_start,
                        identity_end,
                        1,
                    ),
                ]);
                updates.push(FriendUpdate {
                    operation: SocialOperation::Remove,
                    entry: FriendEntry {
                        identity,
                        display_name: None,
                        full_name: None,
                        note: None,
                        profile: None,
                        toon_name: None,
                    },
                });
                continue;
            }
            2 => SocialOperation::Modify,
            _ => {
                return Err(native_error(
                    "friends snapshot has an unknown update choice",
                ));
            }
        };
        provenance.push(decoded_field(
            format!("{path}.operation"),
            "choice selector",
            format!("{operation:?}"),
            operation_start,
            operation_end,
            2,
        ));
        let entry = trace_friend_container(
            protocol,
            reader,
            &format!("{path}.entry"),
            2,
            &mut provenance,
        )?;
        let entry_end = reader.position();
        provenance.push(decoded_field(
            path,
            "FriendUpdate",
            format!("{operation:?}"),
            update_start,
            entry_end,
            1,
        ));
        updates.push(FriendUpdate { operation, entry });
    }
    provenance.push(decoded_field(
        "value.updates",
        "array",
        format!("{} items", updates.len()),
        count.start_bit,
        reader.position(),
        1,
    ));
    Ok(custom_payload(
        Payload::Friends(FriendsPage { updates, complete }),
        provenance,
        "FriendsPage",
        "2 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn friend_toons(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    Ok(friend_toons_with_provenance(protocol, type_id, reader)?.payload)
}

pub(crate) fn friend_toons_with_provenance(
    _protocol: &Protocol,
    _type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let payload_start = reader.position();
    let count_start = reader.position();
    let count = read_usize(reader, 7)?;
    if count > 100 {
        return Err(native_error(
            "friend toon notification has too many entries",
        ));
    }
    let mut traces = Vec::with_capacity(count);
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let entry_start = reader.position();
        let region = read_spanned_u8(reader, 8)?;
        let program_id = read_spanned_u32(reader, 32)?;
        let realm = read_spanned_u32(reader, 32)?;
        let name = decode_spanned_generated_utf8(reader, 7, 2, 100, 25)?;
        let profile_start = reader.position();
        let profile = ProfileAddressTrace {
            label: read_spanned_u32(reader, 32)?,
            id: read_spanned_u64(reader, 64)?,
        };
        let profile_end = reader.position();
        let account_id = read_spanned_u32(reader, 32)?;
        let entry_end = reader.position();
        entries.push(FriendToon {
            account_id: account_id.value,
            program_id: program_id.value,
            profile: (profile.label.value != 0 || profile.id.value != 0).then_some(
                ProfileAddress {
                    label: profile.label.value,
                    id: profile.id.value,
                },
            ),
            toon_name: ToonFullName {
                region: region.value,
                program_id: program_id.value,
                realm: realm.value,
                name: name.value.clone(),
            },
        });
        traces.push(FriendToonTrace {
            entry_start,
            entry_end,
            region,
            program_id,
            realm,
            name,
            profile,
            profile_start,
            profile_end,
            account_id,
        });
    }
    let entries_end = reader.position();
    let complete = read_spanned_u64(reader, 1)?;
    let payload = Payload::FriendToons(FriendToonPage {
        entries,
        complete: complete.value != 0,
    });
    let provenance = friend_toon_provenance(
        &traces,
        count_start,
        entries_end,
        &complete,
        payload_start,
        reader.position(),
    );
    Ok(DecodedPayload::new(payload, provenance))
}

#[derive(Clone, Debug)]
struct SpannedValue<T> {
    value: T,
    start_bit: usize,
    end_bit: usize,
}

#[derive(Clone, Debug)]
struct FriendToonTrace {
    entry_start: usize,
    entry_end: usize,
    region: SpannedValue<u8>,
    program_id: SpannedValue<u32>,
    realm: SpannedValue<u32>,
    name: SpannedValue<String>,
    profile: ProfileAddressTrace,
    profile_start: usize,
    profile_end: usize,
    account_id: SpannedValue<u32>,
}

#[derive(Clone, Debug)]
struct ProfileAddressTrace {
    label: SpannedValue<u32>,
    id: SpannedValue<u64>,
}

fn friend_toon_provenance(
    entries: &[FriendToonTrace],
    count_start: usize,
    entries_end: usize,
    complete: &SpannedValue<u64>,
    payload_start: usize,
    payload_end: usize,
) -> Vec<DecodedField> {
    let mut fields = vec![decoded_field(
        "value",
        "FriendToons",
        format!("{} entries", entries.len()),
        payload_start,
        payload_end,
        0,
    )];
    fields.push(decoded_field(
        "value.entries",
        "array",
        format!("{} items", entries.len()),
        count_start,
        entries_end,
        1,
    ));
    for (index, entry) in entries.iter().enumerate() {
        let path = format!("value.entries[{index}]");
        fields.extend(friend_toon_entry_fields(&path, entry));
    }
    fields.push(decoded_field(
        "value.complete",
        "bool",
        (complete.value != 0).to_string(),
        complete.start_bit,
        complete.end_bit,
        1,
    ));
    fields
}

fn friend_toon_entry_fields(path: &str, entry: &FriendToonTrace) -> Vec<DecodedField> {
    let profile_path = format!("{path}.profile");
    let toon_path = format!("{path}.toon_name");
    vec![
        decoded_field(
            path,
            "object",
            "4 fields",
            entry.entry_start,
            entry.entry_end,
            2,
        ),
        spanned_field(format!("{path}.account_id"), "uint32", &entry.account_id, 3),
        spanned_field(format!("{path}.program_id"), "fourcc", &entry.program_id, 3),
        decoded_field(
            profile_path.clone(),
            "object",
            "2 fields",
            entry.profile_start,
            entry.profile_end,
            3,
        ),
        spanned_field(
            format!("{profile_path}.label"),
            "uint32",
            &entry.profile.label,
            4,
        ),
        spanned_field(format!("{profile_path}.id"), "uint64", &entry.profile.id, 4),
        decoded_field(
            toon_path.clone(),
            "object",
            "4 fields",
            entry.region.start_bit,
            entry.name.end_bit,
            3,
        ),
        spanned_field(format!("{toon_path}.region"), "uint8", &entry.region, 4),
        spanned_field(
            format!("{toon_path}.program_id"),
            "fourcc",
            &entry.program_id,
            4,
        ),
        spanned_field(format!("{toon_path}.realm"), "uint32", &entry.realm, 4),
        spanned_field(format!("{toon_path}.name"), "string", &entry.name, 4),
    ]
}

fn decoded_field(
    path: impl Into<String>,
    kind: &'static str,
    value: impl Into<String>,
    start_bit: usize,
    end_bit: usize,
    depth: usize,
) -> DecodedField {
    DecodedField {
        path: path.into(),
        kind,
        value: value.into(),
        start_bit,
        end_bit,
        depth,
    }
}

fn spanned_field<T: ToString>(
    path: impl Into<String>,
    kind: &'static str,
    value: &SpannedValue<T>,
    depth: usize,
) -> DecodedField {
    decoded_field(
        path,
        kind,
        value.value.to_string(),
        value.start_bit,
        value.end_bit,
        depth,
    )
}

fn read_spanned_u64(reader: &mut BitReader<'_>, width: usize) -> Result<SpannedValue<u64>> {
    let start_bit = reader.position();
    let value = reader.read(width)?;
    Ok(SpannedValue {
        value,
        start_bit,
        end_bit: reader.position(),
    })
}

fn read_spanned_usize(reader: &mut BitReader<'_>, width: usize) -> Result<SpannedValue<usize>> {
    let value = read_spanned_u64(reader, width)?;
    Ok(SpannedValue {
        value: usize::try_from(value.value).map_err(|_| native_error("integer exceeds usize"))?,
        start_bit: value.start_bit,
        end_bit: value.end_bit,
    })
}

fn read_spanned_u32(reader: &mut BitReader<'_>, width: usize) -> Result<SpannedValue<u32>> {
    let value = read_spanned_u64(reader, width)?;
    Ok(SpannedValue {
        value: u32::try_from(value.value).map_err(|_| native_error("integer exceeds u32"))?,
        start_bit: value.start_bit,
        end_bit: value.end_bit,
    })
}

fn read_spanned_u8(reader: &mut BitReader<'_>, width: usize) -> Result<SpannedValue<u8>> {
    let value = read_spanned_u64(reader, width)?;
    Ok(SpannedValue {
        value: u8::try_from(value.value).map_err(|_| native_error("integer exceeds u8"))?,
        start_bit: value.start_bit,
        end_bit: value.end_bit,
    })
}

fn read_spanned_i32(reader: &mut BitReader<'_>) -> Result<SpannedValue<i32>> {
    let value = read_spanned_u32(reader, 32)?;
    Ok(SpannedValue {
        value: i32::from_ne_bytes((value.value ^ 0x8000_0000).to_ne_bytes()),
        start_bit: value.start_bit,
        end_bit: value.end_bit,
    })
}

fn sort_provenance(fields: &mut [DecodedField]) {
    fields.sort_by(|left, right| {
        left.start_bit
            .cmp(&right.start_bit)
            .then_with(|| left.depth.cmp(&right.depth))
            .then_with(|| right.end_bit.cmp(&left.end_bit))
    });
}

fn decode_spanned_generated_utf8(
    reader: &mut BitReader<'_>,
    length_bits: usize,
    minimum_bytes: usize,
    maximum_bytes: usize,
    maximum_characters: usize,
) -> Result<SpannedValue<String>> {
    let start_bit = reader.position();
    let value = decode_generated_utf8(
        reader,
        length_bits,
        minimum_bytes,
        maximum_bytes,
        maximum_characters,
    )?;
    Ok(SpannedValue {
        value,
        start_bit,
        end_bit: reader.position(),
    })
}

pub(crate) fn profile_address_query(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    Ok(profile_address_query_with_provenance(protocol, type_id, reader)?.payload)
}

pub(crate) fn profile_address_query_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let decoded = protocol
        .codec()
        .decode_reflected_traced_from(reader, type_id, "value", 0)?;
    let value = decoded.value;
    let BsnValue::Struct(root) = value else {
        return Err(native_error(
            "profile address query response is not a struct",
        ));
    };
    let request_id = expect_u32(
        root.get("m_requestId")
            .cloned()
            .ok_or_else(|| native_error("profile address query omitted its request id"))?,
        "profile address query request id",
    )?;
    let result = unwrap_optional(
        root.get("m_result")
            .ok_or_else(|| native_error("profile address query omitted its result"))?,
    )
    .ok_or_else(|| native_error("profile address query result is absent"))?;
    let BsnValue::Choice { index, value } = result else {
        return Err(native_error("profile address query result is not a choice"));
    };
    let (address, error) = match *index {
        0 => {
            let BsnValue::Struct(success) = unwrap_optional(value)
                .ok_or_else(|| native_error("profile address query success result is absent"))?
            else {
                return Err(native_error(
                    "profile address query success result is not a struct",
                ));
            };
            let address = profile_address_from_value(
                success
                    .get("m_address")
                    .ok_or_else(|| native_error("profile address query omitted its address"))?,
            )?;
            (address, None)
        }
        1 => {
            let BsnValue::Struct(failure) = unwrap_optional(value)
                .ok_or_else(|| native_error("profile address query failure result is absent"))?
            else {
                return Err(native_error(
                    "profile address query failure result is not a struct",
                ));
            };
            let error = expect_u32(
                failure
                    .get("m_error")
                    .cloned()
                    .ok_or_else(|| native_error("profile address query omitted its error"))?,
                "profile address query error",
            )?;
            (None, Some(error))
        }
        index => {
            return Err(native_error(format!(
                "profile address query returned unknown result {index}"
            )));
        }
    };
    Ok(DecodedPayload::new(
        Payload::ProfileAddressQuery(crate::native::model::ProfileAddressQueryResponse {
            request_id,
            address,
            error,
        }),
        decoded.fields,
    ))
}

pub(crate) fn account_blocks(protocol: &Protocol, reader: &mut BitReader<'_>) -> Result<Payload> {
    Ok(account_blocks_with_provenance(protocol, reader)?.payload)
}

#[expect(
    clippy::too_many_lines,
    reason = "one sequential step per field in the recovered wire layout"
)]
pub(crate) fn account_blocks_with_provenance(
    protocol: &Protocol,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let complete_start = reader.position();
    let complete = if reader.read(1)? == 0 {
        None
    } else {
        Some(reader.read(1)? != 0)
    };
    let complete_end = reader.position();
    let count = read_spanned_usize(reader, 7)?;
    if count.value > 64 {
        return Err(native_error("account block snapshot is too large"));
    }
    let account_type = protocol
        .codec()
        .schema()
        .unique_type_id("Battlenet::Friends::AccountBlockContainer")?;
    let mut provenance = vec![
        decoded_field(
            "value.complete",
            "optional bool",
            complete.map_or_else(|| "none".to_owned(), |value| value.to_string()),
            complete_start,
            complete_end,
            1,
        ),
        spanned_field("value.entries.count", "array length", &count, 2),
    ];
    let mut entries = Vec::with_capacity(count.value);
    for index in 0..count.value {
        let path = format!("value.entries[{index}]");
        let item_start = reader.position();
        let selector = read_spanned_u64(reader, 9)?;
        let account_id = read_spanned_u32(reader, 32)?;
        let nickname_start = reader.position();
        let nickname = (reader.read(1)? != 0)
            .then(|| decode_generated_utf8(reader, 7, 0, 108, 27))
            .transpose()?;
        let nickname_end = reader.position();
        let reserved = read_spanned_u64(reader, 20)?;
        let full_name = protocol.member_type(account_type, "m_fullName")?;
        let full_name_start = reader.position();
        let full_name = if reader.read(1)? != 0 {
            let element = optional_element(protocol, full_name)?;
            let decoded = protocol.codec().decode_reflected_traced_from(
                reader,
                element,
                &format!("{path}.full_name.value"),
                4,
            )?;
            provenance.extend(decoded.fields);
            full_name_from_value(&decoded.value)
        } else {
            None
        };
        let full_name_end = reader.position();
        let role = read_spanned_u32(reader, 32)?;
        provenance.extend([
            decoded_field(
                path.clone(),
                "AccountBlockEntry",
                "6 fields",
                item_start,
                reader.position(),
                2,
            ),
            spanned_field(
                format!("{path}.wire_layout_selector"),
                "obfuscation selector",
                &selector,
                3,
            ),
            spanned_field(format!("{path}.account_id"), "uint32", &account_id, 3),
            decoded_field(
                format!("{path}.nickname"),
                "optional string",
                nickname.clone().unwrap_or_else(|| "none".to_owned()),
                nickname_start,
                nickname_end,
                3,
            ),
            spanned_field(format!("{path}.reserved"), "reserved bits", &reserved, 3),
            decoded_field(
                format!("{path}.full_name"),
                "optional name",
                full_name.clone().unwrap_or_else(|| "none".to_owned()),
                full_name_start,
                full_name_end,
                3,
            ),
            spanned_field(format!("{path}.role"), "uint32", &role, 3),
        ]);
        entries.push(AccountBlockEntry {
            account_id: account_id.value,
            nickname,
            full_name,
            role: role.value,
        });
    }
    provenance.push(decoded_field(
        "value.entries",
        "array",
        format!("{} items", entries.len()),
        count.start_bit,
        reader.position(),
        1,
    ));
    Ok(custom_payload(
        Payload::AccountBlocks(AccountBlockPage { entries, complete }),
        provenance,
        "AccountBlocks",
        "2 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn toon_blocks(reader: &mut BitReader<'_>) -> Result<Payload> {
    Ok(toon_blocks_with_provenance(reader)?.payload)
}

pub(crate) fn toon_blocks_with_provenance(reader: &mut BitReader<'_>) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let count = read_spanned_usize(reader, 7)?;
    if count.value != 0 {
        return Err(native_error(
            "non-empty toon-block snapshots are not supported",
        ));
    }
    let complete_start = reader.position();
    let complete = if reader.read(1)? == 0 {
        None
    } else {
        Some(reader.read(1)? != 0)
    };
    let complete_end = reader.position();
    Ok(custom_payload(
        Payload::StartupSummary(StartupSummary {
            kind: "toon-blocks",
            item_count: 0,
            complete,
        }),
        vec![
            spanned_field("value.count", "array length", &count, 1),
            decoded_field(
                "value.complete",
                "optional bool",
                complete.map_or_else(|| "none".to_owned(), |value| value.to_string()),
                complete_start,
                complete_end,
                1,
            ),
        ],
        "ToonBlocks",
        "2 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn cache_stream_items(reader: &mut BitReader<'_>) -> Result<Payload> {
    Ok(cache_stream_items_with_provenance(reader)?.payload)
}

pub(crate) fn cache_stream_items_with_provenance(
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let count = read_spanned_usize(reader, 6)?;
    if count.value > 49 {
        return Err(native_error("cache response contains too many items"));
    }
    let mut provenance = vec![spanned_field(
        "value.items.count",
        "array length",
        &count,
        2,
    )];
    let mut items = Vec::with_capacity(count.value);
    for index in 0..count.value {
        let path = format!("value.items[{index}]");
        let item_start = reader.position();
        let selector = read_spanned_u64(reader, 23)?;
        let handle_start = reader.position();
        let handle: [u8; 40] = reader
            .read_bytes(40, true)?
            .try_into()
            .expect("exactly forty bytes were read");
        let handle_end = reader.position();
        let publication_time = read_spanned_i32(reader)?;
        items.push(CacheStreamItem {
            publication_time: publication_time.value,
            content_handle: handle,
        });
        provenance.extend([
            decoded_field(
                path.clone(),
                "CacheStreamItem",
                "3 fields",
                item_start,
                reader.position(),
                2,
            ),
            spanned_field(
                format!("{path}.wire_layout_selector"),
                "obfuscation selector",
                &selector,
                3,
            ),
            decoded_field(
                format!("{path}.content_handle"),
                "bytes",
                "40 bytes",
                handle_start,
                handle_end,
                3,
            ),
            spanned_field(
                format!("{path}.publication_time"),
                "int32",
                &publication_time,
                3,
            ),
        ]);
    }
    let items_end = reader.position();
    let token = read_spanned_u32(reader, 32)?;
    let total_items = read_spanned_u64(reader, 16)?;
    let offset = read_spanned_u64(reader, 16)?;
    provenance.extend([
        decoded_field(
            "value.items",
            "array",
            format!("{} items", items.len()),
            count.start_bit,
            items_end,
            1,
        ),
        spanned_field("value.token", "uint32", &token, 1),
        spanned_field("value.total_items", "uint16", &total_items, 1),
        spanned_field("value.offset", "uint16", &offset, 1),
    ]);
    Ok(custom_payload(
        Payload::CacheStreamItems(CacheStreamItems {
            items,
            offset: u16::try_from(offset.value).expect("16-bit field fits in u16"),
            total_items: u16::try_from(total_items.value).expect("16-bit field fits in u16"),
            token: token.value,
        }),
        provenance,
        "CacheStreamItems",
        "4 fields",
        start_bit,
        reader.position(),
    ))
}

/// `Battlenet::Conference::FullConferenceDescription`, in the order the client
/// actually writes it — which is not its declaration order, because the type is
/// flagged obfuscated:
///
/// ```text
///   m_id               u32   32        m_index              u16   16
///   (reserved)                8        (reserved)                 29
///   m_maxMembers       u16   16        LocatorKey tag              2
///   m_allowedPrograms  3 + n*32          public arm: locale 32, channel 16
///   m_targetProportion f32   32        (constant)                  8
///   m_allowedRealms    3 + m*32        m_sortOrder          u16   16
///   m_flags            u32   32
/// ```
///
/// `m_targetProportion` sitting between the two arrays is the part that defeats
/// a guess at the order, and `m_index` leading `ShardName` is why the shard
/// cannot be found by walking back from the naming fields. the proportion is
/// 0.9 and the capacity 200, which is why the server marks a room full at 181
/// rather than at 200. recovered from the element decoder at
/// `0x100edf5d0` and its configuration reader at `0x100eb5400` in the decrypted
/// 97563 build, then checked against a retail page: the computed length lands on
/// the next entry for all 39 of them.
const CONFERENCE_ARRAY_COUNT_BITS: usize = 3;
const CONFERENCE_ELEMENT_BITS: usize = 32;
/// `Conference::LocatorKey` arms: private 0, public 2, club 3. only a public
/// conference is a listed channel, and only its arm has a fixed width.
const LOCATOR_KEY_PUBLIC: u64 = 2;

pub(crate) fn conference_descriptions(reader: &mut BitReader<'_>) -> Result<Payload> {
    Ok(conference_descriptions_with_provenance(reader)?.payload)
}

pub(crate) fn conference_descriptions_with_provenance(
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let is_last = read_spanned_u64(reader, 1)?;
    let count = read_spanned_usize(reader, 6)?;
    let mut provenance = vec![
        spanned_field("value.is_last", "bool", &is_last, 1),
        spanned_field("value.entries.count", "array length", &count, 2),
    ];
    let mut entries = Vec::with_capacity(count.value);
    for index in 0..count.value {
        let entry_start = reader.position();
        let conference_id = read_spanned_u32(reader, 32)?;
        reader.read(8)?; // constant across every observed entry
        let max_members = u16::try_from(reader.read(16)?).unwrap_or_default();
        read_conference_array(reader)?; // m_allowedPrograms
        let target_proportion_bits = u32::try_from(reader.read(32)?).unwrap_or_default();
        read_conference_array(reader)?; // m_allowedRealms
        reader.read(32)?; // m_flags — zero on every observed conference
        let shard = u16::try_from(reader.read(16)?).unwrap_or_default();
        reader.read(29)?; // reserved run inside ShardName
        let locator = reader.read(2)?;
        if locator != LOCATOR_KEY_PUBLIC {
            // a private or club conference is not a listed channel, and its
            // locator arm is a different width, so the walk cannot continue
            return Err(Error::Native(format!(
                "conference description {index} uses locator arm {locator}, not a public channel"
            )));
        }
        let locale = u32::try_from(reader.read(32)?).unwrap_or_default();
        let channel_name_id = u16::try_from(reader.read(16)?).unwrap_or_default();
        reader.read(8)?; // constant across every observed entry
        let sort_order = u16::try_from(reader.read(16)?).unwrap_or_default();
        let path = format!("value.entries[{index}]");
        provenance.extend([
            decoded_field(
                path.clone(),
                "FullConferenceDescription",
                "5 fields",
                entry_start,
                reader.position(),
                2,
            ),
            spanned_field(format!("{path}.m_id"), "Conference::Id", &conference_id, 3),
        ]);
        entries.push(ConferenceDescription {
            conference_id: conference_id.value,
            locale,
            channel_name_id,
            shard,
            sort_order,
            max_members,
            target_proportion_bits,
        });
    }
    provenance.push(decoded_field(
        "value.entries",
        "array",
        format!("{} items", entries.len()),
        count.start_bit,
        reader.position(),
        1,
    ));
    Ok(custom_payload(
        Payload::ConferenceDescriptions(ConferenceDescriptions {
            entries,
            is_last: is_last.value != 0,
        }),
        provenance,
        "ConferenceDescriptions",
        "2 fields",
        start_bit,
        reader.position(),
    ))
}

/// skips one of the configuration's arrays: a three-bit count then that many
/// 32-bit elements — `Program::Id` is a `FourCC` and `Realm::Id` a `u32`.
fn read_conference_array(reader: &mut BitReader<'_>) -> Result<()> {
    let count = usize::try_from(reader.read(CONFERENCE_ARRAY_COUNT_BITS)?).unwrap_or_default();
    for _ in 0..count {
        reader.read(CONFERENCE_ELEMENT_BITS)?;
    }
    Ok(())
}

pub(crate) fn conference_member_counts(reader: &mut BitReader<'_>) -> Result<Payload> {
    Ok(conference_member_counts_with_provenance(reader)?.payload)
}

pub(crate) fn conference_member_counts_with_provenance(
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let is_last = read_spanned_u64(reader, 1)?;
    let reserved = read_spanned_u64(reader, 27)?;
    let sampled_present = read_spanned_u64(reader, 1)?;
    let sampled_at = (sampled_present.value != 0)
        .then(|| read_spanned_i32(reader))
        .transpose()?;
    let count = read_spanned_usize(reader, 6)?;
    let mut provenance = vec![
        spanned_field("value.is_last", "bool", &is_last, 1),
        spanned_field("value.reserved", "reserved bits", &reserved, 1),
        spanned_field(
            "value.generated.present",
            "optional flag",
            &sampled_present,
            1,
        ),
        spanned_field("value.entries.count", "array length", &count, 2),
    ];
    if let Some(sampled_at) = &sampled_at {
        provenance.push(spanned_field(
            "value.m_time.value",
            "Time::Seconds",
            sampled_at,
            1,
        ));
    }
    let mut entries = Vec::with_capacity(count.value);
    for index in 0..count.value {
        let path = format!("value.entries[{index}]");
        let entry_start = reader.position();
        let reserved = read_spanned_u64(reader, 23)?;
        let conference_id = read_spanned_u32(reader, 32)?;
        let members = read_spanned_u64(reader, 16)?;
        let full = read_spanned_u64(reader, 1)?;
        let entry_end = reader.position();
        provenance.extend([
            decoded_field(
                path.clone(),
                "MembershipInfo",
                "4 fields",
                entry_start,
                entry_end,
                2,
            ),
            spanned_field(format!("{path}.reserved"), "reserved bits", &reserved, 3),
            spanned_field(format!("{path}.m_id"), "Conference::Id", &conference_id, 3),
            spanned_field(format!("{path}.m_numMembers"), "uint16", &members, 3),
            spanned_field(format!("{path}.m_isFull"), "bool", &full, 3),
        ]);
        entries.push(ConferenceMemberCount {
            conference_id: conference_id.value,
            members: u16::try_from(members.value).expect("16-bit field fits in u16"),
            full: full.value != 0,
        });
    }
    provenance.push(decoded_field(
        "value.entries",
        "array",
        format!("{} items", entries.len()),
        count.start_bit,
        reader.position(),
        1,
    ));
    Ok(custom_payload(
        Payload::ConferenceMemberCounts(ConferenceMemberCounts {
            entries,
            is_last: is_last.value != 0,
            sampled_at: sampled_at.map(|sampled_at| sampled_at.value),
        }),
        provenance,
        "ConferenceMemberCounts",
        "4 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn channel_list(reader: &mut BitReader<'_>) -> Result<Payload> {
    Ok(channel_list_with_provenance(reader)?.payload)
}

pub(crate) fn channel_list_with_provenance(reader: &mut BitReader<'_>) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let reserved = read_spanned_u64(reader, 27)?;
    let flag = read_spanned_u64(reader, 1)?;
    let selector = read_spanned_u64(reader, 9)?;
    let count = read_spanned_usize(reader, 6)?;
    let mut provenance = vec![
        spanned_field("value.reserved", "reserved bits", &reserved, 1),
        spanned_field("value.flag", "wire flag", &flag, 1),
        spanned_field(
            "value.wire_layout_selector",
            "obfuscation selector",
            &selector,
            1,
        ),
        spanned_field("value.entries.count", "array length", &count, 2),
    ];
    let mut entries = Vec::with_capacity(count.value);
    for index in 0..count.value {
        let path = format!("value.entries[{index}]");
        let item_start = reader.position();
        let kind = read_spanned_u8(reader, 8)?;
        let channel_index = read_spanned_u64(reader, 16)?;
        let item_reserved = read_spanned_u64(reader, 24)?;
        let identifier = read_spanned_u64(reader, 16)?;
        provenance.extend([
            decoded_field(
                path.clone(),
                "ChannelDescriptor",
                "4 fields",
                item_start,
                reader.position(),
                2,
            ),
            spanned_field(format!("{path}.kind"), "uint8", &kind, 3),
            spanned_field(format!("{path}.index"), "uint16", &channel_index, 3),
            spanned_field(
                format!("{path}.reserved"),
                "reserved bits",
                &item_reserved,
                3,
            ),
            spanned_field(format!("{path}.identifier"), "uint16", &identifier, 3),
        ]);
        entries.push(ChannelDescriptor {
            kind: kind.value,
            index: u16::try_from(channel_index.value).expect("16-bit field fits in u16"),
            identifier: u16::try_from(identifier.value).expect("16-bit field fits in u16"),
        });
    }
    provenance.push(decoded_field(
        "value.entries",
        "array",
        format!("{} items", entries.len()),
        count.start_bit,
        reader.position(),
        1,
    ));
    Ok(custom_payload(
        Payload::ChannelList(ChannelList { entries }),
        provenance,
        "ChannelList",
        "4 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn chat_join(reader: &mut BitReader<'_>) -> Result<Payload> {
    if reader.read(1)? == 0 {
        let member_handle = read_u32(reader, 32)?;
        let channel_index = read_u8(reader, 3)?;
        if usize::from(channel_index) >= crate::native::protocol::CHANNEL_INDEX_COUNT {
            return Err(native_error("chat join channel index is invalid"));
        }
        reader.read(32)?;
        reader.read(32)?;
        let channel_type = read_u8(reader, 4)?;
        let (channel_name_id, channel_shard_index) = if reader.read(1)? == 0 {
            (None, None)
        } else {
            let (name_id, shard_index) = decode_channel_name(reader)?;
            (name_id, Some(shard_index))
        };
        if reader.read(1)? != 0 {
            decode_channel_config(reader)?;
        }
        if reader.read(1)? != 0 {
            reader.read(32)?;
        }
        let token = (reader.read(1)? != 0)
            .then(|| read_u32(reader, 32))
            .transpose()?;
        return Ok(Payload::ChatJoin(ChatJoin {
            success: true,
            channel_index: Some(channel_index),
            member_handle: Some(member_handle),
            channel_type: Some(channel_type),
            channel_name_id,
            channel_shard_index,
            reason: None,
            token,
        }));
    }
    let reason = read_u16(reader, 16)?;
    let channel_type = if reader.read(1)? == 0 {
        None
    } else {
        Some(read_u8(reader, 4)?)
    };
    let token = (reader.read(1)? != 0)
        .then(|| read_u32(reader, 32))
        .transpose()?;
    Ok(Payload::ChatJoin(ChatJoin {
        success: false,
        channel_index: None,
        member_handle: None,
        channel_type,
        channel_name_id: None,
        channel_shard_index: None,
        reason: Some(reason),
        token,
    }))
}

#[expect(
    clippy::too_many_lines,
    reason = "the success and failure wire arms are clearest in one decoder"
)]
pub(crate) fn chat_join_with_provenance(reader: &mut BitReader<'_>) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let mut provenance = Vec::new();
    let success = trace_integer(
        reader,
        &mut provenance,
        "value.success",
        "bool (inverted)",
        1,
        1,
    )? == 0;
    provenance
        .last_mut()
        .expect("the success trace was just appended")
        .value = success.to_string();
    let payload = if success {
        let member_handle = u32::try_from(trace_integer(
            reader,
            &mut provenance,
            "value.member_handle",
            "uint32",
            32,
            1,
        )?)
        .expect("32-bit field fits in u32");
        let channel_index = u8::try_from(trace_integer(
            reader,
            &mut provenance,
            "value.channel_index",
            "uint3",
            3,
            1,
        )?)
        .expect("three-bit field fits in u8");
        if usize::from(channel_index) >= crate::native::protocol::CHANNEL_INDEX_COUNT {
            return Err(native_error("chat join channel index is invalid"));
        }
        trace_integer(
            reader,
            &mut provenance,
            "value.conference_id",
            "uint32",
            32,
            1,
        )?;
        trace_integer(reader, &mut provenance, "value.owner_id", "uint32", 32, 1)?;
        let channel_type = u8::try_from(trace_integer(
            reader,
            &mut provenance,
            "value.channel_type",
            "uint4",
            4,
            1,
        )?)
        .expect("four-bit field fits in u8");
        let (channel_name_id, channel_shard_index) = trace_channel_name(reader, &mut provenance)?
            .map_or((None, None), |name| {
                (name.public_id, Some(name.shard_index))
            });
        trace_optional_config(reader, &mut provenance)?;
        trace_optional_bits(reader, &mut provenance, "value.reserved", 32, 1)?;
        let token = trace_optional_bits(reader, &mut provenance, "value.token", 32, 1)?
            .map(|value| u32::try_from(value).expect("32-bit field fits in u32"));
        Payload::ChatJoin(ChatJoin {
            success: true,
            channel_index: Some(channel_index),
            member_handle: Some(member_handle),
            channel_type: Some(channel_type),
            channel_name_id,
            channel_shard_index,
            reason: None,
            token,
        })
    } else {
        let reason = u16::try_from(trace_integer(
            reader,
            &mut provenance,
            "value.reason",
            "uint16",
            16,
            1,
        )?)
        .expect("16-bit field fits in u16");
        let channel_type =
            trace_optional_bits(reader, &mut provenance, "value.channel_type", 4, 1)?
                .map(|value| u8::try_from(value).expect("four-bit field fits in u8"));
        let token = trace_optional_bits(reader, &mut provenance, "value.token", 32, 1)?
            .map(|value| u32::try_from(value).expect("32-bit field fits in u32"));
        Payload::ChatJoin(ChatJoin {
            success: false,
            channel_index: None,
            member_handle: None,
            channel_type,
            channel_name_id: None,
            channel_shard_index: None,
            reason: Some(reason),
            token,
        })
    };
    provenance.push(DecodedField {
        path: "value".to_owned(),
        kind: "Chat.JoinNotify2",
        value: if success { "success" } else { "failure" }.to_owned(),
        start_bit,
        end_bit: reader.position(),
        depth: 0,
    });
    provenance.sort_by(|left, right| {
        left.start_bit
            .cmp(&right.start_bit)
            .then_with(|| left.depth.cmp(&right.depth))
            .then_with(|| right.end_bit.cmp(&left.end_bit))
    });
    Ok(DecodedPayload::new(payload, provenance))
}

struct DecodedChannelName {
    public_id: Option<u16>,
    shard_index: u16,
}

fn trace_channel_name(
    reader: &mut BitReader<'_>,
    fields: &mut Vec<DecodedField>,
) -> Result<Option<DecodedChannelName>> {
    let container_start = reader.position();
    let present = trace_integer(
        reader,
        fields,
        "value.channel_name.present",
        "optional flag",
        1,
        2,
    )? != 0;
    if !present {
        return Ok(None);
    }
    let shard_index = u16::try_from(trace_integer(
        reader,
        fields,
        "value.channel_name.shard_index",
        "uint16",
        16,
        2,
    )?)
    .expect("16-bit field fits in u16");
    trace_integer(
        reader,
        fields,
        "value.channel_name.namespace",
        "uint29",
        29,
        2,
    )?;
    let variant = trace_integer(
        reader,
        fields,
        "value.channel_name.kind",
        "choice selector",
        2,
        2,
    )?;
    let public_id = match variant {
        2 => {
            trace_integer(reader, fields, "value.channel_name.locale", "fourcc", 32, 2)?;
            Some(
                u16::try_from(trace_integer(
                    reader,
                    fields,
                    "value.channel_name.id",
                    "uint16",
                    16,
                    2,
                )?)
                .expect("16-bit field fits in u16"),
            )
        }
        1 | 3 => {
            trace_integer(reader, fields, "value.channel_name.index", "uint16", 16, 2)?;
            trace_integer(reader, fields, "value.channel_name.owner", "uint32", 32, 2)?;
            None
        }
        0 => {
            let start_bit = reader.position();
            let value = decode_generated_utf8(reader, 7, 0, 124, 31)?;
            fields.push(DecodedField {
                path: "value.channel_name.literal".to_owned(),
                kind: "utf8",
                value,
                start_bit,
                end_bit: reader.position(),
                depth: 2,
            });
            None
        }
        _ => unreachable!("two bits have only four values"),
    };
    fields.push(DecodedField {
        path: "value.channel_name".to_owned(),
        kind: "optional choice",
        value: "present".to_owned(),
        start_bit: container_start,
        end_bit: reader.position(),
        depth: 1,
    });
    Ok(Some(DecodedChannelName {
        public_id,
        shard_index,
    }))
}

fn trace_optional_config(reader: &mut BitReader<'_>, fields: &mut Vec<DecodedField>) -> Result<()> {
    let path = "value.channel_config";
    let start_bit = reader.position();
    let present = trace_integer(
        reader,
        fields,
        &format!("{path}.present"),
        "optional flag",
        1,
        2,
    )? != 0;
    if present {
        decode_channel_config(reader)?;
    }
    fields.push(DecodedField {
        path: path.to_owned(),
        kind: "optional",
        value: if present { "present" } else { "none" }.to_owned(),
        start_bit,
        end_bit: reader.position(),
        depth: 1,
    });
    Ok(())
}

fn trace_optional_bits(
    reader: &mut BitReader<'_>,
    fields: &mut Vec<DecodedField>,
    path: &str,
    value_bits: usize,
    depth: usize,
) -> Result<Option<u64>> {
    let start_bit = reader.position();
    let present = trace_integer(
        reader,
        fields,
        &format!("{path}.present"),
        "optional flag",
        1,
        depth + 1,
    )? != 0;
    let value = present
        .then(|| {
            trace_integer(
                reader,
                fields,
                &format!("{path}.value"),
                "bits",
                value_bits,
                depth + 1,
            )
        })
        .transpose()?;
    fields.push(DecodedField {
        path: path.to_owned(),
        kind: "optional",
        value: if present { "present" } else { "none" }.to_owned(),
        start_bit,
        end_bit: reader.position(),
        depth,
    });
    Ok(value)
}

fn trace_integer(
    reader: &mut BitReader<'_>,
    fields: &mut Vec<DecodedField>,
    path: &str,
    kind: &'static str,
    width: usize,
    depth: usize,
) -> Result<u64> {
    let start_bit = reader.position();
    let value = reader.read(width)?;
    fields.push(DecodedField {
        path: path.to_owned(),
        kind,
        value: value.to_string(),
        start_bit,
        end_bit: reader.position(),
        depth,
    });
    Ok(value)
}

pub(crate) fn presence_fields(reader: &mut BitReader<'_>) -> Result<Payload> {
    Ok(presence_fields_with_provenance(reader)?.payload)
}

pub(crate) fn presence_fields_with_provenance(
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let count = read_spanned_usize(reader, 7)?;
    if count.value > 100 {
        return Err(native_error(
            "presence announcement contains too many field definitions",
        ));
    }
    let mut provenance = vec![spanned_field(
        "value.entries.count",
        "array length",
        &count,
        2,
    )];
    let mut entries = Vec::with_capacity(count.value);
    for index in 0..count.value {
        let path = format!("value.entries[{index}]");
        let item_start = reader.position();
        let client_only = read_spanned_u64(reader, 1)?;
        let writable = read_spanned_u64(reader, 1)?;
        let ephemeral = read_spanned_u64(reader, 1)?;
        let fixed_start = reader.position();
        let fixed_absent = reader.read(1)? != 0;
        let fixed_size = if fixed_absent {
            None
        } else {
            Some(reader.read(16)?)
        };
        let fixed_end = reader.position();
        let server_only = read_spanned_u64(reader, 1)?;
        let identifier = read_spanned_u8(reader, 8)?;
        if identifier.value > 25 && identifier.value != u8::MAX {
            return Err(native_error(
                "presence announcement contains an unknown field type",
            ));
        }
        let mut flags = 0;
        if writable.value != 0 {
            flags |= PresenceFieldFlags::WRITABLE;
        }
        if ephemeral.value != 0 {
            flags |= PresenceFieldFlags::EPHEMERAL;
        }
        if server_only.value != 0 {
            flags |= PresenceFieldFlags::SERVER_ONLY;
        }
        if client_only.value != 0 {
            flags |= PresenceFieldFlags::CLIENT_ONLY;
        }
        let handle = read_spanned_u32(reader, 32)?;
        provenance.extend([
            decoded_field(
                path.clone(),
                "PresenceField",
                "7 fields",
                item_start,
                reader.position(),
                2,
            ),
            spanned_field(format!("{path}.client_only"), "bool", &client_only, 3),
            spanned_field(format!("{path}.writable"), "bool", &writable, 3),
            spanned_field(format!("{path}.ephemeral"), "bool", &ephemeral, 3),
            decoded_field(
                format!("{path}.fixed_size"),
                "optional uint16",
                fixed_size.map_or_else(|| "none".to_owned(), |value| value.to_string()),
                fixed_start,
                fixed_end,
                3,
            ),
            spanned_field(format!("{path}.server_only"), "bool", &server_only, 3),
            spanned_field(format!("{path}.identifier"), "uint8", &identifier, 3),
            spanned_field(format!("{path}.handle"), "uint32", &handle, 3),
        ]);
        entries.push(PresenceField {
            handle: handle.value,
            identifier: identifier.value,
            flags: PresenceFieldFlags::from_bits(flags),
            fixed_size: fixed_size
                .map(|value| u16::try_from(value).expect("16-bit field fits in u16")),
        });
    }
    provenance.push(decoded_field(
        "value.entries",
        "array",
        format!("{} items", entries.len()),
        count.start_bit,
        reader.position(),
        1,
    ));
    Ok(custom_payload(
        Payload::PresenceFields(PresenceFields { entries }),
        provenance,
        "PresenceFields",
        "1 field",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn presence_update(reader: &mut BitReader<'_>) -> Result<Payload> {
    Ok(presence_update_with_provenance(reader)?.payload)
}

#[expect(
    clippy::too_many_lines,
    reason = "one sequential step per field in the recovered wire layout"
)]
pub(crate) fn presence_update_with_provenance(
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let prefix = read_spanned_u64(reader, 19)?;
    let online_wire = read_spanned_u64(reader, 1)?;
    let online = online_wire.value == 0;
    let local_presence_id = read_spanned_u32(reader, 32)?;
    let master_presence_id = read_spanned_u32(reader, 32)?;
    let field_bytes = read_spanned_usize(reader, 11)?;
    if field_bytes.value > 1024 {
        return Err(native_error("presence update field data is too large"));
    }
    let field_start = reader.position();
    let field_data = reader.read_bytes(field_bytes.value, true)?;
    let field_end = reader.position();
    let reserved = read_spanned_u64(reader, 11)?;
    let mut provenance = vec![
        spanned_field(
            "value.wire_layout_selector",
            "obfuscation selector",
            &prefix,
            1,
        ),
        decoded_field(
            "value.online",
            "bool (inverted)",
            online.to_string(),
            online_wire.start_bit,
            online_wire.end_bit,
            1,
        ),
        spanned_field("value.local_presence_id", "uint32", &local_presence_id, 1),
        spanned_field("value.master_presence_id", "uint32", &master_presence_id, 1),
        decoded_field(
            "value.field_data",
            "bytes",
            format!("{} bytes", field_data.len()),
            field_start,
            field_end,
            1,
        ),
        spanned_field("value.reserved", "reserved bits", &reserved, 1),
    ];
    let cleared_handles = trace_integer_array(
        reader,
        &mut provenance,
        "value.cleared_handles",
        4,
        32,
        "uint32",
    )?
    .into_iter()
    .map(|value| u32::try_from(value).expect("32-bit field fits in u32"))
    .collect();
    let handles = trace_integer_array(reader, &mut provenance, "value.handles", 4, 32, "uint32")?
        .into_iter()
        .map(|value| u32::try_from(value).expect("32-bit field fits in u32"))
        .collect();
    let variable_sizes = trace_integer_array(
        reader,
        &mut provenance,
        "value.variable_sizes",
        4,
        16,
        "uint16",
    )?
    .into_iter()
    .map(|value| u16::try_from(value).expect("16-bit field fits in u16"))
    .collect();
    let optional_start = reader.position();
    let optional = if reader.read(1)? != 0 {
        let flag = reader.read(1)?;
        let id = reader.read(32)?;
        format!("flag {flag}, id {id}")
    } else {
        "none".to_owned()
    };
    provenance.push(decoded_field(
        "value.optional_target",
        "optional",
        optional,
        optional_start,
        reader.position(),
        1,
    ));
    let suffix = read_spanned_u64(reader, 8)?;
    provenance.push(spanned_field(
        "value.trailing_selector",
        "obfuscation selector",
        &suffix,
        1,
    ));
    Ok(custom_payload(
        Payload::PresenceUpdate(PresenceUpdate {
            local_presence_id: local_presence_id.value,
            master_presence_id: master_presence_id.value,
            field_data,
            cleared_handles,
            handles,
            variable_sizes,
            online,
        }),
        provenance,
        "PresenceUpdate",
        "10 fields",
        start_bit,
        reader.position(),
    ))
}

fn trace_integer_array(
    reader: &mut BitReader<'_>,
    provenance: &mut Vec<DecodedField>,
    path: &str,
    count_bits: usize,
    item_bits: usize,
    item_kind: &'static str,
) -> Result<Vec<u64>> {
    let start_bit = reader.position();
    let count = read_spanned_usize(reader, count_bits)?;
    provenance.push(spanned_field(
        format!("{path}.count"),
        "array length",
        &count,
        2,
    ));
    let mut values = Vec::with_capacity(count.value);
    for index in 0..count.value {
        let item = read_spanned_u64(reader, item_bits)?;
        provenance.push(spanned_field(
            format!("{path}[{index}]"),
            item_kind,
            &item,
            2,
        ));
        values.push(item.value);
    }
    provenance.push(decoded_field(
        path,
        "array",
        format!("{} items", values.len()),
        start_bit,
        reader.position(),
        1,
    ));
    Ok(values)
}

pub(crate) fn current_season(reader: &mut BitReader<'_>) -> Result<Payload> {
    Ok(current_season_with_provenance(reader)?.payload)
}

#[expect(
    clippy::too_many_lines,
    reason = "one sequential step per field in the recovered wire layout"
)]
pub(crate) fn current_season_with_provenance(reader: &mut BitReader<'_>) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let failure = read_spanned_u64(reader, 1)?;
    let mut provenance = vec![spanned_field("value.failure", "bool", &failure, 1)];
    if failure.value != 0 {
        let error = read_spanned_u64(reader, 16)?;
        provenance.push(spanned_field("value.error", "uint16", &error, 1));
        return Ok(custom_payload(
            Payload::StartupSummary(StartupSummary {
                kind: "current-season-error",
                item_count: usize::try_from(error.value).expect("16-bit value fits usize"),
                complete: None,
            }),
            provenance,
            "CurrentSeason",
            "failure",
            start_bit,
            reader.position(),
        ));
    }
    let authority = read_spanned_u64(reader, 1)?;
    provenance.push(spanned_field("value.authority", "bool", &authority, 1));
    let ranked = read_spanned_usize(reader, 7)?;
    if ranked.value > 100 {
        return Err(native_error("current season has too many matchmakers"));
    }
    let ranked_start = ranked.start_bit;
    provenance.push(spanned_field(
        "value.ranked.count",
        "array length",
        &ranked,
        2,
    ));
    for index in 0..ranked.value {
        let item_start = reader.position();
        decode_ranked_matchmaker(reader)?;
        provenance.push(decoded_field(
            format!("value.ranked[{index}]"),
            "RankedMatchmaker",
            "wire record",
            item_start,
            reader.position(),
            2,
        ));
    }
    provenance.push(decoded_field(
        "value.ranked",
        "array",
        format!("{} items", ranked.value),
        ranked_start,
        reader.position(),
        1,
    ));
    let leagues = read_spanned_usize(reader, 9)?;
    if leagues.value > 448 {
        return Err(native_error("current season has too many leagues"));
    }
    let leagues_start = leagues.start_bit;
    provenance.push(spanned_field(
        "value.leagues.count",
        "array length",
        &leagues,
        2,
    ));
    for index in 0..leagues.value {
        let item_start = reader.position();
        decode_profile_address(reader)?;
        reader.read(8)?;
        decode_league_key(reader)?;
        provenance.push(decoded_field(
            format!("value.leagues[{index}]"),
            "League",
            "wire record",
            item_start,
            reader.position(),
            2,
        ));
    }
    provenance.push(decoded_field(
        "value.leagues",
        "array",
        format!("{} items", leagues.value),
        leagues_start,
        reader.position(),
        1,
    ));
    let season_start = reader.position();
    decode_season_info(reader)?;
    provenance.push(decoded_field(
        "value.season",
        "SeasonInfo",
        "wire record",
        season_start,
        reader.position(),
        1,
    ));
    let configurations = read_spanned_usize(reader, 9)?;
    if configurations.value > 448 {
        return Err(native_error(
            "current season has too many league configurations",
        ));
    }
    let configurations_start = configurations.start_bit;
    provenance.push(spanned_field(
        "value.configurations.count",
        "array length",
        &configurations,
        2,
    ));
    for index in 0..configurations.value {
        let item_start = reader.position();
        decode_league_key(reader)?;
        reader.read(64)?;
        if reader.read(1)? != 0 {
            reader.read(32)?;
        }
        provenance.push(decoded_field(
            format!("value.configurations[{index}]"),
            "LeagueConfiguration",
            "wire record",
            item_start,
            reader.position(),
            2,
        ));
    }
    provenance.push(decoded_field(
        "value.configurations",
        "array",
        format!("{} items", configurations.value),
        configurations_start,
        reader.position(),
        1,
    ));
    Ok(custom_payload(
        Payload::StartupSummary(StartupSummary {
            kind: "current-season",
            item_count: ranked.value + leagues.value + configurations.value,
            complete: Some(authority.value != 0),
        }),
        provenance,
        "CurrentSeason",
        "5 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn club_invite_action(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let value = protocol.codec().decode_from(reader, type_id)?;
    let root = value
        .as_struct()
        .ok_or_else(|| native_error("client club invitation is not a struct"))?;
    let action = root
        .get("m_action")
        .and_then(BsnValue::as_struct)
        .ok_or_else(|| native_error("client club invitation omits its action"))?;
    let club_id = expect_u32(
        action
            .get("m_clubId")
            .ok_or_else(|| native_error("club invitation omits its group id"))?
            .clone(),
        "club invitation group id",
    )?;
    let member = toon_handle_from_value(
        action
            .get("m_member")
            .ok_or_else(|| native_error("club invitation omits its member"))?,
    )?
    .ok_or_else(|| native_error("club invitation member is absent"))?;
    Ok(Payload::ClubInviteAction(ClubInviteAction {
        club_id,
        program: member.program_id,
        member_kind: member.region,
        member_realm: member.realm,
        member_id: member.id,
    }))
}

pub(crate) fn club_invite_action_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let decoded = protocol.codec().decode_traced_from(reader, type_id)?;
    let root = decoded
        .value
        .as_struct()
        .ok_or_else(|| native_error("client club invitation is not a struct"))?;
    let action = root
        .get("m_action")
        .and_then(BsnValue::as_struct)
        .ok_or_else(|| native_error("client club invitation omits its action"))?;
    let club_id = expect_u32(
        action
            .get("m_clubId")
            .ok_or_else(|| native_error("club invitation omits its group id"))?
            .clone(),
        "club invitation group id",
    )?;
    let member = toon_handle_from_value(
        action
            .get("m_member")
            .ok_or_else(|| native_error("club invitation omits its member"))?,
    )?
    .ok_or_else(|| native_error("club invitation member is absent"))?;
    Ok(DecodedPayload::new(
        Payload::ClubInviteAction(ClubInviteAction {
            club_id,
            program: member.program_id,
            member_kind: member.region,
            member_realm: member.realm,
            member_id: member.id,
        }),
        decoded.fields,
    ))
}

pub(crate) fn club_info(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let start = reader.position();
    let buffer_bits = reader.data().len() * 8;
    let mut counts = BTreeMap::new();
    let walked_end = match protocol.codec().decode_from(reader, type_id) {
        Ok(value) => {
            collect_club_counts(&value, &mut counts);
            Some(reader.position())
        }
        Err(_) => None,
    };
    reader.set_position(start)?;
    let Ok((mut clubs, _, elements_end, incomplete)) =
        walk_club_elements(reader, walked_end.unwrap_or(buffer_bits))
    else {
        return Err(incomplete_club_frame(buffer_bits));
    };
    apply_club_counts(&mut clubs, &counts);
    if incomplete {
        return Err(incomplete_club_frame(buffer_bits));
    }
    let end = (elements_end + 1).div_ceil(8) * 8;
    if end > buffer_bits {
        return Err(Error::IncompleteFrame(format!(
            "club info needs {} bytes, {} buffered",
            end / 8,
            buffer_bits / 8
        )));
    }
    reader.set_position(end)?;
    Ok(Payload::ClubInfo(clubs))
}

pub(crate) fn club_search(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let value = protocol.codec().decode_reflected_from(reader, type_id)?;
    let mut ids = Vec::new();
    collect_club_ids(&value, &mut ids);
    Ok(Payload::ClubSearch(ids))
}

pub(crate) fn club_search_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    reflected_payload(protocol, type_id, reader, |value| {
        let mut ids = Vec::new();
        collect_club_ids(&value, &mut ids);
        Ok(Payload::ClubSearch(ids))
    })
}

/// head counts lifted from the reflected walk of a club response.
///
/// the element scraper below reads a club's identity from fixed bit offsets and
/// steps over the tail, which is where `m_memberCount` lives; the reflected
/// decode names every field but is only trusted for its end position. rather
/// than teach the scraper new offsets, this reads the counts out of the walk
/// that already happened and merges them in by club id.
#[derive(Clone, Copy, Default)]
struct ClubCounts {
    member_count: Option<u32>,
    online: Option<u32>,
}

fn collect_club_counts(value: &BsnValue, counts: &mut BTreeMap<u32, ClubCounts>) {
    if let BsnValue::Struct(fields) = value {
        // `ClubInfo` — a summary plus the live online status beside it.
        if let (Some(BsnValue::Struct(summary)), Some(BsnValue::Struct(status))) =
            (fields.get("m_summary"), fields.get("m_status"))
            && let Some(club_id) = struct_u32(summary, "m_id")
        {
            let entry = counts.entry(club_id).or_default();
            entry.member_count = entry.member_count.or(struct_u32(summary, "m_memberCount"));
            entry.online = entry.online.or(struct_u32(status, "m_online"));
        }
        // a bare `ClubSummaryInfo`, which knows the roster size but not who is on it.
        if let (Some(club_id), Some(member_count)) =
            (struct_u32(fields, "m_id"), struct_u32(fields, "m_memberCount"))
        {
            let entry = counts.entry(club_id).or_default();
            entry.member_count = entry.member_count.or(Some(member_count));
        }
        for field in &fields.fields {
            collect_club_counts(&field.value, counts);
        }
        return;
    }
    match value {
        BsnValue::Array(items) => {
            for item in items {
                collect_club_counts(item, counts);
            }
        }
        BsnValue::Optional(Some(inner)) | BsnValue::Choice { value: inner, .. } => {
            collect_club_counts(inner, counts);
        }
        _ => {}
    }
}

fn struct_u32(fields: &BsnStruct, field: &str) -> Option<u32> {
    match fields.get(field)? {
        BsnValue::Integer(number) => u32::try_from(*number).ok(),
        _ => None,
    }
}

fn apply_club_counts(clubs: &mut [ClubSummary], counts: &BTreeMap<u32, ClubCounts>) {
    for club in clubs {
        let Some(found) = counts.get(&club.club_id) else {
            continue;
        };
        club.member_count = found.member_count;
        club.online = found.online;
    }
}

fn collect_club_ids(value: &BsnValue, ids: &mut Vec<u32>) {
    match value {
        BsnValue::Array(items) => {
            for item in items {
                if let BsnValue::Integer(id) = item
                    && let Ok(id) = u32::try_from(*id)
                {
                    ids.push(id);
                }
            }
        }
        BsnValue::Optional(Some(inner)) => collect_club_ids(inner, ids),
        BsnValue::Choice { value, .. } => collect_club_ids(value, ids),
        BsnValue::Struct(fields) => {
            for field in &fields.fields {
                collect_club_ids(&field.value, ids);
            }
        }
        _ => {}
    }
}

const CLUB_FIRST_ELEMENT: usize = 52;
const CLUB_ID_OFFSET: usize = 32;
const CLUB_NAME_LENGTH_OFFSET: usize = 72;
const CLUB_NAME_LENGTH_BITS: usize = 8;
const CLUB_CATEGORY_OFFSET: usize = 64;
const CLUB_TYPE_OFFSET: usize = 134;
const CLUB_TAG_FLAG_OFFSET: usize = 206;
const CLUB_PRIVATE_FLAG_OFFSET: usize = 128;
const CLUB_ELEMENT_TAIL: usize = 158;
const CLUB_ICON_FLAG_OFFSET: usize = 28;
const CLUB_ICON_BYTES: usize = 40;
const CLUB_ELEMENT_REMAINDER: usize = CLUB_ELEMENT_TAIL - CLUB_ICON_FLAG_OFFSET - 1;
const CLUB_COUNT_OFFSET: usize = 12;
const CLUB_COUNT_BITS: usize = 8;
const CLUB_LIMIT: usize = 201;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClubSummaryTrace {
    pub item: std::ops::Range<usize>,
    pub club_id: std::ops::Range<usize>,
    pub name: std::ops::Range<usize>,
    pub kind: std::ops::Range<usize>,
    pub category: std::ops::Range<usize>,
    pub private: std::ops::Range<usize>,
}

#[allow(clippy::too_many_lines)]
fn walk_club_elements(
    reader: &mut BitReader<'_>,
    total: usize,
) -> Result<(Vec<ClubSummary>, Vec<ClubSummaryTrace>, usize, bool)> {
    let mut clubs = Vec::new();
    let mut traces = Vec::new();
    reader.set_position(CLUB_COUNT_OFFSET)?;
    let declared = usize::try_from(reader.read(CLUB_COUNT_BITS)?)
        .unwrap_or(CLUB_LIMIT)
        .min(CLUB_LIMIT);
    let mut start = CLUB_FIRST_ELEMENT;
    let mut incomplete = false;
    while clubs.len() < declared {
        let length_at = start + CLUB_NAME_LENGTH_OFFSET;
        if length_at + CLUB_NAME_LENGTH_BITS > total {
            incomplete = true;
            break;
        }
        reader.set_position(start + CLUB_ID_OFFSET)?;
        let Ok(club_id) = u32::try_from(reader.read(32)?) else {
            break;
        };
        reader.set_position(length_at)?;
        let length = usize::try_from(reader.read(CLUB_NAME_LENGTH_BITS)?).unwrap_or(usize::MAX);
        let text_at = (length_at + CLUB_NAME_LENGTH_BITS).div_ceil(8) * 8;
        if length == 0 || length > 32 {
            break;
        }
        if text_at + length * 8 > total {
            incomplete = true;
            break;
        }
        reader.set_position(text_at)?;
        let Ok(name) = reader.read_bytes(length, false).map(String::from_utf8) else {
            break;
        };
        let Ok(name) = name else {
            break;
        };
        if name.chars().any(char::is_control) {
            break;
        }
        let name_end = text_at + length * 8;
        reader.set_position(start + CLUB_CATEGORY_OFFSET)?;
        let category = u8::try_from(reader.read(8)?).unwrap_or_default();
        if name_end + CLUB_TYPE_OFFSET + 8 > total {
            incomplete = true;
            break;
        }
        reader.set_position(name_end + CLUB_TYPE_OFFSET)?;
        let kind = u8::try_from(reader.read(8)?).unwrap_or_default();
        let private_at = name_end + CLUB_PRIVATE_FLAG_OFFSET;
        let private = if private_at < total {
            reader.set_position(private_at)?;
            reader.read(1)? == 1
        } else {
            false
        };
        clubs.push(ClubSummary {
            club_id,
            name: Some(name),
            kind,
            category,
            private,
            member_count: None,
            online: None,
        });

        let flag_at = name_end + CLUB_TAG_FLAG_OFFSET;
        if flag_at + 6 > total {
            incomplete = true;
            break;
        }
        reader.set_position(flag_at)?;
        let mut cursor = flag_at + 1;
        if reader.read(1)? == 1 {
            let tag = usize::try_from(reader.read(5)?).unwrap_or(usize::MAX);
            if tag > 24 {
                break;
            }
            cursor = (cursor + 5).div_ceil(8) * 8 + tag * 8;
        }
        let icon_at = cursor + CLUB_ICON_FLAG_OFFSET;
        if icon_at >= total {
            incomplete = true;
            break;
        }
        reader.set_position(icon_at)?;
        cursor = if reader.read(1)? == 1 {
            let handle = (icon_at + 1).div_ceil(8) * 8 + CLUB_ICON_BYTES * 8;
            if handle > total {
                incomplete = true;
                break;
            }
            handle
        } else {
            icon_at + 1
        };
        let next_start = cursor + CLUB_ELEMENT_REMAINDER;
        traces.push(ClubSummaryTrace {
            item: start..next_start,
            club_id: start + CLUB_ID_OFFSET..start + CLUB_ID_OFFSET + 32,
            name: text_at..name_end,
            kind: name_end + CLUB_TYPE_OFFSET..name_end + CLUB_TYPE_OFFSET + 8,
            category: start + CLUB_CATEGORY_OFFSET..start + CLUB_CATEGORY_OFFSET + 8,
            private: private_at..private_at + 1,
        });
        start = next_start;
    }
    Ok((clubs, traces, start, incomplete))
}

fn incomplete_club_frame(buffer_bits: usize) -> Error {
    Error::IncompleteFrame(format!(
        "club record runs past the {} bytes buffered",
        buffer_bits / 8
    ))
}

pub(crate) fn club_summaries(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let start = reader.position();
    let buffer_bits = reader.data().len() * 8;
    let walked = protocol.codec().decode_from(reader, type_id);
    let total = match &walked {
        Ok(_) => reader.position(),
        Err(_) => buffer_bits,
    };
    let mut counts = BTreeMap::new();
    if let Ok(value) = &walked {
        collect_club_counts(value, &mut counts);
    }
    reader.set_position(start)?;
    let Ok((mut clubs, _, start, incomplete)) = walk_club_elements(reader, total) else {
        return Err(incomplete_club_frame(buffer_bits));
    };
    if incomplete {
        return Err(incomplete_club_frame(buffer_bits));
    }
    apply_club_counts(&mut clubs, &counts);
    // each summary ends with a u32, one rank byte per club, and a final flag.
    let tail = 32 + 8 + clubs.len() * 8 + 1;
    let end = (start + tail).div_ceil(8) * 8;
    if end > buffer_bits {
        return Err(incomplete_club_frame(buffer_bits));
    }
    reader.set_position(end)?;
    Ok(Payload::ClubSummaries(clubs))
}

pub(crate) fn club_summaries_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let buffer_bits = reader.data().len() * 8;
    let walked = protocol.codec().decode_from(reader, type_id);
    let total = match &walked {
        Ok(_) => reader.position(),
        Err(_) => buffer_bits,
    };
    let mut counts = BTreeMap::new();
    if let Ok(value) = &walked {
        collect_club_counts(value, &mut counts);
    }
    reader.set_position(start_bit)?;
    let Ok((mut clubs, traces, elements_end, incomplete)) = walk_club_elements(reader, total)
    else {
        return Err(incomplete_club_frame(buffer_bits));
    };
    apply_club_counts(&mut clubs, &counts);
    if incomplete {
        return Err(incomplete_club_frame(buffer_bits));
    }
    let tail = 32 + 8 + clubs.len() * 8 + 1;
    let end_bit = (elements_end + tail).div_ceil(8) * 8;
    if end_bit > buffer_bits {
        return Err(incomplete_club_frame(buffer_bits));
    }
    reader.set_position(end_bit)?;
    let provenance = club_summary_provenance(&clubs, &traces, start_bit, end_bit);
    Ok(DecodedPayload::new(
        Payload::ClubSummaries(clubs),
        provenance,
    ))
}

pub(crate) fn club_info_with_provenance(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let buffer_bits = reader.data().len() * 8;
    let mut counts = BTreeMap::new();
    let walked_end = match protocol.codec().decode_from(reader, type_id) {
        Ok(value) => {
            collect_club_counts(&value, &mut counts);
            Some(reader.position())
        }
        Err(_) => None,
    };
    reader.set_position(start_bit)?;
    let Ok((mut clubs, traces, elements_end, incomplete)) =
        walk_club_elements(reader, walked_end.unwrap_or(buffer_bits))
    else {
        return Err(incomplete_club_frame(buffer_bits));
    };
    apply_club_counts(&mut clubs, &counts);
    if incomplete {
        return Err(incomplete_club_frame(buffer_bits));
    }
    let end_bit = (elements_end + 1).div_ceil(8) * 8;
    if end_bit > buffer_bits {
        return Err(Error::IncompleteFrame(format!(
            "club info needs {} bytes, {} buffered",
            end_bit / 8,
            buffer_bits / 8
        )));
    }
    reader.set_position(end_bit)?;
    let provenance = club_summary_provenance(&clubs, &traces, start_bit, end_bit);
    Ok(DecodedPayload::new(Payload::ClubInfo(clubs), provenance))
}

fn club_summary_provenance(
    clubs: &[ClubSummary],
    traces: &[ClubSummaryTrace],
    start_bit: usize,
    end_bit: usize,
) -> Vec<DecodedField> {
    if traces.len() != clubs.len() {
        return Vec::new();
    }
    let mut fields = vec![DecodedField {
        path: "value".to_owned(),
        kind: "array",
        value: if clubs.len() == 1 {
            "1 item".to_owned()
        } else {
            format!("{} items", clubs.len())
        },
        start_bit,
        end_bit,
        depth: 0,
    }];
    for (index, (club, trace)) in clubs.iter().zip(traces).enumerate() {
        let path = format!("value[{index}]");
        fields.push(DecodedField {
            path: path.clone(),
            kind: "object",
            value: "5 fields".to_owned(),
            start_bit: trace.item.start,
            end_bit: trace.item.end,
            depth: 1,
        });
        fields.extend([
            traced_club_field(
                format!("{path}.club_id"),
                "uint32",
                club.club_id.to_string(),
                &trace.club_id,
            ),
            traced_club_field(
                format!("{path}.name"),
                "string",
                club.name.clone().unwrap_or_default(),
                &trace.name,
            ),
            traced_club_field(
                format!("{path}.kind"),
                "uint8",
                club.kind.to_string(),
                &trace.kind,
            ),
            traced_club_field(
                format!("{path}.category"),
                "uint8",
                club.category.to_string(),
                &trace.category,
            ),
            traced_club_field(
                format!("{path}.private"),
                "bool",
                club.private.to_string(),
                &trace.private,
            ),
        ]);
    }
    fields
}

fn traced_club_field(
    path: String,
    kind: &'static str,
    value: String,
    range: &std::ops::Range<usize>,
) -> DecodedField {
    DecodedField {
        path,
        kind,
        value,
        start_bit: range.start,
        end_bit: range.end,
        depth: 2,
    }
}

pub(crate) fn club_settings(reader: &mut BitReader<'_>) -> Result<Payload> {
    Ok(club_settings_with_provenance(reader)?.payload)
}

pub(crate) fn club_settings_with_provenance(reader: &mut BitReader<'_>) -> Result<DecodedPayload> {
    let start_bit = reader.position();
    let description = decode_spanned_generated_utf8(reader, 13, 0, 4096, 1024)?;
    let message = decode_spanned_generated_utf8(reader, 13, 0, 4096, 1024)?;
    let first = read_spanned_u32(reader, 32)?;
    let second = read_spanned_u32(reader, 32)?;
    let first_signed = read_spanned_i32(reader)?;
    let second_signed = read_spanned_i32(reader)?;
    let third_signed = read_spanned_i32(reader)?;
    let provenance = vec![
        spanned_field("value.description", "string", &description, 1),
        spanned_field("value.message", "string", &message, 1),
        spanned_field("value.setting_0", "uint32", &first, 1),
        spanned_field("value.setting_1", "uint32", &second, 1),
        spanned_field("value.setting_2", "int32", &first_signed, 1),
        spanned_field("value.setting_3", "int32", &second_signed, 1),
        spanned_field("value.setting_4", "int32", &third_signed, 1),
    ];
    Ok(custom_payload(
        Payload::StartupSummary(StartupSummary {
            kind: "club-settings",
            item_count: 0,
            complete: None,
        }),
        provenance,
        "ClubSettings",
        "7 fields",
        start_bit,
        reader.position(),
    ))
}

pub(crate) fn decode_generated_utf8(
    reader: &mut BitReader<'_>,
    length_bits: usize,
    minimum_bytes: usize,
    maximum_bytes: usize,
    maximum_characters: usize,
) -> Result<String> {
    let byte_count = read_usize(reader, length_bits)? + minimum_bytes;
    if byte_count > maximum_bytes {
        return Err(native_error("generated native string is too long"));
    }
    let bytes = reader.read_bytes(byte_count, true)?;
    let value = String::from_utf8(bytes)
        .map_err(|_| native_error("generated native string is not UTF-8"))?;
    if value.chars().count() > maximum_characters {
        return Err(native_error(
            "generated native string has too many characters",
        ));
    }
    Ok(value)
}

fn choice_variant(shape: &TypeShape, index: i128) -> Result<u32> {
    let position = shape
        .index_values
        .iter()
        .position(|candidate| *candidate == index)
        .ok_or_else(|| native_error(format!("choice has no variant {index}")))?;
    Ok(shape.member_types[position])
}

#[derive(Default)]
struct TracedMemberStatus {
    toon_name: Option<ToonFullName>,
    party_status: Option<PartyMemberStatus>,
}

fn trace_member_status(
    protocol: &Protocol,
    status_shape: &TypeShape,
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
    provenance: &mut Vec<DecodedField>,
) -> Result<TracedMemberStatus> {
    let start_bit = reader.position();
    let choice = read_spanned_u64(reader, 3)?;
    let variant = choice_variant(status_shape, i128::from(choice.value))?;
    provenance.push(spanned_field(
        format!("{path}.kind"),
        "choice selector",
        &choice,
        depth + 1,
    ));
    let mut status = TracedMemberStatus::default();
    match choice.value {
        0 | 1 => {
            let decoded = protocol.codec().decode_reflected_traced_from(
                reader,
                variant,
                &format!("{path}.value"),
                depth + 1,
            )?;
            if choice.value == 1 {
                status.party_status = decoded
                    .value
                    .as_struct()
                    .and_then(|value| value.get("m_partyStatus"))
                    .and_then(BsnValue::as_integer)
                    .and_then(|value| match value {
                        0 => Some(PartyMemberStatus::Invalid),
                        1 => Some(PartyMemberStatus::Online),
                        2 => Some(PartyMemberStatus::Offline),
                        3 => Some(PartyMemberStatus::Invited),
                        _ => None,
                    });
            }
            provenance.extend(decoded.fields);
        }
        2 => {
            let value = read_spanned_u64(reader, 8)?;
            provenance.push(spanned_field(
                format!("{path}.value"),
                "uint8",
                &value,
                depth + 1,
            ));
        }
        3 => {
            let wrapper = protocol.peel_alias(variant)?;
            let wrapper_shape = protocol.codec().schema().shape(wrapper)?;
            let inner = *wrapper_shape
                .member_types
                .first()
                .ok_or_else(|| native_error("talker status wrapper is empty"))?;
            let identifier = protocol.member_type(inner, "m_id")?;
            let decoded = protocol.codec().decode_reflected_traced_from(
                reader,
                identifier,
                &format!("{path}.id"),
                depth + 1,
            )?;
            provenance.extend(decoded.fields);
            let flag = read_spanned_u64(reader, 1)?;
            provenance.push(spanned_field(
                format!("{path}.flag"),
                "bool",
                &flag,
                depth + 1,
            ));
        }
        4 | 6 => {
            let value = read_spanned_u64(reader, 1)?;
            provenance.push(spanned_field(
                format!("{path}.value"),
                "bool",
                &value,
                depth + 1,
            ));
        }
        5 => {
            status.toon_name = Some(trace_chat_display_name(
                reader,
                path,
                depth + 1,
                provenance,
            )?);
        }
        7 => {}
        _ => return Err(native_error("chat member status has an unknown choice")),
    }
    provenance.push(decoded_field(
        path,
        "MemberStatus",
        format!("variant {}", choice.value),
        start_bit,
        reader.position(),
        depth,
    ));
    Ok(status)
}

fn trace_chat_display_name(
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
    provenance: &mut Vec<DecodedField>,
) -> Result<ToonFullName> {
    let region = read_spanned_u8(reader, 8)?;
    let program_id = read_spanned_u32(reader, 32)?;
    let realm = read_spanned_u32(reader, 32)?;
    let name = decode_spanned_generated_utf8(reader, 7, 2, 100, 25)?;
    provenance.extend([
        spanned_field(format!("{path}.region"), "uint8", &region, depth),
        spanned_field(format!("{path}.program_id"), "fourcc", &program_id, depth),
        spanned_field(format!("{path}.realm"), "uint32", &realm, depth),
        spanned_field(format!("{path}.name"), "string", &name, depth),
    ]);
    Ok(ToonFullName {
        region: region.value,
        program_id: program_id.value,
        realm: realm.value,
        name: name.value,
    })
}

fn trace_friend_container(
    protocol: &Protocol,
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
    provenance: &mut Vec<DecodedField>,
) -> Result<FriendEntry> {
    let start_bit = reader.position();
    let choice = read_spanned_u64(reader, 2)?;
    provenance.push(spanned_field(
        format!("{path}.kind"),
        "choice selector",
        &choice,
        depth + 1,
    ));
    let entry = match choice.value {
        0 => trace_friend_character(protocol, reader, path, depth, provenance)?,
        1 => trace_friend_account(protocol, reader, path, depth, provenance)?,
        2 => {
            let account_id = read_spanned_u32(reader, 32)?;
            provenance.push(spanned_field(
                format!("{path}.identity.account_id"),
                "uint32",
                &account_id,
                depth + 1,
            ));
            trace_friend_custom_message(reader, path, depth + 1, provenance)?;
            let last_online = read_spanned_i32(reader)?;
            provenance.push(spanned_field(
                format!("{path}.last_online"),
                "int32",
                &last_online,
                depth + 1,
            ));
            FriendEntry {
                identity: FriendIdentity::Account(account_id.value),
                display_name: None,
                full_name: None,
                note: None,
                profile: None,
                toon_name: None,
            }
        }
        _ => {
            return Err(native_error(
                "friends update contains an unknown container choice",
            ));
        }
    };
    provenance.push(decoded_field(
        path,
        "FriendEntry",
        format!("variant {}", choice.value),
        start_bit,
        reader.position(),
        depth,
    ));
    Ok(entry)
}

fn trace_friend_account(
    protocol: &Protocol,
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
    provenance: &mut Vec<DecodedField>,
) -> Result<FriendEntry> {
    let account_type = protocol
        .codec()
        .schema()
        .unique_type_id("Battlenet::Friends::FriendContainer5::Account")?;
    let account_id = read_spanned_u32(reader, 32)?;
    provenance.push(spanned_field(
        format!("{path}.identity.account_id"),
        "uint32",
        &account_id,
        depth + 1,
    ));
    let full_name_start = reader.position();
    let full_name = if reader.read(1)? != 0 {
        let full_name_type = protocol.member_type(account_type, "m_fullName")?;
        let element = optional_element(protocol, full_name_type)?;
        let decoded = protocol.codec().decode_reflected_traced_from(
            reader,
            element,
            &format!("{path}.full_name.value"),
            depth + 2,
        )?;
        provenance.extend(decoded.fields);
        full_name_from_value(&decoded.value)
    } else {
        None
    };
    provenance.push(decoded_field(
        format!("{path}.full_name"),
        "optional name",
        full_name.clone().unwrap_or_else(|| "none".to_owned()),
        full_name_start,
        reader.position(),
        depth + 1,
    ));
    let display_name = trace_optional_utf8(
        reader,
        provenance,
        &format!("{path}.display_name"),
        depth + 1,
        7,
        0,
        108,
        27,
    )?;
    let profile_value = traced_reflected_field(
        protocol,
        account_type,
        "m_profile",
        reader,
        &format!("{path}.profile"),
        depth + 1,
        provenance,
    )?;
    let profile = profile_address_from_value(&profile_value)?;
    trace_friend_custom_message(reader, path, depth + 1, provenance)?;
    let note = trace_optional_utf8(
        reader,
        provenance,
        &format!("{path}.note"),
        depth + 1,
        9,
        0,
        508,
        127,
    )?;
    let last_online = read_spanned_i32(reader)?;
    let account_serial = read_spanned_u64(reader, 64)?;
    let game_account_id = read_spanned_u32(reader, 32)?;
    provenance.extend([
        spanned_field(
            format!("{path}.last_online"),
            "int32",
            &last_online,
            depth + 1,
        ),
        spanned_field(
            format!("{path}.account_serial"),
            "uint64",
            &account_serial,
            depth + 1,
        ),
        spanned_field(
            format!("{path}.game_account_id"),
            "uint32",
            &game_account_id,
            depth + 1,
        ),
    ]);
    Ok(FriendEntry {
        identity: FriendIdentity::Account(account_id.value),
        display_name,
        full_name,
        note,
        profile,
        toon_name: None,
    })
}

fn trace_friend_character(
    protocol: &Protocol,
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
    provenance: &mut Vec<DecodedField>,
) -> Result<FriendEntry> {
    let character_type = protocol
        .codec()
        .schema()
        .unique_type_id("Battlenet::Friends::FriendContainer5::Character")?;
    let identity = trace_toon_handle(
        protocol,
        reader,
        &format!("{path}.identity"),
        depth + 1,
        provenance,
    )?;
    let display_name = decode_spanned_generated_utf8(reader, 7, 2, 100, 25)?;
    provenance.push(spanned_field(
        format!("{path}.display_name"),
        "string",
        &display_name,
        depth + 1,
    ));
    let profile_value = traced_reflected_field(
        protocol,
        character_type,
        "m_profile",
        reader,
        &format!("{path}.profile"),
        depth + 1,
        provenance,
    )?;
    let profile = profile_address_from_value(&profile_value)?;
    let note = trace_optional_utf8(
        reader,
        provenance,
        &format!("{path}.note"),
        depth + 1,
        9,
        0,
        508,
        127,
    )?;
    Ok(FriendEntry {
        identity,
        display_name: Some(display_name.value),
        full_name: None,
        note,
        profile,
        toon_name: None,
    })
}

fn trace_toon_handle(
    protocol: &Protocol,
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
    provenance: &mut Vec<DecodedField>,
) -> Result<FriendIdentity> {
    let start_bit = reader.position();
    let selected_type = protocol.incoming_type(
        crate::native::protocol::TOON_SLOT,
        crate::native::protocol::TOON_SELECTED_COMMAND,
    )?;
    let handle_type = protocol.member_type(selected_type, "m_toonHandle")?;
    let mut values = [0_u64; 4];
    for (value, name) in values
        .iter_mut()
        .zip(["m_programId", "m_region", "m_realm", "m_id"])
    {
        let decoded = traced_reflected_field(
            protocol,
            handle_type,
            name,
            reader,
            &format!("{path}.{name}"),
            depth + 1,
            provenance,
        )?;
        *value = match decoded {
            BsnValue::FourCc(value) => u64::from(value),
            value => expect_u64(value, "toon handle field")?,
        };
    }
    provenance.push(decoded_field(
        path,
        "ToonHandle",
        "4 fields",
        start_bit,
        reader.position(),
        depth,
    ));
    Ok(FriendIdentity::Character {
        program_id: u32::try_from(values[0]).expect("program id fits in u32"),
        region: u32::try_from(values[1]).expect("region fits in u32"),
        realm: u32::try_from(values[2]).expect("realm fits in u32"),
        id: values[3],
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the native string bounds are schema data"
)]
fn trace_optional_utf8(
    reader: &mut BitReader<'_>,
    provenance: &mut Vec<DecodedField>,
    path: &str,
    depth: usize,
    length_bits: usize,
    minimum_bytes: usize,
    maximum_bytes: usize,
    maximum_characters: usize,
) -> Result<Option<String>> {
    let start_bit = reader.position();
    let value = if reader.read(1)? != 0 {
        Some(decode_generated_utf8(
            reader,
            length_bits,
            minimum_bytes,
            maximum_bytes,
            maximum_characters,
        )?)
    } else {
        None
    };
    provenance.push(decoded_field(
        path,
        "optional string",
        value.clone().unwrap_or_else(|| "none".to_owned()),
        start_bit,
        reader.position(),
        depth,
    ));
    Ok(value)
}

fn trace_friend_custom_message(
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
    provenance: &mut Vec<DecodedField>,
) -> Result<()> {
    let start_bit = reader.position();
    let present = reader.read(1)? != 0;
    if present {
        let timestamp = read_spanned_i32(reader)?;
        let message = decode_spanned_generated_utf8(reader, 9, 0, 508, 127)?;
        provenance.extend([
            spanned_field(
                format!("{path}.custom_message.timestamp"),
                "int32",
                &timestamp,
                depth + 1,
            ),
            spanned_field(
                format!("{path}.custom_message.text"),
                "string",
                &message,
                depth + 1,
            ),
        ]);
    }
    provenance.push(decoded_field(
        format!("{path}.custom_message"),
        "optional",
        if present { "present" } else { "none" },
        start_bit,
        reader.position(),
        depth,
    ));
    Ok(())
}

fn full_name_from_value(value: &BsnValue) -> Option<String> {
    let value = unwrap_optional(value)?;
    let BsnValue::Struct(value) = value else {
        return None;
    };
    let given = value.get("m_givenName").and_then(bsn_string);
    let surname = value.get("m_surname").and_then(bsn_string);
    match (given, surname) {
        (Some(given), Some(surname)) => Some(format!("{given} {surname}")),
        (Some(given), None) => Some(given.to_owned()),
        (None, Some(surname)) => Some(surname.to_owned()),
        (None, None) => None,
    }
}

fn unwrap_optional(value: &BsnValue) -> Option<&BsnValue> {
    match value {
        BsnValue::Optional(Some(value)) => unwrap_optional(value),
        BsnValue::Optional(None) => None,
        value => Some(value),
    }
}

fn bsn_string(value: &BsnValue) -> Option<&str> {
    match unwrap_optional(value)? {
        BsnValue::String(value) => Some(value),
        _ => None,
    }
}

fn toon_full_name_from_value(value: &BsnValue) -> Result<ToonFullName> {
    let value = unwrap_optional(value)
        .ok_or_else(|| native_error("toon full name is absent"))?
        .as_struct()
        .ok_or_else(|| native_error("toon full name is not a struct"))?;
    let region = u8::try_from(expect_integer(
        value
            .get("m_region")
            .ok_or_else(|| native_error("toon full name omits its region"))?
            .clone(),
        "toon region",
    )?)
    .map_err(|_| native_error("toon region is outside u8"))?;
    let program_id = expect_u32(
        value
            .get("m_programId")
            .ok_or_else(|| native_error("toon full name omits its program"))?
            .clone(),
        "toon program",
    )?;
    let realm = expect_u32(
        value
            .get("m_realm")
            .ok_or_else(|| native_error("toon full name omits its realm"))?
            .clone(),
        "toon realm",
    )?;
    let name = value
        .get("m_name")
        .and_then(bsn_string)
        .ok_or_else(|| native_error("toon full name omits its name"))?
        .to_owned();
    Ok(ToonFullName {
        region,
        program_id,
        realm,
        name,
    })
}

fn toon_handle_from_value(value: &BsnValue) -> Result<Option<ToonHandle>> {
    let Some(value) = unwrap_optional(value) else {
        return Ok(None);
    };
    let value = value
        .as_struct()
        .ok_or_else(|| native_error("toon handle is not a struct"))?;
    let region = u8::try_from(expect_integer(
        value
            .get("m_region")
            .ok_or_else(|| native_error("toon handle omits its region"))?
            .clone(),
        "toon region",
    )?)
    .map_err(|_| native_error("toon region is outside u8"))?;
    let program_id = expect_u32(
        value
            .get("m_programId")
            .ok_or_else(|| native_error("toon handle omits its program"))?
            .clone(),
        "toon program",
    )?;
    let realm = expect_u32(
        value
            .get("m_realm")
            .ok_or_else(|| native_error("toon handle omits its realm"))?
            .clone(),
        "toon realm",
    )?;
    let id = expect_u64(
        value
            .get("m_id")
            .ok_or_else(|| native_error("toon handle omits its id"))?
            .clone(),
        "toon id",
    )?;
    Ok(Some(ToonHandle {
        region,
        program_id,
        realm,
        id,
    }))
}

fn profile_address_from_value(value: &BsnValue) -> Result<Option<ProfileAddress>> {
    let value =
        unwrap_optional(value).ok_or_else(|| native_error("friend profile address is absent"))?;
    let BsnValue::Struct(value) = value else {
        return Err(native_error("friend profile address is not a struct"));
    };
    let label = value
        .get("m_label")
        .cloned()
        .ok_or_else(|| native_error("friend profile address omitted its label"))?;
    let id = value
        .get("m_id")
        .cloned()
        .ok_or_else(|| native_error("friend profile address omitted its id"))?;
    let address = ProfileAddress {
        label: expect_u32(label, "friend profile label")?,
        id: expect_u64(id, "friend profile id")?,
    };
    Ok((address.label != 0 || address.id != 0).then_some(address))
}

fn optional_element(protocol: &Protocol, type_id: u32) -> Result<u32> {
    let type_id = protocol.peel_alias(type_id)?;
    let shape = protocol.codec().schema().shape(type_id)?;
    if shape.kind != TypeKind::Optional {
        return Err(native_error("expected an optional native field"));
    }
    shape
        .element_type
        .ok_or_else(|| native_error("optional native field has no element type"))
}

fn expect_integer(value: BsnValue, label: &str) -> Result<i128> {
    match value {
        BsnValue::Integer(value) => Ok(value),
        BsnValue::Optional(Some(value)) => expect_integer(*value, label),
        _ => Err(native_error(format!("{label} is not an integer"))),
    }
}

fn expect_u32(value: BsnValue, label: &str) -> Result<u32> {
    match value {
        BsnValue::FourCc(value) => Ok(value),
        value => u32::try_from(expect_integer(value, label)?)
            .map_err(|_| native_error(format!("{label} is outside u32"))),
    }
}

fn expect_u64(value: BsnValue, label: &str) -> Result<u64> {
    u64::try_from(expect_integer(value, label)?)
        .map_err(|_| native_error(format!("{label} is outside u64")))
}

fn expect_bytes(value: BsnValue, label: &str) -> Result<Vec<u8>> {
    match value {
        BsnValue::Bytes(value) => Ok(value),
        BsnValue::Optional(Some(value)) => expect_bytes(*value, label),
        _ => Err(native_error(format!("{label} is not bytes"))),
    }
}

fn decode_channel_name(reader: &mut BitReader<'_>) -> Result<(Option<u16>, u16)> {
    let shard_index = read_u16(reader, 16)?;
    reader.read(29)?;
    let public_id = match reader.read(2)? {
        2 => {
            reader.read(32)?;
            Some(read_u16(reader, 16)?)
        }
        1 | 3 => {
            reader.read(16)?;
            reader.read(32)?;
            None
        }
        0 => {
            decode_generated_utf8(reader, 7, 0, 124, 31)?;
            None
        }
        _ => unreachable!("two bits have only four values"),
    };
    Ok((public_id, shard_index))
}

fn decode_channel_config(reader: &mut BitReader<'_>) -> Result<()> {
    reader.read(8)?;
    reader.read(16)?;
    let programs = read_usize(reader, 3)?;
    if programs > 4 {
        return Err(native_error("channel config has too many programs"));
    }
    for _ in 0..programs {
        reader.read(32)?;
    }
    reader.read(32)?;
    let realms = read_usize(reader, 3)?;
    if realms > 4 {
        return Err(native_error("channel config has too many realms"));
    }
    for _ in 0..realms {
        reader.read(32)?;
    }
    reader.read(32)?;
    Ok(())
}

fn decode_profile_address(reader: &mut BitReader<'_>) -> Result<()> {
    reader.read(32)?;
    reader.read(64)?;
    Ok(())
}

fn decode_ranked_key(reader: &mut BitReader<'_>) -> Result<()> {
    reader.read(32)?;
    reader.read(16)?;
    reader.read(32)?;
    reader.read(2)?;
    Ok(())
}

fn decode_league_key(reader: &mut BitReader<'_>) -> Result<()> {
    decode_ranked_key(reader)?;
    reader.read(3)?;
    Ok(())
}

fn decode_ranked_matchmaker(reader: &mut BitReader<'_>) -> Result<()> {
    reader.read(64)?;
    reader.read(16)?;
    reader.read(19)?;
    reader.read(64)?;
    reader.read(16)?;
    reader.read(20)?;
    reader.read(16)?;
    decode_profile_address(reader)?;
    decode_ranked_key(reader)?;
    reader.read(32)?;
    Ok(())
}

fn decode_season_info(reader: &mut BitReader<'_>) -> Result<()> {
    reader.read(32)?;
    reader.read(10)?;
    if reader.read(1)? != 0 {
        reader.read(32)?;
    }
    reader.read(16)?;
    if reader.read(1)? != 0 {
        reader.read(16)?;
    }
    reader.read(32)?;
    reader.read(16)?;
    reader.read(16)?;
    reader.read(25)?;
    if reader.read(1)? != 0 {
        reader.read(16)?;
    }
    if reader.read(1)? != 0 {
        reader.read(32)?;
    }
    if reader.read(1)? != 0 {
        reader.read(16)?;
    }
    reader.read(64)?;
    Ok(())
}

fn read_usize(reader: &mut BitReader<'_>, width: usize) -> Result<usize> {
    usize::try_from(reader.read(width)?).map_err(|_| native_error("integer exceeds usize"))
}

fn read_u32(reader: &mut BitReader<'_>, width: usize) -> Result<u32> {
    u32::try_from(reader.read(width)?).map_err(|_| native_error("integer exceeds u32"))
}

fn read_u16(reader: &mut BitReader<'_>, width: usize) -> Result<u16> {
    u16::try_from(reader.read(width)?).map_err(|_| native_error("integer exceeds u16"))
}

fn read_u8(reader: &mut BitReader<'_>, width: usize) -> Result<u8> {
    u8::try_from(reader.read(width)?).map_err(|_| native_error("integer exceeds u8"))
}

fn native_error(message: impl Into<String>) -> Error {
    Error::Native(message.into())
}

#[cfg(test)]
mod tests {
    use crate::{
        bsn::{
            bits::BitWriter,
            value::{BsnField, BsnStruct},
        },
        native::{
            model::Payload,
            protocol::{CHAT_MESSAGE_COMMAND, CHAT_SLOT, Protocol},
        },
    };

    use super::*;

    fn protocol() -> Protocol {
        Protocol::current().unwrap()
    }

    #[test]
    fn toon_list_traces_every_physical_wire_field() {
        let protocol = protocol();
        let root_type = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Client::Toon::ToonList")
            .unwrap();
        let display_type = protocol
            .array_element(protocol.member_type(root_type, "m_toonDisplays").unwrap())
            .unwrap();
        let profile_type = protocol.member_type(display_type, "m_profile").unwrap();
        let realm_type = protocol.member_type(display_type, "m_realm").unwrap();
        let profile_type = protocol.peel_alias(profile_type).unwrap();
        let profile_shape = protocol.codec().schema().shape(profile_type).unwrap();
        let profile = BsnValue::Struct(BsnStruct::new(
            profile_type,
            vec![
                BsnField::named(
                    profile_shape.index_values[0],
                    "m_label",
                    BsnValue::Integer(0x1020_3040),
                ),
                BsnField::named(
                    profile_shape.index_values[1],
                    "m_id",
                    BsnValue::Integer(0x1122_3344_5566_7788),
                ),
            ],
        ));
        let mut writer = BitWriter::new();
        writer.write(1, 6).unwrap();
        writer.write(3, 7).unwrap();
        writer.align().unwrap();
        writer.write_bytes(b"Nova!", false).unwrap();
        writer
            .write(u64::from(123_i32.cast_unsigned() ^ 0x8000_0000), 32)
            .unwrap();
        writer.write(5, 3).unwrap();
        writer.write(0xa1b2_c3d4, 32).unwrap();
        protocol
            .codec()
            .encode_reflected_into(&mut writer, profile_type, &profile)
            .unwrap();
        protocol
            .codec()
            .encode_reflected_into(&mut writer, realm_type, &BsnValue::Integer(7))
            .unwrap();
        let logical_bits = writer.position();
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes, Some(logical_bits)).unwrap();

        let decoded = toon_list_with_provenance(&protocol, root_type, &mut reader).unwrap();
        let Payload::ToonList(list) = decoded.payload else {
            panic!("expected a toon list");
        };
        assert_eq!(list.displays[0].name, "Nova!");
        assert_eq!(list.displays[0].realm, 7);
        assert_eq!(reader.position(), logical_bits);

        for path in [
            "value.displays.count",
            "value.displays[0].name",
            "value.displays[0].last_online",
            "value.displays[0].wire_layout_selector",
            "value.displays[0].flags",
            "value.displays[0].profile.m_label",
            "value.displays[0].profile.m_id",
            "value.displays[0].realm",
        ] {
            let field = decoded
                .provenance
                .iter()
                .find(|field| field.path == path)
                .unwrap_or_else(|| panic!("missing {path}"));
            assert!(field.end_bit > field.start_bit, "{path}");
        }
        assert_eq!(
            decoded
                .provenance
                .iter()
                .find(|field| field.path == "value.displays[0].wire_layout_selector")
                .unwrap()
                .value,
            "5"
        );
        assert_eq!(
            decoded
                .provenance
                .iter()
                .find(|field| field.path == "value.displays[0].last_online")
                .unwrap()
                .value,
            "123"
        );
    }

    #[test]
    fn null_friend_profile_address_is_absent() {
        let address = BsnValue::Struct(BsnStruct::new(
            0,
            vec![
                BsnField::named(0, "m_label", BsnValue::Integer(0)),
                BsnField::named(1, "m_id", BsnValue::Integer(0)),
            ],
        ));

        assert_eq!(profile_address_from_value(&address).unwrap(), None);
    }

    #[test]
    fn retail_chat_message_decodes_at_the_exact_boundary() {
        let protocol = protocol();
        let packet = hex::decode(
            "4b0505012f0b04616e796f6f6e6520666f72206d75746174696f6e206f72206f6e65206d697373696f6e3f00",
        )
        .unwrap();
        let mut reader = BitReader::new(&packet, None).unwrap();
        reader.set_position(11).unwrap();
        let type_id = protocol
            .incoming_type(CHAT_SLOT, CHAT_MESSAGE_COMMAND)
            .unwrap();
        let Payload::ChatMessage(message) = chat_message(&protocol, type_id, &mut reader).unwrap()
        else {
            panic!("expected a chat message");
        };
        assert_eq!(reader.position() - 11, 336);
        assert_eq!(message.channel_index, 0);
        assert_eq!(message.member_handle, 2_623_867);
        assert_eq!(message.body, "anyoone for mutation or one mission?");
    }

    #[test]
    fn chat_invite_decodes_retail_generated_order() {
        let mut writer = BitWriter::new();
        writer.write(3, 4).unwrap();
        writer.write(0x1234_5678, 32).unwrap();
        writer.write(6, 3).unwrap();
        let logical_bits = writer.position();
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes, Some(logical_bits)).unwrap();

        let decoded = chat_invite_with_provenance(&mut reader).unwrap();
        let Payload::ChatInvite(invite) = decoded.payload else {
            panic!("expected a chat invite");
        };
        assert_eq!(
            invite,
            ChatInvite {
                channel_type: 3,
                inviter_presence: 0x1234_5678,
                channel_index: 6,
            }
        );
        assert_eq!(reader.position(), logical_bits);
        assert_eq!(
            decoded
                .provenance
                .iter()
                .find(|field| field.path == "value.m_channelIndex")
                .map(|field| (field.start_bit, field.end_bit)),
            Some((36, 39))
        );
    }

    #[test]
    fn retail_presence_update_decodes_online_at_the_exact_boundary() {
        let packet = hex::decode(
            "0c1c00019b9064019b906407030200000e6d000000000000000100005332525052530000ea707cc7ea707cc45804060020000100200005002003050020010200200103002001810104b46d0300",
        )
        .unwrap();
        let mut reader = BitReader::new(&packet, Some(609)).unwrap();
        reader.set_position(3).unwrap();

        let Payload::PresenceUpdate(update) = presence_update(&mut reader).unwrap() else {
            panic!("expected a presence update");
        };
        assert_eq!(reader.position() - 3, 606);
        assert!(update.online);
        assert_eq!(update.local_presence_id, 13_486_180);
        assert_eq!(update.master_presence_id, 13_486_180);
        assert_eq!(update.handles.len(), 6);
    }

    #[test]
    fn retail_friend_toon_decodes_generated_order_at_the_exact_boundary() {
        let protocol = protocol();
        let packet = hex::decode(
            "460301010014cc0200000011004563686f657323323935cafebabe7f1884100000000002fe223701",
        )
        .unwrap();
        let mut reader = BitReader::new(&packet, None).unwrap();
        reader.set_position(11).unwrap();

        let Payload::FriendToons(page) = friend_toons(&protocol, 2_770, &mut reader).unwrap()
        else {
            panic!("expected a friend-toon page");
        };
        assert!(page.complete);
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].account_id, 50_209_335);
        assert_eq!(page.entries[0].program_id, crate::bgs::fourcc("S2"));
        assert_eq!(
            page.entries[0].toon_name,
            ToonFullName {
                region: 1,
                program_id: crate::bgs::fourcc("S2"),
                realm: 1,
                name: "Echoes#295".into(),
            }
        );
        assert_eq!(
            page.entries[0].profile,
            Some(ProfileAddress {
                label: 0xcafe_babe,
                id: 0x7f18_8410_0000_0000,
            })
        );
        assert_eq!(reader.position(), 313);
    }

    #[test]
    fn retail_profile_read_start_decodes_at_the_exact_boundary() {
        let protocol = protocol();
        let packet = hex::decode("06000000010000c00100000000").unwrap();
        let mut reader = BitReader::new(&packet, Some(101)).unwrap();
        reader.set_position(3).unwrap();
        let result_type = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Profile::ProfileDataResponse")
            .unwrap();
        let token_type = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Token")
            .unwrap();
        let Payload::ProfileRead(response) =
            profile_read(&protocol, result_type, token_type, &mut reader).unwrap()
        else {
            panic!("expected a profile read response");
        };
        assert_eq!(reader.position() - 3, 98);
        assert_eq!(response.request_id, 0);
        assert_eq!(
            response.result,
            ProfileReadResult::Start {
                packet_count: 1,
                record_type: 6145,
            }
        );
    }

    #[test]
    fn non_empty_account_block_page_uses_the_retail_generated_order() {
        let protocol = protocol();
        let mut writer = BitWriter::new();
        writer.write(1, 1).unwrap();
        writer.write(1, 1).unwrap();
        writer.write(1, 7).unwrap();
        writer.write(0x155, 9).unwrap();
        writer.write(0x1020_3040, 32).unwrap();
        writer.write(1, 1).unwrap();
        writer.write(7, 7).unwrap();
        writer.write_bytes(b"Blocked", true).unwrap();
        writer.write(0xabcde, 20).unwrap();
        writer.write(1, 1).unwrap();
        writer.write(3, 8).unwrap();
        writer.write_bytes("李".as_bytes(), true).unwrap();
        writer.write(6, 8).unwrap();
        writer.write_bytes("Pérez".as_bytes(), true).unwrap();
        writer.write(3, 32).unwrap();
        let expected_bits = writer.position();
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes, Some(expected_bits)).unwrap();

        let Payload::AccountBlocks(page) = account_blocks(&protocol, &mut reader).unwrap() else {
            panic!("expected an account-block page");
        };
        assert_eq!(page.complete, Some(true));
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].account_id, 0x1020_3040);
        assert_eq!(page.entries[0].nickname.as_deref(), Some("Blocked"));
        assert_eq!(page.entries[0].full_name.as_deref(), Some("李 Pérez"));
        assert_eq!(page.entries[0].role, 3);
        assert_eq!(reader.position(), expected_bits);
    }

    #[test]
    fn cache_response_generated_order_round_trips() {
        let mut writer = BitWriter::new();
        writer.write(2, 6).unwrap();
        for (publication_time, fill) in [(123_i32, 0x11), (-7, 0x22)] {
            writer.write(0x12_3456, 23).unwrap();
            writer.write_bytes(&[fill; 40], true).unwrap();
            writer
                .write(
                    u64::from(publication_time.cast_unsigned() ^ 0x8000_0000),
                    32,
                )
                .unwrap();
        }
        writer.write(99, 32).unwrap();
        writer.write(7, 16).unwrap();
        writer.write(3, 16).unwrap();
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes, None).unwrap();
        let Payload::CacheStreamItems(response) = cache_stream_items(&mut reader).unwrap() else {
            panic!("expected cache response");
        };
        assert_eq!(response.items[0].publication_time, 123);
        assert_eq!(response.items[1].publication_time, -7);
        assert_eq!(response.total_items, 7);
        assert_eq!(response.offset, 3);
    }

    #[test]
    fn conference_member_counts_carry_population_and_fullness() {
        let mut writer = BitWriter::new();
        writer.write(1, 1).unwrap();
        writer.write(0x12_3456, 27).unwrap();
        writer.write(1, 1).unwrap();
        writer.write(0x1020_3040, 32).unwrap();
        writer.write(2, 6).unwrap();
        for (conference_id, members, full) in [(102_192_u64, 24_u64, false), (102_194, 181, true)] {
            writer.write(0x65_4321, 23).unwrap();
            writer.write(conference_id, 32).unwrap();
            writer.write(members, 16).unwrap();
            writer.write(u64::from(full), 1).unwrap();
        }
        let expected_bits = writer.position();
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes, Some(expected_bits)).unwrap();
        let Payload::ConferenceMemberCounts(page) = conference_member_counts(&mut reader).unwrap()
        else {
            panic!("expected conference member counts");
        };
        assert!(page.is_last);
        assert_eq!(page.sampled_at, Some(0x1020_3040 ^ i32::MIN));
        assert_eq!(
            page.entries,
            vec![
                ConferenceMemberCount {
                    conference_id: 102_192,
                    members: 24,
                    full: false,
                },
                ConferenceMemberCount {
                    conference_id: 102_194,
                    members: 181,
                    full: true,
                },
            ]
        );
        assert_eq!(reader.position(), expected_bits);
    }

    /// a retail `Chat/24` page captured from SC2 itself (2026-08-13), stored as
    /// it came off the wire.
    ///
    /// this is what proves the route is `ConferenceDescriptions`: the entries
    /// name real public channels, and their count matches the header exactly —
    /// which only holds when `m_id` is read as the *first* field of an entry
    /// rather than the last. reading the naming fields as belonging to the
    /// entry they follow shifts every conference by one position and loses the
    /// first entry entirely.
    #[test]
    fn retail_conference_descriptions_name_every_public_channel() {
        const PAYLOAD_START: usize = 3;
        const PAYLOAD_BITS: usize = 13882;
        let bytes = conference_descriptions_vector();
        let mut reader = BitReader::new(&bytes, Some(PAYLOAD_START + PAYLOAD_BITS)).unwrap();
        reader.set_position(PAYLOAD_START).unwrap();
        let Payload::ConferenceDescriptions(page) = conference_descriptions(&mut reader).unwrap()
        else {
            panic!("expected conference descriptions");
        };

        assert!(page.is_last);
        assert_eq!(page.entries.len(), 39);

        let named = |conference_id: u32| {
            page.entries
                .iter()
                .find(|entry| entry.conference_id == conference_id)
                .map(|entry| (entry.locale_tag(), entry.channel_name_id, entry.sort_order))
        };
        // general is enUS 1028 and sorts first; co-op is 1033 and sorts fifth
        assert_eq!(named(129_215), Some(("enUS".to_owned(), 1028, 1)));
        assert_eq!(named(102_194), Some(("enUS".to_owned(), 1028, 1)));
        assert_eq!(named(102_214), Some(("enUS".to_owned(), 1033, 5)));

        // a channel is served by several conferences, and the same channel
        // exists per locale — which is what the counts have to be grouped by
        // a busy channel is spread across numbered rooms — general, general 2,
        // and so on — which is what the catalogue's `%d` is a placeholder for
        let general = page
            .entries
            .iter()
            .filter(|entry| entry.channel_name_id == 1028 && entry.locale_tag() == "enUS")
            .map(|entry| entry.shard)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(general, (1..=12).collect::<std::collections::BTreeSet<u16>>());
        // a quiet channel has only its first room
        let cow = page
            .entries
            .iter()
            .filter(|entry| entry.channel_name_id == 1029 && entry.locale_tag() == "enUS")
            .map(|entry| entry.shard)
            .collect::<Vec<_>>();
        assert_eq!(cow, vec![1]);
        let locales = page
            .entries
            .iter()
            .map(ConferenceDescription::locale_tag)
            .collect::<std::collections::BTreeSet<_>>();
        assert!(locales.contains("enUS") && locales.len() > 1);
        // only a handful of catalogued channels exist as live rooms at all
        let channels = page
            .entries
            .iter()
            .map(|entry| entry.channel_name_id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(channels.len(), 6);
        // capacity and fill target come from the configuration now, rather than
        // being inferred from watching m_isFull flip
        assert!(page.entries.iter().all(|entry| entry.max_members == 200));
        let cow = page
            .entries
            .iter()
            .find(|entry| entry.channel_name_id == 1029)
            .expect("The Cow Level is listed");
        assert!((cow.target_proportion() - 0.9).abs() < f32::EPSILON);
        // 200 seats at 0.9 is why the counts page reports full at 181
        assert_eq!(cow.full_at(), 180);
        // the walk is arithmetic — every entry's length is computed, so landing
        // on the payload's last bit is the proof the layout is right
        assert_eq!(reader.position(), PAYLOAD_START + PAYLOAD_BITS);
    }

    /// the same retail page decoded a second way — through the codec, walking
    /// the registered wire layouts — and checked against the hand-written
    /// decoder field for field.
    ///
    /// the two share no code: one is a bit reader in this file, the other is
    /// the generic reflection walker driven by the permutations in
    /// `wire_layout.rs`. agreeing on all 39 entries of a 13882-bit page, and
    /// both landing on its final bit, is what makes the recovered field order
    /// evidence rather than a plausible story.
    #[test]
    fn the_registered_wire_layout_agrees_with_the_hand_written_decoder() {
        let protocol = Protocol::current().unwrap();
        let bytes = conference_descriptions_vector();
        let type_id = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Client::Chat::ConferenceDescriptions")
            .unwrap();

        let mut reader = BitReader::new(&bytes, Some(3 + 13882)).unwrap();
        reader.set_position(3).unwrap();
        let reflected = protocol.codec().decode_from(&mut reader, type_id).unwrap();
        assert_eq!(reader.position(), 3 + 13882, "the layout consumes the page");

        let mut reader = BitReader::new(&bytes, Some(3 + 13882)).unwrap();
        reader.set_position(3).unwrap();
        let Payload::ConferenceDescriptions(page) = conference_descriptions(&mut reader).unwrap()
        else {
            panic!("expected conference descriptions");
        };

        let BsnValue::Struct(root) = &reflected else {
            panic!("expected a struct");
        };
        let Some(BsnValue::Array(list)) = root.get("m_list") else {
            panic!("expected m_list");
        };
        assert_eq!(list.len(), page.entries.len());
        for (value, entry) in list.iter().zip(&page.entries) {
            let BsnValue::Struct(described) = value else {
                panic!("expected a conference");
            };
            let integer = |name: &str| match described.get(name) {
                Some(BsnValue::Integer(number)) => u32::try_from(*number).ok(),
                _ => None,
            };
            assert_eq!(integer("m_id"), Some(entry.conference_id));
            assert_eq!(integer("m_sortOrder"), Some(u32::from(entry.sort_order)));
        }
    }

    /// a retail `Chat/26` record captured from SC2 itself (2026-08-03), stored
    /// exactly as it came off the wire — the payload starts three bits into the
    /// first byte, where the routing header left it.
    ///
    /// this is the evidence that the route is `ConferenceMemberCounts` and not
    /// the channel directory it was long labelled as: the counts move between
    /// samples, and every conference the server marks full sits at the same
    /// cap, which a sort order would never do.
    #[test]
    fn retail_conference_member_counts_decode_at_the_exact_boundary() {
        const PAYLOAD_START: usize = 3;
        const PAYLOAD_BITS: usize = 2011;
        let bytes = hex::decode(
            "0d00ba80ea6fa771db0ca311000c791800d8c93800000afe0e053518d805000c791d01ded160\
         01000c791205f5f10805000c791000d0c8f800000c791700c1c93000000c791405f5f1180500\
         08360f0343c8d003000c7a0200c0d08800000c7a0005d4f87805000c7a0405d4f89805000c79\
         1a05f5f14805000c791903cae14003000c791f00c0c97000000c791e05f5f168050008421003\
         9828d803000c791c01d4d15801000c791300d4c91000000c791100c0c90000000fb7100040b9\
         1000000c791600cdc92800000c791500cbc92000000c7a0300c0d09000000c7a0100c0d08000\
         000c7a0604dff0a804000c7a0505d4f8a005000c791b0001",
        )
        .expect("vector is hex");
        let mut reader = BitReader::new(&bytes, Some(PAYLOAD_START + PAYLOAD_BITS)).unwrap();
        reader.set_position(PAYLOAD_START).unwrap();
        let Payload::ConferenceMemberCounts(page) = conference_member_counts(&mut reader).unwrap()
        else {
            panic!("expected conference member counts");
        };

        assert!(page.is_last);
        assert_eq!(page.sampled_at, Some(1_785_702_257));
        assert_eq!(page.entries.len(), 27);
        assert_eq!(
            page.entries[..3],
            [
                ConferenceMemberCount {
                    conference_id: 102_200,
                    members: 24,
                    full: false,
                },
                ConferenceMemberCount {
                    conference_id: 90_062,
                    members: 181,
                    full: true,
                },
                ConferenceMemberCount {
                    conference_id: 102_205,
                    members: 62,
                    full: false,
                },
            ]
        );
        // the server flips `m_isFull` at a fixed cap, so every full conference
        // reports the same population — the invariant that identifies the field
        let capped = page
            .entries
            .iter()
            .filter(|entry| entry.full)
            .map(|entry| entry.members)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(capped, std::collections::BTreeSet::from([181]));
        assert!(page.entries.iter().all(|entry| entry.members <= 181));
        assert_eq!(reader.position(), PAYLOAD_START + PAYLOAD_BITS);
    }

    fn conference_descriptions_vector() -> Vec<u8> {
        hex::decode(
            "9d030063cbca033244432b932ffb333346000000010000000200000000000100081041ca\
             e69a58080a01000600031e2c1e014849195c9b3fd999990a000000010000000200000000\
             0001000080685734d50840080100040018f2ff020c280000a63290cae46f7ecccc660200\
             0000010000000200000000000100048064b72aa905020001000100c79494006404000533\
             428657263ff6666626000000010000000200000000000100002060c1d109121004010001\
             00063c273c03084932b9373fb3333304000000010000000200000000000100010064adca\
             aa0381020100060031e5e10519482195c92ffd9999660000000001000001000000000000\
             01000880656e5553040801000400018f2a2a00c802000a664a0cae4d3feccccc16000000\
             0100000002000000000001000040702b9a6a182004010001000c796d1906284865726f3f\
             666666020000000100000002000000000001000200721d10940602020101020063cacb02\
             3244432b932ffb333346000000010000000200000000000100081041e0e8845208080100\
             0400020d4f0d0148010014cc3ed999990a00000001000000020000000000030000806856\
             e5550340090100050015fcfe0c0c180000a6327ecccc6602000000010000000200000000\
             010102048164b72aa905030001010100842828006404000533428657263ff66666260000\
             0001000000020000000000070000206095b95513100401000100063d043d034800002999\
             4832b9373fb3333304000000010000000200000000000800010064adcaaa038004010001\
             0031e7e407190800014c32fd99996600000000010000010000000000000100088065734d\
             58040601000300018f383800c802000a664a0cae4d3feccccc1600000001000000020000\
             00000004000040702b72aa132004010001000c7a631a0648000053324865726f3f666666\
             020000000100000002000000000101010201665b95540701000100010063d0d100320800\
             029952432b932ffb333346000000010000000200000000000600301046cadcaa53080401\
             000100031e421e0148020014cc4a195c9b3fd999990a0000000100000002000000000002\
             000080685734d50840040100010018f3f7030c180000a6327ecccc660200000001000000\
             0200000000000200048064b72aa905030001010100c79b9a01640200053332f666662600\
             0000010000000200000000000100002060c1d10912100901000500063c333c0348000029\
             994832b9373fb33333040000000100000002000000000001000100740e884a0280040100\
             010031e6e106191000014c722195c92ffd99996600000000010000010000000000010004\
             0881656e5553040401000100018f454500c801000a663aeccccc16000000010000000200\
             0000000004000040702b72aa132006010003000c7a661a0628000053323f666666020000\
             000100000002000000000004000200665b95540702010101010063cecd02320800029952\
             432b932ffb333346000000010000000200000000000100081041cae69a58080401000100\
             031e3f1e0148010014cc3ed999990a00000001000000020000000100014000c06856e555\
             0340050106040018f3f4030c180000a6327ecccc66020000000100000002000000000003\
             00048064b72aa905030001010100c79d9c016404000533428657263ff666662600000001\
             000000020000000000050000206095b95513100401000100063c323c0348000029994832\
             b9373fb3333304000000010000000200000000000100010064adcaaa0380040100010031\
             e6e006190800014c32fd999966000000000100000100000000000001000880656e555304\
             0601000300018f3d3d00c801000a663aeccccc1600000001000000020000000000010000\
             40702b9a6a182009010005000c797e190648000053324865726f3f666666020000000100\
             000002000000000003000200665b95540701000100010063cdcd01320400029932fb3333\
             46000000010000000200000000000100081041e0e88452080601000300031e401e014801\
             0014cc3ed999990a00000001000000020000000000010000806856e55503400901000500\
             18f3fb030c180000a6327ecccc6602000000010000000200000000000200048064b72aa9\
             05040101020100fb787800640200053332f6666626000000010000000200000000000500\
             00206095b9551310090100050007e2ff220348000029994832b9373fb333330400000001\
             0000000200000000000a00010064adcaaa038004010001003f181800191000014c722195\
             c92ffd999966000000000100000100000000000501140885656e555304040100010001f8\
             c1c100c802000a664a0cae4d3feccccc16000000010000000200000000000c000040702b\
             72aa132004010001",
        )
        .expect("vector is hex")
    }

    fn field(name: &str, value: BsnValue) -> BsnField {
        BsnField::named(0, name.to_owned(), value)
    }

    fn club_struct(fields: Vec<BsnField>) -> BsnValue {
        BsnValue::Struct(BsnStruct::new(0, fields))
    }

    #[test]
    fn club_info_yields_both_the_roster_total_and_who_is_online() {
        let summary = club_struct(vec![
            field("m_id", BsnValue::Integer(535_225)),
            field("m_name", BsnValue::String("cecw".to_owned())),
            field("m_memberCount", BsnValue::Integer(230)),
        ]);
        let status = club_struct(vec![
            field("m_online", BsnValue::Integer(14)),
            field("m_ingame", BsnValue::Integer(6)),
            field("m_inchat", BsnValue::Integer(9)),
        ]);
        let response = club_struct(vec![field(
            "m_clubs",
            BsnValue::Array(vec![club_struct(vec![
                field("m_summary", summary),
                field("m_status", status),
            ])]),
        )]);

        let mut counts = BTreeMap::new();
        collect_club_counts(&response, &mut counts);
        let found = counts.get(&535_225).copied().expect("club is counted");
        assert_eq!(found.member_count, Some(230));
        assert_eq!(found.online, Some(14));
    }

    #[test]
    fn a_bare_summary_reports_the_roster_but_not_the_room() {
        let response = club_struct(vec![field(
            "m_clubs",
            BsnValue::Array(vec![club_struct(vec![
                field("m_id", BsnValue::Integer(7)),
                field("m_memberCount", BsnValue::Integer(42)),
            ])]),
        )]);

        let mut counts = BTreeMap::new();
        collect_club_counts(&response, &mut counts);
        let found = counts.get(&7).copied().expect("club is counted");
        assert_eq!(found.member_count, Some(42));
        assert_eq!(found.online, None);
    }

    #[test]
    fn harvested_counts_reach_the_clubs_they_belong_to() {
        let mut clubs = vec![
            ClubSummary {
                club_id: 7,
                name: Some("Night Owls".to_owned()),
                kind: 1,
                category: 1,
                private: false,
                member_count: None,
                online: None,
            },
            ClubSummary {
                club_id: 9,
                name: Some("Unlisted".to_owned()),
                kind: 1,
                category: 1,
                private: false,
                member_count: None,
                online: None,
            },
        ];
        let counts = BTreeMap::from([(
            7,
            ClubCounts {
                member_count: Some(42),
                online: Some(5),
            },
        )]);

        apply_club_counts(&mut clubs, &counts);
        assert_eq!(clubs[0].member_count, Some(42));
        assert_eq!(clubs[0].online, Some(5));
        // a club the walk never named keeps its unknowns rather than a zero
        assert_eq!(clubs[1].member_count, None);
        assert_eq!(clubs[1].online, None);
    }
}
