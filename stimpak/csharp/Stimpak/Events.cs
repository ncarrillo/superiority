using System.Text.Json;
using System.Text.Json.Serialization;

namespace Stimpak;

/// <summary>rust serialises these as snake_case.</summary>
internal sealed class SnakeCaseEnumConverter<T>() : JsonStringEnumConverter<T>(JsonNamingPolicy.SnakeCaseLower)
    where T : struct, Enum;

[JsonConverter(typeof(SnakeCaseEnumConverter<Presence>))]
public enum Presence
{
    Unknown = 0,
    Available,
    Away,
    Busy,
    InGame,
    Offline,
}

/// <summary>
/// one description of a person, as of one event. often incomplete — a join
/// names somebody before their profile exists. bind to
/// <see cref="PeopleRegistry"/> rather than holding one of these.
/// </summary>
public sealed record User(
    [property: JsonPropertyName("handle")] uint Handle,
    [property: JsonPropertyName("presence_id")] uint? PresenceId,
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("clan_tag")] string? ClanTag,
    [property: JsonPropertyName("presence")] Presence Presence);

[JsonPolymorphic(TypeDiscriminatorPropertyName = "kind")]
[JsonDerivedType(typeof(PublicChannel), "public")]
[JsonDerivedType(typeof(PrivateChannel), "private")]
[JsonDerivedType(typeof(GroupChannel), "group")]
[JsonDerivedType(typeof(PartyChannel), "party")]
/// <summary><see cref="Name"/> is resolved from the catalogue, so it reads the
/// same as it would in the app rather than "Public 1028".</summary>
public abstract record ChatChannel(
    [property: JsonPropertyName("name")] string Name);

public sealed record PublicChannel(
    [property: JsonPropertyName("id")] ushort Id,
    string Name) : ChatChannel(Name);

public sealed record PrivateChannel(string Name) : ChatChannel(Name);

public sealed record GroupChannel(
    [property: JsonPropertyName("club_id")] uint ClubId,
    string Name) : ChatChannel(Name);

public sealed record PartyChannel(string Name) : ChatChannel(Name);

public sealed record Friend(
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("presence")] Presence Presence);

/// <summary>match on the concrete type; the json that carried it is an implementation detail.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type", UnknownDerivedTypeHandling = JsonUnknownDerivedTypeHandling.FallBackToNearestAncestor)]
[JsonDerivedType(typeof(StageChanged), "stage")]
[JsonDerivedType(typeof(AuthenticationRequired), "authentication_required")]
[JsonDerivedType(typeof(AccountConnected), "account")]
[JsonDerivedType(typeof(Joined), "joined")]
[JsonDerivedType(typeof(JoinRejected), "join_rejected")]
[JsonDerivedType(typeof(Left), "left")]
[JsonDerivedType(typeof(PublicChannelsReceived), "public_channels")]
[JsonDerivedType(typeof(RosterReceived), "roster")]
[JsonDerivedType(typeof(MemberJoined), "member_joined")]
[JsonDerivedType(typeof(MemberLeft), "member_left")]
[JsonDerivedType(typeof(MessageReceived), "message")]
[JsonDerivedType(typeof(WhisperReceived), "whisper")]
[JsonDerivedType(typeof(WhisperFailed), "whisper_failed")]
[JsonDerivedType(typeof(FriendsReceived), "friends")]
[JsonDerivedType(typeof(GroupInvitation), "group_invitation")]
[JsonDerivedType(typeof(PartyInvitation), "party_invitation")]
[JsonDerivedType(typeof(GroupSummaryReceived), "group_summary")]
[JsonDerivedType(typeof(GroupSearchReceived), "group_search")]
[JsonDerivedType(typeof(CommandFailed), "command_error")]
[JsonDerivedType(typeof(SessionFailed), "error")]
[JsonDerivedType(typeof(UnrecognisedEvent), "other")]
[JsonDerivedType(typeof(SessionEnded), "session_ended")]
public abstract record SC2Event;

[JsonConverter(typeof(SnakeCaseEnumConverter<Stage>))]
public enum Stage
{
    Disconnected = 0,
    WebAuthentication,
    GameUtilities,
    NativeAuthentication,
    ChatBootstrap,
    Connected,
}

public sealed record StageChanged(
    [property: JsonPropertyName("stage")] Stage Stage) : SC2Event;

/// <summary>
/// Complete this request manually with <see cref="StimpakClient.SubmitAuth"/>,
/// or pass it to an optional in-process authentication provider.
/// a bot with a cached credential never sees this.
/// </summary>
public sealed record AuthenticationRequired(
    [property: JsonPropertyName("auth_id")] ulong AuthId,
    [property: JsonPropertyName("url")] string Url,
    [property: JsonPropertyName("fresh_account")] bool FreshAccount) : SC2Event;

public sealed record AccountSummary(
    [property: JsonPropertyName("account_id")] ulong? AccountId,
    [property: JsonPropertyName("battle_tag")] string? BattleTag,
    [property: JsonPropertyName("region")] uint? Region,
    [property: JsonPropertyName("games")] IReadOnlyList<string>? Games);

/// <summary>the Battle.net account established for this connection.</summary>
public sealed record AccountConnected(
    [property: JsonPropertyName("account")] AccountSummary Account) : SC2Event;

public sealed record Joined(
    [property: JsonPropertyName("channel_index")] byte ChannelIndex,
    [property: JsonPropertyName("channel")] ChatChannel Channel,
    [property: JsonPropertyName("local_handle")] uint LocalHandle) : SC2Event;

public sealed record JoinRejected(
    [property: JsonPropertyName("channel")] ChatChannel? Channel,
    [property: JsonPropertyName("reason")] ushort? Reason) : SC2Event;

public sealed record Left(
    [property: JsonPropertyName("channel_index")] byte ChannelIndex,
    [property: JsonPropertyName("reason")] ushort? Reason) : SC2Event;

/// <summary>the channels this account may join. arrives once per session.</summary>
public sealed record PublicChannelsReceived(
    [property: JsonPropertyName("channels")] IReadOnlyList<ChatChannel> Channels) : SC2Event;

public sealed record RosterReceived(
    [property: JsonPropertyName("channel_index")] byte ChannelIndex,
    [property: JsonPropertyName("complete")] bool Complete,
    [property: JsonPropertyName("users")] IReadOnlyList<User> Users) : SC2Event;

public sealed record MemberJoined(
    [property: JsonPropertyName("channel_index")] byte ChannelIndex,
    [property: JsonPropertyName("user")] User User) : SC2Event;

public sealed record MemberLeft(
    [property: JsonPropertyName("channel_index")] byte ChannelIndex,
    [property: JsonPropertyName("user")] User User) : SC2Event;

public sealed record MessageReceived(
    [property: JsonPropertyName("channel_index")] byte ChannelIndex,
    [property: JsonPropertyName("sender")] User Sender,
    [property: JsonPropertyName("body")] string Body) : SC2Event;

public sealed record WhisperReceived(
    [property: JsonPropertyName("peer")] string Peer,
    [property: JsonPropertyName("body")] string Body,
    [property: JsonPropertyName("outgoing")] bool Outgoing) : SC2Event;

public sealed record WhisperFailed(
    [property: JsonPropertyName("peer")] string Peer,
    [property: JsonPropertyName("reason")] string Reason) : SC2Event;

public sealed record FriendsReceived(
    [property: JsonPropertyName("friends")] IReadOnlyList<Friend> Friends) : SC2Event;

public sealed record GroupInvitation(
    [property: JsonPropertyName("club_id")] uint ClubId) : SC2Event;

public sealed record PartyInvitation(
    [property: JsonPropertyName("inviter")] string? Inviter,
    [property: JsonPropertyName("channel_index")] byte ChannelIndex) : SC2Event;

public sealed record GroupSummaryReceived(
    [property: JsonPropertyName("club_id")] uint ClubId,
    [property: JsonPropertyName("name")] string? Name,
    [property: JsonPropertyName("kind")] byte Kind,
    [property: JsonPropertyName("category")] byte Category,
    [property: JsonPropertyName("private")] bool Private,
    [property: JsonPropertyName("member")] bool Member,
    [property: JsonPropertyName("member_count")] uint? MemberCount,
    [property: JsonPropertyName("online")] uint? Online) : SC2Event;

public sealed record GroupSearchReceived(
    [property: JsonPropertyName("club_ids")] IReadOnlyList<uint> ClubIds) : SC2Event;

public sealed record CommandFailed(
    [property: JsonPropertyName("message")] string Message) : SC2Event;

public sealed record SessionFailed(
    [property: JsonPropertyName("message")] string Message) : SC2Event;

/// <summary>decoded, but with no case in this binding yet. the variant is named
/// without its payload, which keeps block lists out of it.</summary>
public sealed record UnrecognisedEvent(
    [property: JsonPropertyName("kind")] string Kind) : SC2Event;

/// <summary>
/// an event type introduced by a newer native library. Its raw JSON is kept so
/// a host can log or forward it without losing the event entirely.
/// </summary>
public sealed record UnknownEvent(string EventType, string RawJson) : SC2Event;

/// <summary>
/// native JSON that did not satisfy the managed event schema. This is surfaced
/// instead of being silently discarded.
/// </summary>
public sealed record EventProtocolError(string Detail, string RawJson) : SC2Event;

/// <summary>
/// the session has finished and this client will report nothing further. the
/// caller owns the lifecycle: dispose and open a new one to reconnect.
/// </summary>
public sealed record SessionEnded : SC2Event;
