using System.Text.Json;

namespace Stimpak;

/// <summary>
/// Keeps forward compatibility explicit. System.Text.Json's polymorphic
/// fallback governs undeclared runtime types during serialization; it does not
/// preserve an unknown discriminator during deserialization.
/// </summary>
internal static class EventJson
{
    private static readonly JsonSerializerOptions Options = new();

    internal static string SerializeTargets(IReadOnlyList<ChannelTarget> targets) =>
        JsonSerializer.Serialize(targets, Options);

    internal static SC2Event Deserialize(string json)
    {
        try
        {
            using var document = JsonDocument.Parse(json);
            if (!document.RootElement.TryGetProperty("type", out var property) ||
                property.ValueKind != JsonValueKind.String)
            {
                return new EventProtocolError("event has no string type discriminator", json);
            }

            var type = property.GetString()!;
            return type switch
            {
                "stage" => Read<StageChanged>(json),
                "authentication_required" => Read<AuthenticationRequired>(json),
                "account" => Read<AccountConnected>(json),
                "joined" => Read<Joined>(json),
                "join_rejected" => Read<JoinRejected>(json),
                "left" => Read<Left>(json),
                "public_channels" => Read<PublicChannelsReceived>(json),
                "roster" => Read<RosterReceived>(json),
                "member_joined" => Read<MemberJoined>(json),
                "member_left" => Read<MemberLeft>(json),
                "message" => Read<MessageReceived>(json),
                "whisper" => Read<WhisperReceived>(json),
                "whisper_failed" => Read<WhisperFailed>(json),
                "friends" => Read<FriendsReceived>(json),
                "group_invitation" => Read<GroupInvitation>(json),
                "party_invitation" => Read<PartyInvitation>(json),
                "group_summary" => Read<GroupSummaryReceived>(json),
                "group_search" => Read<GroupSearchReceived>(json),
                "command_error" => Read<CommandFailed>(json),
                "error" => Read<SessionFailed>(json),
                "other" => Read<UnrecognisedEvent>(json),
                "session_ended" => Read<SessionEnded>(json),
                _ => new UnknownEvent(type, json),
            };
        }
        catch (Exception error) when (error is JsonException or NotSupportedException)
        {
            return new EventProtocolError(error.Message, json);
        }
    }

    private static SC2Event Read<T>(string json) where T : SC2Event
    {
        var value = JsonSerializer.Deserialize<T>(json, Options);
        return value is null
            ? new EventProtocolError($"{typeof(T).Name} deserialized to null", json)
            : value;
    }
}
