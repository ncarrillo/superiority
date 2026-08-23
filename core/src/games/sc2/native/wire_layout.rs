use crate::{
    Error, Result,
    bsn::{
        bits::{BitReader, BitWriter},
        codec::{Codec, DecodedField, StructWireLayout, WireField, WireLayout},
        value::{BsnField, BsnStruct, BsnValue},
    },
};

const VERIFIED_REFLECTED: &[&str] = &[
    "Battlenet::Client::Achievement::ListenRequest",
    "Battlenet::Client::Chat::StatusChangeRequest",
    "Battlenet::Client::Chat::DatagramConnectionUpdate",
    "Battlenet::Client::Chat::WhisperRecv",
    "Battlenet::Client::Chat::WhisperEchoRecv",
    "Battlenet::Client::Connection::ServerVersion",
    "Battlenet::Client::Connection::RegulatorUpdate",
    "Battlenet::Client::Party::BeginReadyProcess",
    "Battlenet::Client::Party::ModifyMapOptions",
    "Battlenet::Client::Party::MapOptionsChange",
    "Battlenet::Client::Party::ReadyProcessUpdate",
    "Battlenet::Client::Presence::StatisticsUpdate",
    "Battlenet::Client::Presence::TemporaryPresenceResponse",
    "Battlenet::Client::Profile::SendStatsUIEvent",
    "Battlenet::Client::S2Master::MMQGetInfoRequest",
    "Battlenet::Client::S2Master::MMQGetListResponse",
    "Battlenet::Client::Toon::InitialNotifiesComplete",
];

const CANDIDATE_REFLECTED: &[&str] = &[
    "Battlenet::Client::Toon::CaisTimeUpdate",
    "Battlenet::Client::S2Master::MMQAnnounce",
    "Battlenet::MatchMaker::Announce",
    "Battlenet::MatchMaker::StaticInfo",
    "Battlenet::MatchMaker::HistogramSet",
    "Battlenet::MatchMaker::PerGameQueueInfo",
];

const IDENTITY_0: &[WireField] = &[];
const IDENTITY_1: &[WireField] = &[WireField::new(0, 0)];
/// `ConferenceDescriptions {m_list, m_isLast}` — the flag leads the page.
const CONFERENCE_DESCRIPTIONS: &[WireField] = &[WireField::new(1, 0), WireField::new(0, 0)];
/// `FullConferenceDescription {m_parentCategory, m_name, m_sortOrder,
/// m_configuration, m_id}` — the id leads, the category and sort order close.
const FULL_CONFERENCE_DESCRIPTION: &[WireField] = &[
    WireField::new(4, 0),
    WireField::new(3, 0),
    WireField::new(1, 0),
    WireField::new(0, 0),
    WireField::new(2, 0),
];
/// `ConferenceConfiguration {m_maxMembers, m_allowedPrograms, m_allowedRealms,
/// m_flags, m_targetProportion}` — the fill target sits between the arrays,
/// which is what makes this order impossible to guess.
const CONFERENCE_CONFIGURATION: &[WireField] = &[
    WireField::new(0, 8),
    WireField::new(1, 0),
    WireField::new(4, 0),
    WireField::new(2, 0),
    WireField::new(3, 0),
];
/// `ShardName {m_key, m_index}` — the index leads, then a reserved run, then
/// the locator. that run is why the shard cannot be found by walking back from
/// the locator's fields.
const SHARD_NAME: &[WireField] = &[WireField::new(1, 0), WireField::new(0, 29)];
const IDENTITY_2: &[WireField] = &[WireField::new(0, 0), WireField::new(1, 0)];
const IDENTITY_3: &[WireField] = &[
    WireField::new(0, 0),
    WireField::new(1, 0),
    WireField::new(2, 0),
];
const IDENTITY_4: &[WireField] = &[
    WireField::new(0, 0),
    WireField::new(1, 0),
    WireField::new(2, 0),
    WireField::new(3, 0),
];
/// `FriendInvitationAddedNotify {m_invitation, m_isEndOfList}` — the page flag
/// precedes the invitation. Recovered from the retail generated reader.
const FRIEND_INVITATION_ADDED: &[WireField] = &[WireField::new(1, 0), WireField::new(0, 0)];
/// `FriendInvitation` as emitted by the current retail service. The fourteen
/// bits before the profile are generated-reader state, not a reflected field.
const FRIEND_INVITATION: &[WireField] = &[
    WireField::new(3, 0),
    WireField::new(4, 0),
    WireField::new(6, 14),
    WireField::new(1, 0),
    WireField::new(5, 0),
    WireField::new(7, 0),
    WireField::new(0, 0),
    WireField::new(2, 0),
];
const TOON_HANDLE: &[WireField] = &[
    WireField::new(1, 0),
    WireField::new(0, 0),
    WireField::new(2, 0),
    WireField::new(3, 0),
];
const CLUB_INVITE_ACTION: &[WireField] = &[
    WireField::new(2, 0),
    WireField::new(1, 0),
    WireField::new(0, 0),
    WireField::new(3, 11),
];
const MODIFY_CHANNEL_LIST: &[WireField] = &[
    WireField::new(0, 0),
    WireField::new(3, 0),
    WireField::new(2, 4),
    WireField::new(1, 0),
];
const ACHIEVEMENT_CRITERIA_UPDATE: &[WireField] = &[
    WireField::new(1, 0),
    WireField::new(3, 0),
    WireField::new(0, 0),
    WireField::new(4, 0),
    WireField::new(2, 0),
];
const ACHIEVEMENT_PERSISTENT_RECORD: &[WireField] = &[
    WireField::new(2, 0),
    WireField::new(0, 0),
    WireField::new(1, 0),
];
const CHAT_CATEGORY_DESCRIPTIONS: &[WireField] = &[WireField::new(1, 0), WireField::new(0, 0)];
const CHAT_CREATE_AND_INVITE: &[WireField] = &[
    WireField::new(0, 0),
    WireField::new(3, 0),
    WireField::new(1, 0),
    WireField::new(2, 0),
];
const PARTY_MODIFY_NON_LOBBY_ATTRIBUTE_LIST: &[WireField] = &[WireField::new(0, 20)];
const MATCHMAKER_FILTER: &[WireField] = &[
    WireField::new(1, 0),
    WireField::new(0, 17),
    WireField::new(2, 0),
];
const CLUB_NAME: &[WireField] = &[WireField::new(0, 16)];
const CLUB_SUBSCRIPTION_SYNC_INFO: &[WireField] = &[
    WireField::new(2, 11),
    WireField::new(0, 0),
    WireField::new(1, 0),
];
const CLUB_CHANGE_INFO: &[WireField] = &[
    WireField::new(0, 6),
    WireField::new(3, 0),
    WireField::new(1, 0),
    WireField::new(2, 0),
];
const BILLING_INFO: &[WireField] = &[
    WireField::new(2, 0),
    WireField::new(1, 19),
    WireField::new(0, 28),
    WireField::new(3, 0),
];
/// `ConnectionClosing {m_header, m_closingReason, m_badData, m_packets, m_now}`.
/// The client emits its clock and packet history before the failure detail.
/// Recovered from the retail generated reader.
const CONNECTION_CLOSING: &[WireField] = &[
    WireField::new(4, 0),
    WireField::new(0, 0),
    WireField::new(3, 0),
    WireField::new(1, 0),
    WireField::new(2, 0),
];
/// `PacketInfo {m_layer, m_command, m_offset, m_size, m_time}`.
/// Recovered from the retail generated reader.
const PACKET_INFO: &[WireField] = &[
    WireField::new(1, 0),
    WireField::new(0, 0),
    WireField::new(4, 0),
    WireField::new(2, 0),
    WireField::new(3, 0),
];
pub(super) fn register(codec: &mut Codec) -> Result<()> {
    codec.register_struct_wire_layout(
        "Battlenet::Client::Friends::FriendInvitationAddedNotify",
        StructWireLayout::new(
            "generated Friends::FriendInvitationAddedNotify",
            FRIEND_INVITATION_ADDED,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Friends::FriendInvitation",
        StructWireLayout::new("generated Friends::FriendInvitation", FRIEND_INVITATION),
    )?;
    codec.register_candidate_struct_wire_layout(
        "Battlenet::Client::Club::ClubChangeNotification",
        StructWireLayout::new("captured Club::ClubChangeNotification", IDENTITY_1),
    )?;
    codec.register_candidate_struct_wire_layout(
        "Battlenet::Club::ClubChangeInfo",
        StructWireLayout::new("captured Club::ClubChangeInfo", CLUB_CHANGE_INFO),
    )?;
    codec.register_wire_layout(
        "Battlenet::Club::ClubSummaryInfo",
        WireLayout::new_traced(
            "generated Club::ClubSummaryInfo",
            decode_club_summary_info,
            decode_club_summary_info_traced,
            encode_club_summary_info,
        ),
    )?;
    codec.register_wire_layout(
        "Battlenet::Club::ClubUserText",
        WireLayout::new_traced(
            "generated Club::ClubUserText",
            decode_club_user_text,
            decode_club_user_text_traced,
            encode_club_user_text,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Club::ClubSubscribeRequest",
        StructWireLayout::new("generated Club::ClubSubscribeRequest", IDENTITY_1),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Club::SubscriptionSyncInfo",
        StructWireLayout::new(
            "generated Club::SubscriptionSyncInfo",
            CLUB_SUBSCRIPTION_SYNC_INFO,
        ),
    )?;
    codec.register_candidate_struct_wire_layout(
        "Battlenet::Client::Chat::CreateAndInviteRequest",
        StructWireLayout::new(
            "captured Chat::CreateAndInviteRequest",
            CHAT_CREATE_AND_INVITE,
        ),
    )?;
    codec.register_candidate_struct_wire_layout(
        "Battlenet::Client::Party::ModifyNonLobbyAttributeList",
        StructWireLayout::new(
            "captured Party::ModifyNonLobbyAttributeList",
            PARTY_MODIFY_NON_LOBBY_ATTRIBUTE_LIST,
        ),
    )?;
    codec.register_candidate_struct_wire_layout(
        "Battlenet::Client::Profile::ResolveToonHandleToNameRequest",
        StructWireLayout::new(
            "captured Profile::ResolveToonHandleToNameRequest",
            IDENTITY_2,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Profile::ResolveToonHandleToName",
        StructWireLayout::new("empty Profile::ResolveToonHandleToName", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Connection::LogoutRequest",
        StructWireLayout::new("empty Connection::LogoutRequest", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Connection::ConnectionClosing",
        StructWireLayout::new("generated Connection::ConnectionClosing", CONNECTION_CLOSING),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::PacketInfo",
        StructWireLayout::new("generated Connection::PacketInfo", PACKET_INFO),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::S2Map::S2ListMapFavorites",
        StructWireLayout::new("empty S2Map::S2ListMapFavorites", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Club::InviteAction",
        StructWireLayout::new("identity Client::Club::InviteAction", IDENTITY_1),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Achievement::Data",
        StructWireLayout::new("identity Client::Achievement::Data", IDENTITY_2),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Achievement::PersistentRecord",
        StructWireLayout::new(
            "generated Achievement::PersistentRecord",
            ACHIEVEMENT_PERSISTENT_RECORD,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Achievement::QuestUpdateRecord",
        StructWireLayout::new("identity Achievement::QuestUpdateRecord", IDENTITY_1),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Achievement::CriteriaUpdateRecord",
        StructWireLayout::new(
            "generated Achievement::CriteriaUpdateRecord",
            ACHIEVEMENT_CRITERIA_UPDATE,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Club::InviteAction",
        StructWireLayout::new("generated Club::InviteAction", CLUB_INVITE_ACTION),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::ModifyChannelListRequest",
        StructWireLayout::new(
            "generated Chat::ModifyChannelListRequest",
            MODIFY_CHANNEL_LIST,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::ModifyChannelListRequest2",
        StructWireLayout::new("identity Chat::ModifyChannelListRequest2", IDENTITY_4),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::ModifyChannelListResponse2",
        StructWireLayout::new("identity Chat::ModifyChannelListResponse2", IDENTITY_2),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::CategoryDescriptions",
        StructWireLayout::new(
            "generated Chat::CategoryDescriptions",
            CHAT_CATEGORY_DESCRIPTIONS,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::S2Map::S2ListMapFavoritesRequest",
        StructWireLayout::new("identity S2Map::S2ListMapFavoritesRequest", IDENTITY_2),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::S2Map::S2ListMapFavoritesResponse",
        StructWireLayout::new("identity S2Map::S2ListMapFavoritesResponse", IDENTITY_2),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::S2Master::MMQSubscribe",
        StructWireLayout::new("identity S2Master::MMQSubscribe", IDENTITY_2),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::MatchMaker::Filter",
        StructWireLayout::new("generated MatchMaker::Filter", MATCHMAKER_FILTER),
    )?;
    codec.register_wire_layout(
        "Battlenet::Conference::CategoryDescription",
        WireLayout::new_traced(
            "generated Conference::CategoryDescription",
            decode_category_description,
            decode_category_description_traced,
            encode_category_description,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::ConferenceDescriptions",
        StructWireLayout::new(
            "generated Chat::ConferenceDescriptions",
            CONFERENCE_DESCRIPTIONS,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Conference::FullConferenceDescription",
        StructWireLayout::new(
            "generated Conference::FullConferenceDescription",
            FULL_CONFERENCE_DESCRIPTION,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Conference::ConferenceConfiguration",
        StructWireLayout::new(
            "generated Conference::ConferenceConfiguration",
            CONFERENCE_CONFIGURATION,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Conference::ShardName",
        StructWireLayout::new("generated Conference::ShardName", SHARD_NAME),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Conference::PublicPartialName",
        StructWireLayout::new("identity Conference::PublicPartialName", IDENTITY_2),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Conference::ClubName",
        StructWireLayout::new("generated Conference::ClubName", CLUB_NAME),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Toon::Handle",
        StructWireLayout::new("generated Toon::Handle", TOON_HANDLE),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::BillingUpdateNotify",
        StructWireLayout::new("identity Toon::BillingUpdateNotify", IDENTITY_1),
    )?;
    register_session_and_toon_layouts(codec)?;
    register_empty_layouts(codec)?;
    for name in VERIFIED_REFLECTED {
        codec.register_verified_reflected(name)?;
    }
    for name in CANDIDATE_REFLECTED {
        codec.register_candidate_reflected(name)?;
    }
    Ok(())
}

/// the marked types that reflect no members at all. A layout is a field count
/// followed by one entry per field, and filler is only ever emitted before a
/// field, so a type with no fields transmits nothing whatever its order would
/// have been. These need no recovery from generated code.
fn register_empty_layouts(codec: &mut Codec) -> Result<()> {
    codec.register_struct_wire_layout(
        "Battlenet::Client::Cache::GetStreamItems",
        StructWireLayout::new("empty Cache::GetStreamItems", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::ChannelList",
        StructWireLayout::new("empty Chat::ChannelList", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::EnumCategoryDescriptions",
        StructWireLayout::new("empty Chat::EnumCategoryDescriptions", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::EnumConferenceDescriptions",
        StructWireLayout::new("empty Chat::EnumConferenceDescriptions", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::EnumConferenceMemberCounts",
        StructWireLayout::new("empty Chat::EnumConferenceMemberCounts", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::Message",
        StructWireLayout::new("empty Chat::Message", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::Whisper",
        StructWireLayout::new("empty Chat::Whisper", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::WhisperEcho",
        StructWireLayout::new("empty Chat::WhisperEcho", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Connection::EnableEncryption",
        StructWireLayout::new("empty Connection::EnableEncryption", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Friends::ToonsOfFriendPacket",
        StructWireLayout::new("empty Friends::ToonsOfFriendPacket", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Presence::TemporaryPresence",
        StructWireLayout::new("empty Presence::TemporaryPresence", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Profile::AddressQuery",
        StructWireLayout::new("empty Profile::AddressQuery", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Profile::Read",
        StructWireLayout::new("empty Profile::Read", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Profile::ResolveToonNameToHandle",
        StructWireLayout::new("empty Profile::ResolveToonNameToHandle", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::S2Master::CurrentSeason",
        StructWireLayout::new("empty S2Master::CurrentSeason", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::S2Master::MMQGetInfo",
        StructWireLayout::new("empty S2Master::MMQGetInfo", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::S2Master::MMQGetList",
        StructWireLayout::new("empty S2Master::MMQGetList", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Presence::SharedPackets::UpdateBase",
        StructWireLayout::new("empty Presence::SharedPackets::UpdateBase", IDENTITY_0),
    )?;
    Ok(())
}

fn register_session_and_toon_layouts(codec: &mut Codec) -> Result<()> {
    codec.register_struct_wire_layout(
        "Battlenet::Session::BillingInfo",
        StructWireLayout::new("generated Session::BillingInfo", BILLING_INFO),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::ToonCreateInit",
        StructWireLayout::new("empty Toon::ToonCreateInit", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::ToonCreateSetup",
        StructWireLayout::new("empty Toon::ToonCreateSetup", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::ToonCreateFinal",
        StructWireLayout::new("identity Toon::ToonCreateFinal", IDENTITY_1),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Toon::ToonCreationData",
        StructWireLayout::new("identity Toon::ToonCreationData", IDENTITY_1),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::ToonCreateCancel",
        StructWireLayout::new("empty Toon::ToonCreateCancel", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::ToonCreated",
        StructWireLayout::new("identity Toon::ToonCreated", IDENTITY_3),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::Failure",
        StructWireLayout::new("identity Toon::Failure", IDENTITY_1),
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
struct ClubSummaryValues {
    start_bit: usize,
    end_bit: usize,
    fields: Vec<(usize, BsnValue, usize, usize)>,
}

fn decode_club_summary_info(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
) -> Result<BsnValue> {
    let values = read_club_summary_info(codec, root_type, reader)?;
    build_struct(codec, root_type, values.fields)
}

fn decode_club_summary_info_traced(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
) -> Result<(BsnValue, Vec<DecodedField>)> {
    let values = read_club_summary_info(codec, root_type, reader)?;
    let shape = codec.schema().shape(root_type)?;
    let mut traced = vec![DecodedField {
        path: path.to_owned(),
        kind: "struct",
        value: format!("{} fields", values.fields.len()),
        start_bit: values.start_bit,
        end_bit: values.end_bit,
        depth,
    }];
    for (position, value, start_bit, end_bit) in &values.fields {
        let name = shape.member_names[*position].as_deref().unwrap_or("field");
        traced.push(DecodedField {
            path: format!("{path}.{name}"),
            kind: "value",
            value: display_value(value),
            start_bit: *start_bit,
            end_bit: *end_bit,
            depth: depth + 1,
        });
    }
    Ok((build_struct(codec, root_type, values.fields)?, traced))
}

fn read_club_summary_info(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
) -> Result<ClubSummaryValues> {
    let start_bit = reader.position();
    let mut fields = Vec::with_capacity(12);
    read_club_field(codec, root_type, reader, 6, &mut fields)?;
    read_club_field(codec, root_type, reader, 10, &mut fields)?;
    read_club_field(codec, root_type, reader, 0, &mut fields)?;
    read_club_field(codec, root_type, reader, 5, &mut fields)?;

    let name_start = reader.position();
    let name = read_generated_string(reader, 8, 32)?;
    fields.push((2, BsnValue::String(name), name_start, reader.position()));

    reader.read(6)?;
    read_club_field(codec, root_type, reader, 9, &mut fields)?;
    read_club_field(codec, root_type, reader, 7, &mut fields)?;
    read_club_field(codec, root_type, reader, 4, &mut fields)?;
    read_club_field(codec, root_type, reader, 1, &mut fields)?;
    read_club_field(codec, root_type, reader, 11, &mut fields)?;

    let tag_start = reader.position();
    let tag = if reader.read(1)? == 0 {
        BsnValue::none()
    } else {
        BsnValue::some(BsnValue::String(read_generated_string(reader, 5, 24)?))
    };
    fields.push((3, tag, tag_start, reader.position()));

    reader.read(25)?;
    read_club_field(codec, root_type, reader, 8, &mut fields)?;
    Ok(ClubSummaryValues {
        start_bit,
        end_bit: reader.position(),
        fields,
    })
}

fn read_club_field(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
    position: usize,
    fields: &mut Vec<(usize, BsnValue, usize, usize)>,
) -> Result<()> {
    let shape = codec.schema().shape(root_type)?;
    let member_type = *shape
        .member_types
        .get(position)
        .ok_or_else(|| bsn_wire_error(format!("club summary metadata omits field {position}")))?;
    let start_bit = reader.position();
    let value = codec.decode_from(reader, member_type)?;
    fields.push((position, value, start_bit, reader.position()));
    Ok(())
}

fn read_generated_string(
    reader: &mut BitReader<'_>,
    count_width: usize,
    maximum: usize,
) -> Result<String> {
    let length = usize::try_from(reader.read(count_width)?)
        .map_err(|_| bsn_wire_error("generated string length exceeds usize"))?;
    if length > maximum {
        return Err(bsn_wire_error(format!(
            "generated string length {length} exceeds {maximum}"
        )));
    }
    Ok(String::from_utf8_lossy(&reader.read_bytes(length, true)?).into_owned())
}

fn build_struct(
    codec: &Codec,
    root_type: u32,
    values: Vec<(usize, BsnValue, usize, usize)>,
) -> Result<BsnValue> {
    let shape = codec.schema().shape(root_type)?;
    let mut values = values
        .into_iter()
        .map(|(position, value, _, _)| (position, value))
        .collect::<std::collections::BTreeMap<_, _>>();
    let fields = (0..shape.member_types.len())
        .map(|position| {
            Ok(BsnField {
                index: shape.index_values[position],
                name: shape.member_names[position].clone(),
                value: values.remove(&position).ok_or_else(|| {
                    bsn_wire_error(format!("generated layout omits field {position}"))
                })?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(BsnValue::Struct(BsnStruct::new(root_type, fields)))
}

fn display_value(value: &BsnValue) -> String {
    match value {
        BsnValue::Integer(value) => value.to_string(),
        BsnValue::FourCc(value) => String::from_utf8_lossy(&value.to_be_bytes()).into_owned(),
        BsnValue::String(value) => value.clone(),
        BsnValue::Bool(value) => value.to_string(),
        BsnValue::Optional(Some(value)) => display_value(value),
        BsnValue::Optional(None) => "none".to_owned(),
        BsnValue::Array(values) => format!("{} items", values.len()),
        BsnValue::Struct(value) => format!("{} fields", value.fields.len()),
        BsnValue::Bytes(value) => format!("{} bytes", value.len()),
        BsnValue::Choice { index, .. } => format!("variant {index}"),
        BsnValue::BitArray(value) => format!("{} bits", value.bit_count),
        BsnValue::Float32(value) => value.to_string(),
        BsnValue::Float64(value) => value.to_string(),
        BsnValue::Void => "void".to_owned(),
    }
}

fn encode_club_summary_info(
    _codec: &Codec,
    _root_type: u32,
    _writer: &mut BitWriter,
    _value: &BsnValue,
) -> Result<()> {
    Err(bsn_wire_error("club summary information is inbound-only"))
}

fn decode_club_user_text(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
) -> Result<BsnValue> {
    let values = read_club_user_text(codec, root_type, reader)?;
    build_struct(codec, root_type, values.fields)
}

fn decode_club_user_text_traced(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
) -> Result<(BsnValue, Vec<DecodedField>)> {
    let values = read_club_user_text(codec, root_type, reader)?;
    let shape = codec.schema().shape(root_type)?;
    let mut traced = vec![DecodedField {
        path: path.to_owned(),
        kind: "struct",
        value: format!("{} fields", values.fields.len()),
        start_bit: values.start_bit,
        end_bit: values.end_bit,
        depth,
    }];
    for (position, value, start_bit, end_bit) in &values.fields {
        let name = shape.member_names[*position].as_deref().unwrap_or("field");
        traced.push(DecodedField {
            path: format!("{path}.{name}"),
            kind: "value",
            value: display_value(value),
            start_bit: *start_bit,
            end_bit: *end_bit,
            depth: depth + 1,
        });
    }
    Ok((build_struct(codec, root_type, values.fields)?, traced))
}

fn read_club_user_text(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
) -> Result<ClubSummaryValues> {
    let start_bit = reader.position();
    let mut fields = Vec::with_capacity(4);
    read_club_field(codec, root_type, reader, 0, &mut fields)?;
    read_club_field(codec, root_type, reader, 3, &mut fields)?;

    let text_start = reader.position();
    let text = read_generated_string(reader, 13, 4096)?;
    fields.push((2, BsnValue::String(text), text_start, reader.position()));

    read_club_field(codec, root_type, reader, 1, &mut fields)?;
    Ok(ClubSummaryValues {
        start_bit,
        end_bit: reader.position(),
        fields,
    })
}

fn encode_club_user_text(
    _codec: &Codec,
    _root_type: u32,
    _writer: &mut BitWriter,
    _value: &BsnValue,
) -> Result<()> {
    Err(bsn_wire_error("club user text is inbound-only"))
}

#[derive(Clone, Debug)]
struct CategoryDescriptionValues {
    start_bit: usize,
    end_bit: usize,
    name: Spanned<i128>,
    generated_start_bit: usize,
    generated_end_bit: usize,
    id: Spanned<i128>,
    sort_order: Spanned<i128>,
}

#[derive(Clone, Debug)]
struct Spanned<T> {
    value: T,
    start_bit: usize,
    end_bit: usize,
}

fn decode_category_description(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
) -> Result<BsnValue> {
    let values = read_category_description(reader)?;
    build_category_description(codec, root_type, &values)
}

fn decode_category_description_traced(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
) -> Result<(BsnValue, Vec<DecodedField>)> {
    let values = read_category_description(reader)?;
    let fields = vec![
        DecodedField {
            path: path.to_owned(),
            kind: "struct",
            value: "3 fields".to_owned(),
            start_bit: values.start_bit,
            end_bit: values.end_bit,
            depth,
        },
        traced_integer(format!("{path}.m_name"), "uint32", &values.name, depth + 1),
        DecodedField {
            path: format!("{path}.generated"),
            kind: "generated fields",
            value: "ignored".to_owned(),
            start_bit: values.generated_start_bit,
            end_bit: values.generated_end_bit,
            depth: depth + 1,
        },
        traced_integer(format!("{path}.m_id"), "uint8", &values.id, depth + 1),
        traced_integer(
            format!("{path}.m_sortOrder"),
            "uint16",
            &values.sort_order,
            depth + 1,
        ),
    ];
    Ok((
        build_category_description(codec, root_type, &values)?,
        fields,
    ))
}

fn read_category_description(reader: &mut BitReader<'_>) -> Result<CategoryDescriptionValues> {
    let start_bit = reader.position();
    let name = read_spanned(reader, 32, i128::from)?;
    let generated_start_bit = reader.position();
    read_category_generated_state(reader)?;
    read_category_generated_variant(reader)?;
    let generated_end_bit = reader.position();
    let id = read_spanned(reader, 8, i128::from)?;
    let sort_order = read_spanned(reader, 16, i128::from)?;
    Ok(CategoryDescriptionValues {
        start_bit,
        end_bit: reader.position(),
        name,
        generated_start_bit,
        generated_end_bit,
        id,
        sort_order,
    })
}

fn read_category_generated_state(reader: &mut BitReader<'_>) -> Result<()> {
    reader.read(8)?;
    reader.read(16)?;
    read_bounded_u32_array(reader)?;
    reader.read(32)?;
    read_bounded_u32_array(reader)?;
    reader.read(32)?;
    Ok(())
}

fn read_bounded_u32_array(reader: &mut BitReader<'_>) -> Result<()> {
    let count = usize::try_from(reader.read(3)?).expect("3-bit value fits in usize");
    if count >= 5 {
        return Err(bsn_wire_error(format!(
            "generated category array count {count} exceeds four"
        )));
    }
    for _ in 0..count {
        reader.read(32)?;
    }
    Ok(())
}

fn read_category_generated_variant(reader: &mut BitReader<'_>) -> Result<()> {
    reader.read(16)?;
    reader.read(29)?;
    match reader.read(2)? {
        0 => {
            let length = usize::try_from(reader.read(7)?).expect("7-bit value fits in usize");
            if length >= 125 {
                return Err(bsn_wire_error(format!(
                    "generated category string length {length} exceeds 124"
                )));
            }
            reader.read_bytes(length, true)?;
        }
        2 => {
            reader.read(32)?;
            reader.read(16)?;
        }
        3 => {
            reader.read(16)?;
            reader.read(32)?;
        }
        tag => {
            return Err(bsn_wire_error(format!(
                "unsupported generated category variant {tag}"
            )));
        }
    }
    Ok(())
}

fn build_category_description(
    codec: &Codec,
    root_type: u32,
    values: &CategoryDescriptionValues,
) -> Result<BsnValue> {
    let shape = codec.schema().shape(root_type)?;
    let logical_values = [
        ("m_id", BsnValue::Integer(values.id.value)),
        ("m_name", BsnValue::Integer(values.name.value)),
        ("m_sortOrder", BsnValue::Integer(values.sort_order.value)),
    ];
    let mut fields = Vec::with_capacity(logical_values.len());
    for (name, value) in logical_values {
        let position = shape
            .member_names
            .iter()
            .position(|candidate| candidate.as_deref() == Some(name))
            .ok_or_else(|| bsn_wire_error(format!("category metadata omits field {name}")))?;
        fields.push(BsnField {
            index: shape.index_values[position],
            name: shape.member_names[position].clone(),
            value,
        });
    }
    Ok(BsnValue::Struct(BsnStruct::new(root_type, fields)))
}

fn encode_category_description(
    _codec: &Codec,
    _root_type: u32,
    _writer: &mut BitWriter,
    _value: &BsnValue,
) -> Result<()> {
    Err(bsn_wire_error(
        "generated category descriptions are inbound-only",
    ))
}

fn read_spanned<T>(
    reader: &mut BitReader<'_>,
    width: usize,
    convert: impl FnOnce(u64) -> T,
) -> Result<Spanned<T>> {
    let start_bit = reader.position();
    let value = convert(reader.read(width)?);
    Ok(Spanned {
        value,
        start_bit,
        end_bit: reader.position(),
    })
}

fn traced_integer(
    path: String,
    kind: &'static str,
    value: &Spanned<i128>,
    depth: usize,
) -> DecodedField {
    DecodedField {
        path,
        kind,
        value: value.value.to_string(),
        start_bit: value.start_bit,
        end_bit: value.end_bit,
        depth,
    }
}

fn bsn_wire_error(message: impl Into<String>) -> Error {
    Error::BsnWire(message.into())
}
