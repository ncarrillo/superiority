use sc2_core::bsn::value::{BsnBitArray, BsnField, BsnStruct, BsnValue};
use sc2_core::bsn::{FourCc, FromBsn};
use sc2_core::metadata::{IntegerRange, Schema, TypeKind};
use sc2_core::native::Protocol;
use sc2_core::native::schema;

fn min_count(range: Option<IntegerRange>) -> usize {
    usize::try_from(range.map_or(0, |r| r.minimum).max(0)).unwrap_or(0)
}

fn synth(meta: &Schema, id: u32) -> BsnValue {
    let shape = meta.shape(id).unwrap();
    match shape.kind {
        TypeKind::Alias => synth(meta, shape.element_type.unwrap()),
        TypeKind::Integer => BsnValue::Integer(shape.value_range.map_or(0, |r| r.minimum)),
        TypeKind::Enum => BsnValue::Integer(shape.index_values.first().copied().unwrap_or(0)),
        TypeKind::Bool => BsnValue::Bool(false),
        TypeKind::FourCc => BsnValue::FourCc(0),
        TypeKind::Float32 => BsnValue::Float32(0.0),
        TypeKind::Float64 => BsnValue::Float64(0.0),
        TypeKind::Void => BsnValue::Void,
        TypeKind::String => BsnValue::String("a".repeat(min_count(shape.value_range))),
        TypeKind::ByteString | TypeKind::Blob => {
            BsnValue::Bytes(vec![0u8; min_count(shape.value_range)])
        }
        TypeKind::BitArray => {
            let bits = min_count(shape.value_range);
            BsnValue::BitArray(BsnBitArray {
                data: vec![0u8; bits.div_ceil(8)],
                bit_count: bits,
            })
        }
        TypeKind::Optional => BsnValue::none(),
        TypeKind::Array => {
            let element = shape.element_type.unwrap();
            BsnValue::Array(
                (0..min_count(shape.value_range))
                    .map(|_| synth(meta, element))
                    .collect(),
            )
        }
        TypeKind::Struct => {
            let fields = shape
                .index_values
                .iter()
                .zip(&shape.member_types)
                .zip(&shape.member_names)
                .map(|((index, member), name)| BsnField {
                    index: *index,
                    name: name.clone(),
                    value: synth(meta, *member),
                })
                .collect();
            BsnValue::Struct(BsnStruct::new(id, fields))
        }
        TypeKind::Choice => {
            BsnValue::choice(shape.index_values[0], synth(meta, shape.member_types[0]))
        }
    }
}

fn dynamic_value(metadata: &Schema, name: &str) -> BsnValue {
    let type_id = metadata.unique_type_id(name).unwrap();
    synth(metadata, type_id)
}

#[test]
fn toon_handle_projects_but_is_not_claimed_as_a_reflected_wire_layout() {
    let protocol = Protocol::current().unwrap();
    let codec = protocol.codec();
    let type_id = codec
        .schema()
        .unique_type_id("Battlenet::Toon::Handle")
        .unwrap();

    let original = BsnValue::Struct(BsnStruct::new(
        type_id,
        vec![
            BsnField::named(0, "m_region", BsnValue::Integer(1)),
            BsnField::named(1, "m_programId", BsnValue::FourCc(0)),
            BsnField::named(2, "m_realm", BsnValue::Integer(1)),
            BsnField::named(3, "m_id", BsnValue::Integer(12_345)),
        ],
    ));
    let error = codec.encode(type_id, &original).unwrap_err().to_string();
    assert!(error.contains("Obfuscated"), "{error}");

    let handle = schema::toon::ToonHandle::from_bsn(&original).unwrap();
    assert_eq!(handle.region, 1);
    assert_eq!(handle.program_id, FourCc(0));
    assert_eq!(handle.realm, 1);
    assert_eq!(handle.id, 12_345);
}

#[test]
fn generated_messages_project_dynamic_values() {
    let protocol = Protocol::current().unwrap();
    let codec = protocol.codec();

    macro_rules! check {
        ($bsn:literal => $ty:ty) => {{
            let value = dynamic_value(codec.schema(), $bsn);
            <$ty as FromBsn>::from_bsn(&value)
                .unwrap_or_else(|error| panic!("{} failed to project: {error:?}", $bsn));
        }};
    }

    check!("Battlenet::Client::Chat::MessageRecv" => schema::chat::ClientChatMessageRecv);
    check!("Battlenet::Client::Chat::WhisperRecv" => schema::chat::ClientChatWhisperRecv);
    check!("Battlenet::Client::Chat::JoinNotify2" => schema::chat::ClientChatJoinNotify2);
    check!("Battlenet::Client::Chat::MembershipChangeNotify" => schema::chat::ClientChatMembershipChangeNotify);
    check!("Battlenet::Client::Chat::InviteNotify" => schema::chat::ClientChatInviteNotify);
    check!("Battlenet::Client::Authentication::LogonResponse3" => schema::authentication::ClientAuthenticationLogonResponse3);
    check!("Battlenet::Client::Connection::ServerVersion" => schema::connection::ClientConnectionServerVersion);
    check!("Battlenet::Client::Toon::ToonList" => schema::toon::ClientToonToonList);
    check!("Battlenet::Client::Toon::Welcome" => schema::toon::ClientToonWelcome);
    check!("Battlenet::Client::Club::GetToonClubsResponse" => schema::club::ClientClubGetToonClubsResponse);
    check!("Battlenet::Client::Club::GetClubInfoResponse" => schema::club::ClientClubGetClubInfoResponse);
    check!("Battlenet::Client::Friends::FriendsListNotify5" => schema::friends::ClientFriendsFriendsListNotify5);
    check!("Battlenet::Client::Friends::ToonsOfFriendsNotify" => schema::friends::ClientFriendsToonsOfFriendsNotify);
    check!("Battlenet::Client::Presence::StatisticsUpdate" => schema::presence::ClientPresenceStatisticsUpdate);
    check!("Battlenet::Client::Profile::ReadResponse" => schema::profile::ClientProfileReadResponse);
    check!("Battlenet::Profile::ProfileDataResponse" => schema::profile::ProfileProfileDataResponse);
}
