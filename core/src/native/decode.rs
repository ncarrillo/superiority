use crate::{
    Error, Result,
    bsn::bits::BitReader,
    bsn::value::BsnValue,
    metadata::{TypeKind, TypeShape},
    native::model::{
        AccountBlockEntry, AccountBlockPage, CacheStreamItem, CacheStreamItems, ChannelDescriptor,
        ChannelList, ChatInvite, ChatJoin, ChatMembership, ChatMessage, ChatWhisper,
        ClubInviteAction, ClubSummary, ConferenceDescription, ConferenceDescriptions,
        ConnectionMessageFrame, FriendEntry, FriendIdentity, FriendToon, FriendToonPage,
        FriendUpdate, FriendsPage, MemberClanTag, MembershipChange, MembershipKind, Payload,
        PresenceField, PresenceFieldFlags, PresenceFields, PresenceUpdate, ProfileAddress,
        ProfileReadResponse, ProfileReadResult, SocialOperation, StartupSummary, ToonDisplay,
        ToonFullName, ToonHandle, ToonList, ToonNameResolved, ToonSelected,
    },
    native::protocol::Protocol,
};

pub(crate) fn message_frame(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let payload_type = protocol.member_type(type_id, "m_payload")?;
    let frame_type_type = protocol.member_type(type_id, "m_frameType")?;
    let headers_type = protocol.member_type(type_id, "m_headers")?;
    let payload = expect_bytes(
        protocol
            .codec()
            .decode_reflected_from(reader, payload_type)?,
        "message frame payload",
    )?;
    let frame_type = expect_integer(
        protocol
            .codec()
            .decode_reflected_from(reader, frame_type_type)?,
        "message frame type",
    )?;
    let headers = match protocol
        .codec()
        .decode_reflected_from(reader, headers_type)?
    {
        BsnValue::Array(values) => values
            .into_iter()
            .map(|value| match value {
                BsnValue::Struct(value) => Ok(value),
                _ => Err(native_error("message frame header is not a struct")),
            })
            .collect::<Result<Vec<_>>>()?,
        _ => return Err(native_error("message frame headers are not an array")),
    };
    Ok(Payload::MessageFrame(ConnectionMessageFrame {
        frame_type,
        headers,
        payload,
    }))
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

pub(crate) fn profile_read(
    protocol: &Protocol,
    result_type: u32,
    token_type: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let result = protocol
        .codec()
        .decode_reflected_from(reader, result_type)?;
    let request_id = u32::try_from(expect_integer(
        protocol.codec().decode_reflected_from(reader, token_type)?,
        "profile read request id",
    )?)
    .map_err(|_| native_error("profile read request id is outside u32"))?;
    let BsnValue::Choice { index, value } = result else {
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
    Ok(Payload::ProfileRead(ProfileReadResponse {
        request_id,
        result,
    }))
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

pub(crate) fn toon_list(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let displays_field = protocol.member_type(type_id, "m_toonDisplays")?;
    let display_type = protocol.array_element(displays_field)?;
    let profile_type = protocol.member_type(display_type, "m_profile")?;
    let realm_type = protocol.member_type(display_type, "m_realm")?;
    let count = read_usize(reader, 6)?;
    if count > 50 {
        return Err(native_error("native ToonList contains too many displays"));
    }
    let mut displays = Vec::with_capacity(count);
    for _ in 0..count {
        let name = decode_generated_utf8(reader, 7, 2, 100, 25)?;
        decode_i32(reader)?;
        reader.read(3)?;
        reader.read(32)?;
        protocol
            .codec()
            .decode_reflected_from(reader, profile_type)?;
        let realm = u32::try_from(expect_integer(
            protocol.codec().decode_reflected_from(reader, realm_type)?,
            "toon realm",
        )?)
        .map_err(|_| native_error("toon realm is outside u32"))?;
        displays.push(ToonDisplay { name, realm });
    }
    Ok(Payload::ToonList(ToonList { displays }))
}

pub(crate) fn toon_selected(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let record_address = protocol.member_type(type_id, "m_recordAddress")?;
    let handle_type = protocol.member_type(type_id, "m_toonHandle")?;
    let realm_type = protocol.member_type(type_id, "m_realm")?;
    let last_logon_type = protocol.member_type(type_id, "m_lastLogon")?;
    protocol
        .codec()
        .decode_reflected_from(reader, record_address)?;
    let mut fields = [0_u64; 4];
    for (slot, name) in fields
        .iter_mut()
        .zip(["m_programId", "m_region", "m_realm", "m_id"])
    {
        let field_type = protocol.member_type(handle_type, name)?;
        let value = protocol.codec().decode_reflected_from(reader, field_type)?;
        *slot = match value {
            BsnValue::FourCc(code) => u64::from(code),
            other => u64::try_from(expect_integer(other, "selected toon handle")?)
                .map_err(|_| native_error("selected toon handle field is outside u64"))?,
        };
    }
    let handle = ToonHandle {
        program_id: u32::try_from(fields[0])
            .map_err(|_| native_error("selected toon program id is outside u32"))?,
        region: u8::try_from(fields[1])
            .map_err(|_| native_error("selected toon region is outside u8"))?,
        realm: u32::try_from(fields[2])
            .map_err(|_| native_error("selected toon realm is outside u32"))?,
        id: fields[3],
    };
    let realm = u32::try_from(expect_integer(
        protocol.codec().decode_reflected_from(reader, realm_type)?,
        "selected toon realm",
    )?)
    .map_err(|_| native_error("selected toon realm is outside u32"))?;
    protocol
        .codec()
        .decode_reflected_from(reader, last_logon_type)?;
    let name = decode_generated_utf8(reader, 7, 2, 100, 25)?;
    Ok(Payload::ToonSelected(ToonSelected {
        name,
        realm,
        handle,
    }))
}

pub(crate) fn toon_welcome(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    decode_field(protocol, type_id, "m_depotRegion", reader)?;

    let achievements = protocol.member_type(type_id, "m_achievementHandles")?;
    let achievement_type = protocol.array_element(achievements)?;
    let achievement_count = read_usize(reader, 4)?;
    for _ in 0..achievement_count {
        decode_field(protocol, achievement_type, "m_handle", reader)?;
        decode_field(protocol, achievement_type, "m_program", reader)?;
    }
    decode_field(protocol, type_id, "m_isPlayingFromIGR", reader)?;
    decode_field(protocol, type_id, "m_defaultPortrait", reader)?;
    reader.read(31)?;
    for name in [
        "m_maxGameServerConnectTimeoutMS",
        "m_programName",
        "m_programFlags",
    ] {
        decode_field(protocol, type_id, name, reader)?;
    }

    let realms = protocol.member_type(type_id, "m_realmMapList")?;
    let realm_type = protocol.array_element(realms)?;
    let handle_type = protocol.member_type(realm_type, "m_to")?;
    let from_list = protocol.member_type(realm_type, "m_fromList")?;
    let from_type = protocol.array_element(from_list)?;
    let realm_count = read_usize(reader, 3)?;
    if realm_count > 4 {
        return Err(native_error("Welcome has too many realm mappings"));
    }
    for _ in 0..realm_count {
        decode_field(protocol, handle_type, "m_id", reader)?;
        decode_field(protocol, handle_type, "m_programId", reader)?;
        reader.read(1)?;
        decode_field(protocol, handle_type, "m_region", reader)?;
        let from_count = read_usize(reader, 5)?;
        if from_count > 16 {
            return Err(native_error("Welcome realm mapping is too large"));
        }
        for _ in 0..from_count {
            protocol.codec().decode_reflected_from(reader, from_type)?;
        }
    }

    let unlockables = protocol.member_type(type_id, "m_unlockablesFiles")?;
    let unlockable_type = protocol.array_element(unlockables)?;
    let handle = protocol.member_type(unlockable_type, "m_unlockDefinitionFileCacheHandle")?;
    let unlockable_count = read_usize(reader, 8)?;
    if unlockable_count > 128 {
        return Err(native_error("Welcome has too many unlockable files"));
    }
    for _ in 0..unlockable_count {
        reader.read(4)?;
        protocol.codec().decode_reflected_from(reader, handle)?;
    }
    reader.read(3)?;
    decode_field(protocol, type_id, "m_maxMapFavorites", reader)?;
    decode_generated_utf8(reader, 13, 0, 4096, 1024)?;
    decode_generated_utf8(reader, 13, 0, 4096, 1024)?;
    decode_field(protocol, type_id, "m_currentTime", reader)?;
    Ok(Payload::StartupSummary(StartupSummary {
        kind: "toon-welcome",
        item_count: achievement_count + realm_count + unlockable_count,
        complete: None,
    }))
}

pub(crate) fn chat_message(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    decode_field(protocol, type_id, "Message", reader)?;
    let member_handle = u32::try_from(expect_integer(
        decode_field(protocol, type_id, "m_memberHandle", reader)?,
        "chat member handle",
    )?)
    .map_err(|_| native_error("chat member handle is outside u32"))?;
    let body = decode_generated_utf8(reader, 10, 0, 1020, 255)?;
    let channel_index = u8::try_from(expect_integer(
        decode_field(protocol, type_id, "m_channelIndex", reader)?,
        "chat channel index",
    )?)
    .map_err(|_| native_error("chat channel index is outside u8"))?;
    Ok(Payload::ChatMessage(ChatMessage {
        channel_index,
        member_handle,
        body,
    }))
}

pub(crate) fn chat_invite(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let BsnValue::Struct(invite) = protocol.codec().decode_reflected_from(reader, type_id)? else {
        return Err(native_error("chat invite is not a struct"));
    };
    let channel_index = u8::try_from(expect_integer(
        invite
            .get("m_channelIndex")
            .ok_or_else(|| native_error("chat invite omits channel index"))?
            .clone(),
        "chat invite channel index",
    )?)
    .map_err(|_| native_error("chat invite channel index is outside u8"))?;
    let inviter_presence = u32::try_from(expect_integer(
        invite
            .get("m_inviterPresence")
            .ok_or_else(|| native_error("chat invite omits inviter presence"))?
            .clone(),
        "chat invite inviter presence",
    )?)
    .map_err(|_| native_error("chat invite inviter presence is outside u32"))?;
    Ok(Payload::ChatInvite(ChatInvite {
        channel_index,
        inviter_presence,
    }))
}

pub(crate) fn chat_whisper(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
    echo: bool,
) -> Result<Payload> {
    decode_field(protocol, type_id, "Whisper", reader)?;
    let peer = decode_chat_display_name(reader)?;
    let body = decode_generated_utf8(reader, 10, 0, 1020, 255)?;
    let whisper = ChatWhisper { peer, body };
    Ok(if echo {
        Payload::ChatWhisperEcho(whisper)
    } else {
        Payload::ChatWhisper(whisper)
    })
}

pub(crate) fn chat_membership(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let end_of_initial = reader.read(1)? != 0;
    let channel_index = read_u8(reader, 3)?;
    if usize::from(channel_index) >= crate::native::protocol::CHANNEL_INDEX_COUNT {
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

    let count = read_usize(reader, 6)? + 1;
    let mut changes = Vec::with_capacity(count);
    for _ in 0..count {
        match reader.read(2)? {
            0 => changes.push(MembershipChange {
                kind: MembershipKind::Leave,
                member_handle: read_u32(reader, 32)?,
                presence_id: None,
                display_name: None,
                toon_name: None,
                reason: Some(read_u16(reader, 16)?),
            }),
            1 => {
                let member_handle = read_u32(reader, 32)?;
                let presence_id = read_u32(reader, 32)?;
                let status_count = read_usize(reader, 3)?;
                let mut display_name = None;
                let mut toon_name = None;
                for _ in 0..status_count {
                    if let Some(name) = decode_member_status(protocol, &status_shape, reader)? {
                        display_name = Some(name.name.clone());
                        toon_name = Some(name);
                    }
                }
                changes.push(MembershipChange {
                    kind: MembershipKind::Join,
                    member_handle,
                    presence_id: Some(presence_id),
                    display_name,
                    toon_name,
                    reason: None,
                });
            }
            2 => {
                let member_handle = read_u32(reader, 32)?;
                let toon_name = decode_member_status(protocol, &status_shape, reader)?;
                changes.push(MembershipChange {
                    kind: MembershipKind::Status,
                    member_handle,
                    presence_id: None,
                    display_name: toon_name.as_ref().map(|name| name.name.clone()),
                    toon_name,
                    reason: None,
                });
            }
            _ => return Err(native_error("chat membership change has an unknown choice")),
        }
    }
    Ok(Payload::ChatMembership(ChatMembership {
        channel_index,
        end_of_initial,
        changes,
    }))
}

pub(crate) fn friends_list(protocol: &Protocol, reader: &mut BitReader<'_>) -> Result<Payload> {
    let complete = if reader.read(1)? == 0 {
        None
    } else {
        Some(reader.read(1)? != 0)
    };
    let count = read_usize(reader, 7)?;
    if count > 64 {
        return Err(native_error("friends snapshot has too many updates"));
    }
    let mut updates = Vec::with_capacity(count);
    for _ in 0..count {
        let operation = match reader.read(2)? {
            0 => SocialOperation::Add,
            1 => {
                let identity = if reader.read(1)? == 0 {
                    FriendIdentity::Account(read_u32(reader, 32)?)
                } else {
                    decode_toon_handle(protocol, reader)?
                };
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
        updates.push(FriendUpdate {
            operation,
            entry: decode_friend_container(protocol, reader)?,
        });
    }
    Ok(Payload::Friends(FriendsPage { updates, complete }))
}

pub(crate) fn friend_toons(
    _protocol: &Protocol,
    _type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let count = read_usize(reader, 7)?;
    if count > 100 {
        return Err(native_error(
            "friend toon notification has too many entries",
        ));
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let region = read_u8(reader, 8)?;
        let program_id = read_u32(reader, 32)?;
        let realm = read_u32(reader, 32)?;
        let name = decode_generated_utf8(reader, 7, 2, 100, 25)?;
        let profile = ProfileAddress {
            label: read_u32(reader, 32)?,
            id: reader.read(64)?,
        };
        let account_id = read_u32(reader, 32)?;
        entries.push(FriendToon {
            account_id,
            program_id,
            profile: (profile.label != 0 || profile.id != 0).then_some(profile),
            toon_name: ToonFullName {
                region,
                program_id,
                realm,
                name,
            },
        });
    }
    let complete = reader.read(1)? != 0;
    Ok(Payload::FriendToons(FriendToonPage { entries, complete }))
}

pub(crate) fn profile_address_query(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let value = protocol.codec().decode_reflected_from(reader, type_id)?;
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
    Ok(Payload::ProfileAddressQuery(
        crate::native::model::ProfileAddressQueryResponse {
            request_id,
            address,
            error,
        },
    ))
}

pub(crate) fn account_blocks(protocol: &Protocol, reader: &mut BitReader<'_>) -> Result<Payload> {
    let complete = if reader.read(1)? == 0 {
        None
    } else {
        Some(reader.read(1)? != 0)
    };
    let count = read_usize(reader, 7)?;
    if count > 64 {
        return Err(native_error("account block snapshot is too large"));
    }
    let account_type = protocol
        .codec()
        .schema()
        .unique_type_id("Battlenet::Friends::AccountBlockContainer")?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        reader.read(9)?;
        let account_id = read_u32(reader, 32)?;
        let nickname = (reader.read(1)? != 0)
            .then(|| decode_generated_utf8(reader, 7, 0, 108, 27))
            .transpose()?;
        reader.read(20)?;
        let full_name = protocol.member_type(account_type, "m_fullName")?;
        let full_name = if reader.read(1)? != 0 {
            let element = optional_element(protocol, full_name)?;
            full_name_from_value(&protocol.codec().decode_reflected_from(reader, element)?)
        } else {
            None
        };
        let role = read_u32(reader, 32)?;
        entries.push(AccountBlockEntry {
            account_id,
            nickname,
            full_name,
            role,
        });
    }
    Ok(Payload::AccountBlocks(AccountBlockPage {
        entries,
        complete,
    }))
}

pub(crate) fn toon_blocks(reader: &mut BitReader<'_>) -> Result<Payload> {
    let count = read_usize(reader, 7)?;
    if count != 0 {
        return Err(native_error(
            "non-empty toon-block snapshots are not supported",
        ));
    }
    let complete = if reader.read(1)? == 0 {
        None
    } else {
        Some(reader.read(1)? != 0)
    };
    Ok(Payload::StartupSummary(StartupSummary {
        kind: "toon-blocks",
        item_count: 0,
        complete,
    }))
}

pub(crate) fn cache_stream_items(reader: &mut BitReader<'_>) -> Result<Payload> {
    let count = read_usize(reader, 6)?;
    if count > 49 {
        return Err(native_error("cache response contains too many items"));
    }
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        reader.read(23)?;
        let handle: [u8; 40] = reader
            .read_bytes(40, true)?
            .try_into()
            .expect("exactly forty bytes were read");
        let publication_time = decode_i32(reader)?;
        items.push(CacheStreamItem {
            publication_time,
            content_handle: handle,
        });
    }
    let token = read_u32(reader, 32)?;
    let total_items = read_u16(reader, 16)?;
    let offset = read_u16(reader, 16)?;
    Ok(Payload::CacheStreamItems(CacheStreamItems {
        items,
        offset,
        total_items,
        token,
    }))
}

pub(crate) fn conference_descriptions(reader: &mut BitReader<'_>) -> Result<Payload> {
    let is_last = reader.read(1)? != 0;
    reader.read(27)?;
    if reader.read(1)? != 0 {
        reader.read(32)?;
    }
    let count = read_usize(reader, 6)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        reader.read(23)?;
        entries.push(ConferenceDescription {
            identifier: read_u32(reader, 32)?,
            sort_order: read_u16(reader, 16)?,
            marker: reader.read(1)? != 0,
        });
    }
    Ok(Payload::ConferenceDescriptions(ConferenceDescriptions {
        entries,
        is_last,
    }))
}

pub(crate) fn channel_list(reader: &mut BitReader<'_>) -> Result<Payload> {
    reader.read(27)?;
    reader.read(1)?;
    reader.read(9)?;
    let count = read_usize(reader, 6)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let kind = read_u8(reader, 8)?;
        let index = read_u16(reader, 16)?;
        reader.read(24)?;
        entries.push(ChannelDescriptor {
            kind,
            index,
            identifier: read_u16(reader, 16)?,
        });
    }
    Ok(Payload::ChannelList(ChannelList { entries }))
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
        let channel_name_id = if reader.read(1)? == 0 {
            None
        } else {
            decode_channel_name(reader)?
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
        reason: Some(reason),
        token,
    }))
}

pub(crate) fn presence_fields(reader: &mut BitReader<'_>) -> Result<Payload> {
    let count = read_usize(reader, 7)?;
    if count > 100 {
        return Err(native_error(
            "presence announcement contains too many field definitions",
        ));
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let client_only = reader.read(1)? != 0;
        let writable = reader.read(1)? != 0;
        let ephemeral = reader.read(1)? != 0;
        let fixed_size = if reader.read(1)? == 0 {
            Some(read_u16(reader, 16)?)
        } else {
            None
        };
        let server_only = reader.read(1)? != 0;
        let identifier = read_u8(reader, 8)?;
        if identifier > 25 && identifier != u8::MAX {
            return Err(native_error(
                "presence announcement contains an unknown field type",
            ));
        }
        let mut flags = 0;
        if writable {
            flags |= PresenceFieldFlags::WRITABLE;
        }
        if ephemeral {
            flags |= PresenceFieldFlags::EPHEMERAL;
        }
        if server_only {
            flags |= PresenceFieldFlags::SERVER_ONLY;
        }
        if client_only {
            flags |= PresenceFieldFlags::CLIENT_ONLY;
        }
        entries.push(PresenceField {
            handle: read_u32(reader, 32)?,
            identifier,
            flags: PresenceFieldFlags::from_bits(flags),
            fixed_size,
        });
    }
    Ok(Payload::PresenceFields(PresenceFields { entries }))
}

pub(crate) fn presence_update(reader: &mut BitReader<'_>) -> Result<Payload> {
    reader.read(19)?;
    // the generated presence flag is zero while online.
    let online = reader.read(1)? == 0;
    let local_presence_id = read_u32(reader, 32)?;
    let master_presence_id = read_u32(reader, 32)?;
    let field_bytes = read_usize(reader, 11)?;
    if field_bytes > 1024 {
        return Err(native_error("presence update field data is too large"));
    }
    let field_data = reader.read_bytes(field_bytes, true)?;
    reader.read(11)?;
    let cleared_count = read_usize(reader, 4)?;
    let cleared_handles = (0..cleared_count)
        .map(|_| read_u32(reader, 32))
        .collect::<Result<Vec<_>>>()?;
    let handle_count = read_usize(reader, 4)?;
    let handles = (0..handle_count)
        .map(|_| read_u32(reader, 32))
        .collect::<Result<Vec<_>>>()?;
    let variable_size_count = read_usize(reader, 4)?;
    let variable_sizes = (0..variable_size_count)
        .map(|_| read_u16(reader, 16))
        .collect::<Result<Vec<_>>>()?;
    if reader.read(1)? != 0 {
        reader.read(1)?;
        read_u32(reader, 32)?;
    }
    reader.read(8)?;
    Ok(Payload::PresenceUpdate(PresenceUpdate {
        local_presence_id,
        master_presence_id,
        field_data,
        cleared_handles,
        handles,
        variable_sizes,
        online,
    }))
}

pub(crate) fn current_season(reader: &mut BitReader<'_>) -> Result<Payload> {
    if reader.read(1)? != 0 {
        let error = reader.read(16)?;
        return Ok(Payload::StartupSummary(StartupSummary {
            kind: "current-season-error",
            item_count: usize::try_from(error).expect("16-bit value fits usize"),
            complete: None,
        }));
    }
    let authority = reader.read(1)? != 0;
    let ranked = read_usize(reader, 7)?;
    if ranked > 100 {
        return Err(native_error("current season has too many matchmakers"));
    }
    for _ in 0..ranked {
        decode_ranked_matchmaker(reader)?;
    }
    let leagues = read_usize(reader, 9)?;
    if leagues > 448 {
        return Err(native_error("current season has too many leagues"));
    }
    for _ in 0..leagues {
        decode_profile_address(reader)?;
        reader.read(8)?;
        decode_league_key(reader)?;
    }
    decode_season_info(reader)?;
    let configurations = read_usize(reader, 9)?;
    if configurations > 448 {
        return Err(native_error(
            "current season has too many league configurations",
        ));
    }
    for _ in 0..configurations {
        decode_league_key(reader)?;
        reader.read(64)?;
        if reader.read(1)? != 0 {
            reader.read(32)?;
        }
    }
    Ok(Payload::StartupSummary(StartupSummary {
        kind: "current-season",
        item_count: ranked + leagues + configurations,
        complete: Some(authority),
    }))
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

pub(crate) fn club_info(
    protocol: &Protocol,
    type_id: u32,
    reader: &mut BitReader<'_>,
) -> Result<Payload> {
    let start = reader.position();
    let buffer_bits = reader.data().len() * 8;
    let walked_end = match protocol.codec().decode_from(reader, type_id) {
        Ok(_) => Some(reader.position()),
        Err(_) => None,
    };
    reader.set_position(start)?;
    let Ok((clubs, elements_end, incomplete)) =
        walk_club_elements(reader, walked_end.unwrap_or(buffer_bits))
    else {
        return Err(incomplete_club_frame(buffer_bits));
    };
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

fn walk_club_elements(
    reader: &mut BitReader<'_>,
    total: usize,
) -> Result<(Vec<ClubSummary>, usize, bool)> {
    let mut clubs = Vec::new();
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
        start = cursor + CLUB_ELEMENT_REMAINDER;
    }
    Ok((clubs, start, incomplete))
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
    reader.set_position(start)?;
    let Ok((clubs, start, incomplete)) = walk_club_elements(reader, total) else {
        return Err(incomplete_club_frame(buffer_bits));
    };
    if incomplete {
        return Err(incomplete_club_frame(buffer_bits));
    }
    // each summary ends with a u32, one rank byte per club, and a final flag.
    let tail = 32 + 8 + clubs.len() * 8 + 1;
    let end = (start + tail).div_ceil(8) * 8;
    if end > buffer_bits {
        return Err(incomplete_club_frame(buffer_bits));
    }
    reader.set_position(end)?;
    Ok(Payload::ClubSummaries(clubs))
}

pub(crate) fn club_settings(reader: &mut BitReader<'_>) -> Result<Payload> {
    decode_generated_utf8(reader, 13, 0, 4096, 1024)?;
    decode_generated_utf8(reader, 13, 0, 4096, 1024)?;
    reader.read(32)?;
    reader.read(32)?;
    decode_i32(reader)?;
    decode_i32(reader)?;
    decode_i32(reader)?;
    Ok(Payload::StartupSummary(StartupSummary {
        kind: "club-settings",
        item_count: 0,
        complete: None,
    }))
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

fn decode_field(
    protocol: &Protocol,
    struct_type: u32,
    name: &str,
    reader: &mut BitReader<'_>,
) -> Result<BsnValue> {
    let field_type = protocol.member_type(struct_type, name)?;
    let start_position = reader.position();
    protocol
        .codec()
        .decode_reflected_from(reader, field_type)
        .map_err(|error| match error {
            Error::IncompleteFrame(_) => error,
            Error::BsnWire(message) => {
                let struct_name = protocol
                    .codec()
                    .schema()
                    .type_metadata(struct_type)
                    .ok()
                    .and_then(|metadata| metadata.name)
                    .unwrap_or_else(|| "unnamed".to_owned());
                Error::BsnWire(format!(
                    "{message}; field {struct_name}.{name}, bits {start_position}..{}",
                    reader.position()
                ))
            }
            other => other,
        })
}

fn choice_variant(shape: &TypeShape, index: i128) -> Result<u32> {
    let position = shape
        .index_values
        .iter()
        .position(|candidate| *candidate == index)
        .ok_or_else(|| native_error(format!("choice has no variant {index}")))?;
    Ok(shape.member_types[position])
}

fn decode_member_status(
    protocol: &Protocol,
    status_shape: &TypeShape,
    reader: &mut BitReader<'_>,
) -> Result<Option<ToonFullName>> {
    let choice = i128::from(reader.read(3)?);
    let variant = choice_variant(status_shape, choice)?;
    match choice {
        0 | 1 => {
            protocol.codec().decode_reflected_from(reader, variant)?;
        }
        2 => {
            reader.read(8)?;
        }
        3 => {
            let wrapper = protocol.peel_alias(variant)?;
            let wrapper_shape = protocol.codec().schema().shape(wrapper)?;
            let inner = *wrapper_shape
                .member_types
                .first()
                .ok_or_else(|| native_error("talker status wrapper is empty"))?;
            let identifier = protocol.member_type(inner, "m_id")?;
            protocol.codec().decode_reflected_from(reader, identifier)?;
            reader.read(1)?;
        }
        4 | 6 => {
            reader.read(1)?;
        }
        5 => return Ok(Some(decode_chat_display_name(reader)?)),
        7 => {}
        _ => return Err(native_error("chat member status has an unknown choice")),
    }
    Ok(None)
}

fn decode_chat_display_name(reader: &mut BitReader<'_>) -> Result<ToonFullName> {
    Ok(ToonFullName {
        region: read_u8(reader, 8)?,
        program_id: read_u32(reader, 32)?,
        realm: read_u32(reader, 32)?,
        name: decode_generated_utf8(reader, 7, 2, 100, 25)?,
    })
}

fn decode_friend_container(protocol: &Protocol, reader: &mut BitReader<'_>) -> Result<FriendEntry> {
    match reader.read(2)? {
        0 => decode_friend_character(protocol, reader),
        1 => decode_friend_account(protocol, reader),
        2 => {
            let account_id = read_u32(reader, 32)?;
            if reader.read(1)? != 0 {
                decode_friend_custom_message(reader)?;
            }
            decode_i32(reader)?;
            Ok(FriendEntry {
                identity: FriendIdentity::Account(account_id),
                display_name: None,
                full_name: None,
                note: None,
                profile: None,
                toon_name: None,
            })
        }
        _ => Err(native_error(
            "friends update contains an unknown container choice",
        )),
    }
}

fn decode_friend_account(protocol: &Protocol, reader: &mut BitReader<'_>) -> Result<FriendEntry> {
    let account_type = protocol
        .codec()
        .schema()
        .unique_type_id("Battlenet::Friends::FriendContainer5::Account")?;
    let account_id = read_u32(reader, 32)?;
    let full_name = if reader.read(1)? != 0 {
        let full_name = protocol.member_type(account_type, "m_fullName")?;
        let element = optional_element(protocol, full_name)?;
        full_name_from_value(&protocol.codec().decode_reflected_from(reader, element)?)
    } else {
        None
    };
    let display_name = (reader.read(1)? != 0)
        .then(|| decode_generated_utf8(reader, 7, 0, 108, 27))
        .transpose()?;
    let profile =
        profile_address_from_value(&decode_field(protocol, account_type, "m_profile", reader)?)?;
    if reader.read(1)? != 0 {
        decode_friend_custom_message(reader)?;
    }
    let note = (reader.read(1)? != 0)
        .then(|| decode_generated_utf8(reader, 9, 0, 508, 127))
        .transpose()?;
    decode_i32(reader)?;
    reader.read(64)?;
    reader.read(32)?;
    Ok(FriendEntry {
        identity: FriendIdentity::Account(account_id),
        display_name,
        full_name,
        note,
        profile,
        toon_name: None,
    })
}

fn decode_friend_character(protocol: &Protocol, reader: &mut BitReader<'_>) -> Result<FriendEntry> {
    let character_type = protocol
        .codec()
        .schema()
        .unique_type_id("Battlenet::Friends::FriendContainer5::Character")?;
    let identity = decode_toon_handle(protocol, reader)?;
    let display_name = decode_generated_utf8(reader, 7, 2, 100, 25)?;
    let profile = profile_address_from_value(&decode_field(
        protocol,
        character_type,
        "m_profile",
        reader,
    )?)?;
    let note = (reader.read(1)? != 0)
        .then(|| decode_generated_utf8(reader, 9, 0, 508, 127))
        .transpose()?;
    Ok(FriendEntry {
        identity,
        display_name: Some(display_name),
        full_name: None,
        note,
        profile,
        toon_name: None,
    })
}

fn decode_toon_handle(protocol: &Protocol, reader: &mut BitReader<'_>) -> Result<FriendIdentity> {
    let selected_type = protocol.incoming_type(
        crate::native::protocol::TOON_SLOT,
        crate::native::protocol::TOON_SELECTED_COMMAND,
    )?;
    let handle_type = protocol.member_type(selected_type, "m_toonHandle")?;
    let program_id = expect_u32(
        decode_field(protocol, handle_type, "m_programId", reader)?,
        "toon program id",
    )?;
    let region = expect_u32(
        decode_field(protocol, handle_type, "m_region", reader)?,
        "toon region",
    )?;
    let realm = expect_u32(
        decode_field(protocol, handle_type, "m_realm", reader)?,
        "toon realm",
    )?;
    let id = expect_u64(
        decode_field(protocol, handle_type, "m_id", reader)?,
        "toon id",
    )?;
    Ok(FriendIdentity::Character {
        program_id,
        region,
        realm,
        id,
    })
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

fn decode_friend_custom_message(reader: &mut BitReader<'_>) -> Result<()> {
    decode_i32(reader)?;
    decode_generated_utf8(reader, 9, 0, 508, 127)?;
    Ok(())
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

fn decode_channel_name(reader: &mut BitReader<'_>) -> Result<Option<u16>> {
    reader.read(16)?;
    reader.read(29)?;
    match reader.read(2)? {
        2 => {
            reader.read(32)?;
            Ok(Some(read_u16(reader, 16)?))
        }
        1 | 3 => {
            reader.read(16)?;
            reader.read(32)?;
            Ok(None)
        }
        0 => {
            decode_generated_utf8(reader, 7, 0, 124, 31)?;
            Ok(None)
        }
        _ => unreachable!("two bits have only four values"),
    }
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

fn decode_i32(reader: &mut BitReader<'_>) -> Result<i32> {
    Ok(i32::from_ne_bytes(
        (read_u32(reader, 32)? ^ 0x8000_0000).to_ne_bytes(),
    ))
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
    fn fixed_chat_directory_decoders_preserve_public_fields() {
        let mut writer = BitWriter::new();
        writer.write(1, 1).unwrap();
        writer.write(0x12_3456, 27).unwrap();
        writer.write(0, 1).unwrap();
        writer.write(2, 6).unwrap();
        for (identifier, order, marker) in [(0x1020_3040, 3, false), (0x5060_7080, 9, true)] {
            writer.write(0x65_4321, 23).unwrap();
            writer.write(identifier, 32).unwrap();
            writer.write(order, 16).unwrap();
            writer.write(u64::from(marker), 1).unwrap();
        }
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes, None).unwrap();
        let Payload::ConferenceDescriptions(directory) =
            conference_descriptions(&mut reader).unwrap()
        else {
            panic!("expected conference descriptions");
        };
        assert!(directory.is_last);
        assert_eq!(directory.entries[0].identifier, 0x1020_3040);
        assert_eq!(directory.entries[1].sort_order, 9);
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
}
