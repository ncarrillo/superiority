use crate::native::inspect::Field;

fn normalized_name(path: &str) -> String {
    let leaf = path.rsplit('.').next().unwrap_or(path);
    if leaf
        .strip_suffix(']')
        .and_then(|leaf| leaf.rsplit_once('['))
        .is_some_and(|(_, index)| index.chars().all(|character| character.is_ascii_digit()))
    {
        return "array_item".to_owned();
    }
    let leaf = leaf.strip_prefix("m_").unwrap_or(leaf);
    let mut normalized = String::with_capacity(leaf.len());
    for (index, character) in leaf.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 && !normalized.ends_with('_') {
                normalized.push('_');
            }
            normalized.push(character.to_ascii_lowercase());
        } else {
            normalized.push(character.to_ascii_lowercase());
        }
    }
    normalized
}

fn known_meaning(path: &str, name: &str) -> Option<&'static str> {
    if path == "route" {
        return Some(
            "Routing header that selects the service and command used to decode this record.",
        );
    }
    if path == "payload" {
        return Some(
            "Command payload decoded according to the selected service and command schema.",
        );
    }
    if path == "padding" {
        return Some("Zero-valued alignment bits that complete the final wire byte.");
    }
    match name {
        "command_id" => Some("Command number within the routed service."),
        "service_present" => {
            Some("Indicates whether an explicit service slot follows in the routing header.")
        }
        "service_slot" => {
            Some("Negotiated service slot used to select the protocol service for this record.")
        }
        "success" => Some(
            "Indicates whether the operation completed successfully. Some chat records encode false on the wire as success.",
        ),
        "member_handle" => {
            Some("Channel-local handle that identifies a member within the current chat channel.")
        }
        "channel_index" => Some("Client session slot assigned to an open chat channel."),
        "conference_id" => Some("Server identifier for the conference backing this chat channel."),
        "owner_id" => Some("Identifier of the account or entity that owns this channel."),
        "channel_type" => Some("Protocol classification of the chat channel."),
        "channel_name" => Some(
            "Structured identity of the chat channel, including its namespace and selected naming form.",
        ),
        "channel_config" => {
            Some("Optional server-provided configuration associated with this chat channel.")
        }
        "presence_id" | "inviter_presence" => Some(
            "Presence identifier used to correlate this user with presence and profile records.",
        ),
        "local_presence_id" => Some("Presence identifier assigned within the current session."),
        "master_presence_id" => Some("Canonical presence identifier used across related sessions."),
        "display_name" => Some("User-facing Battle.net display name."),
        "toon_name" => {
            Some("Structured StarCraft II character identity, including region, realm, and name.")
        }
        "program_id" => {
            Some("Four-character product identifier for the game or Battle.net program.")
        }
        "region" => Some("Battle.net region component of this structured identifier."),
        "realm" => Some("Realm component of this StarCraft II character identity."),
        "namespace" => Some("Namespace component that separates channel identifier domains."),
        "locale" => Some("Four-character locale code associated with this value."),
        "index" if path.contains("channel_name") => {
            Some("Channel identifier within the selected namespace.")
        }
        "owner" if path.contains("channel_name") => {
            Some("Owner component of this structured channel identity.")
        }
        "literal" if path.contains("channel_name") => {
            Some("Literal channel name used by this channel identity variant.")
        }
        "club_id" => Some("Stable identifier for a Battle.net clan or community."),
        "account_id" => Some("Stable identifier for a Battle.net account."),
        "request_id" => Some("Identifier used to correlate this response with its request."),
        "token" => Some("Opaque value used to correlate or authorize the associated operation."),
        "reason" => {
            Some("Protocol reason code explaining why the operation failed or changed state.")
        }
        "operation" => {
            Some("Change operation describing whether the item was added, removed, or modified.")
        }
        "complete" => {
            Some("Indicates whether this page completes the current streamed result set.")
        }
        "item_count" => Some("Number of items represented by this decoded response."),
        "publication_time" => {
            Some("Server publication timestamp associated with this cached item.")
        }
        "content_handle" => {
            Some("Opaque content-addressed handle for retrieving the associated cached data.")
        }
        "frame_type" => Some("Discriminant identifying the kind of connection message frame."),
        "headers" => Some("Metadata headers carried by this connection message frame."),
        "message" | "body" => Some("Text content carried by this chat message."),
        "changes" => Some("Ordered set of membership changes reported by this notification."),
        "member_status" => {
            Some("Presence and identity information reported for this channel member.")
        }
        "sender" => Some("Identity of the user that originated this message or event."),
        "target" => Some("Identity or channel targeted by this operation."),
        "private" => Some("Indicates whether membership or discovery of this group is restricted."),
        "category" => Some("Server-defined category used to classify this group or item."),
        "name" if path.contains("toon") => Some("StarCraft II character name."),
        "name" if path.contains("club") => Some("User-facing name of this clan or community."),
        "name" => Some("User-facing name associated with this schema object."),
        "note" => Some("User-authored Battle.net friend note."),
        "full_name" => Some("Real-name value shared through Battle.net social permissions."),
        "role" => Some("Server-defined role assigned to this account or group member."),
        "kind" if path.contains("channel_name") => {
            Some("Choice selector that determines how the channel name is encoded.")
        }
        "kind" => Some(
            "Schema discriminator identifying the variant or classification represented by this value.",
        ),
        "present" => Some(
            "Presence bit that controls whether the associated optional value follows on the wire.",
        ),
        "array_item" => Some("One element of the containing schema array."),
        _ => None,
    }
}

fn encoded_meaning(field: &Field, container: bool) -> String {
    if let Some(meaning) = known_meaning(&field.path, &normalized_name(&field.path)) {
        return meaning.to_owned();
    }
    let kind = field.kind.to_ascii_lowercase();
    if container || kind.contains("struct") || kind == "object" {
        "Structured schema value containing the nested properties shown below.".to_owned()
    } else if kind.contains("array") {
        "Ordered collection of values described by the nested schema entries.".to_owned()
    } else if kind.contains("optional") {
        "Optional schema value whose presence is controlled by an encoded marker.".to_owned()
    } else if kind.contains("choice") || kind.contains("variant") {
        "Tagged schema value whose selector determines which alternative is encoded.".to_owned()
    } else if kind.contains("bool") {
        "Boolean value encoded by this schema property.".to_owned()
    } else if kind.contains("uint") || kind.contains("integer") || kind == "number" {
        "Unsigned numeric value encoded with the width declared by the schema.".to_owned()
    } else if kind.contains("string") || kind.contains("fourcc") {
        "Textual value decoded using the representation declared by the schema.".to_owned()
    } else if kind.contains("blob") || kind.contains("bytes") {
        "Opaque byte sequence carried by this schema property.".to_owned()
    } else {
        "Decoded value exposed by the native protocol schema.".to_owned()
    }
}

pub(super) fn tooltip_detail(field: &Field, container: bool) -> String {
    let meaning = encoded_meaning(field, container);
    if field.exact_range {
        format!(
            "{meaning} Encoded as {} in bits [{}, {}).",
            field.kind, field.start_bit, field.end_bit
        )
    } else {
        format!(
            "{meaning} Decoded as {}; its encoded value is contained within bits [{}, {}), but an exact field boundary is not available.",
            field.kind, field.start_bit, field.end_bit
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::inspect::FieldRole;

    fn field(path: &str, kind: &'static str, exact_range: bool) -> Field {
        Field {
            path: path.to_owned(),
            kind,
            value: "42".to_owned(),
            start_bit: 7,
            end_bit: 11,
            depth: 1,
            role: FieldRole::Payload,
            exact_range,
        }
    }

    #[test]
    fn normalizes_schema_member_names() {
        assert_eq!(normalized_name("payload.m_memberHandle"), "member_handle");
        assert_eq!(normalized_name("payload.items[0]"), "array_item");
    }

    #[test]
    fn describes_known_fields_and_exact_ranges() {
        let detail = tooltip_detail(&field("payload.m_memberHandle", "uint32", true), false);
        assert!(detail.contains("Channel-local handle"));
        assert!(detail.contains("bits [7, 11)"));
    }

    #[test]
    fn describes_unknown_fields_without_inventing_semantics() {
        let detail = tooltip_detail(&field("payload.m_unknown", "uint4", false), false);
        assert!(detail.contains("Unsigned numeric value"));
        assert!(detail.contains("contained within bits [7, 11)"));
    }
}
