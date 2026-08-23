using System.Text.Json.Serialization;

namespace Stimpak;

/// <summary>construction settings that do not change between connections.</summary>
public sealed record StimpakClientOptions(string CredentialPath)
{
    /// <summary>
    /// optional path to <c>stimpak-auth-window</c>. When omitted, Stimpak also
    /// checks <c>STIMPAK_AUTH_WINDOW</c> and the application's directory.
    /// </summary>
    public string? AuthWindowPath { get; init; }

    /// <summary>
    /// maximum number of unread managed events. Superseded roster snapshots
    /// are coalesced before this limit is applied.
    /// </summary>
    public int EventCapacity { get; init; } = 2048;
}

/// <summary>settings for one connection attempt.</summary>
public sealed record StimpakConnectOptions
{
    /// <summary>bypass the cached credential and open authentication.</summary>
    public bool ForceInteractive { get; init; }

    /// <summary>
    /// refuse the connection if the cached credential belongs to another
    /// Battle.net account. Use the stable id from <see cref="AccountConnected"/>.
    /// </summary>
    public ulong? ExpectedAccountId { get; init; }

    /// <summary>
    /// channels joined during startup. Empty means General. Party channels are
    /// joined only by accepting an invitation and are therefore not targets.
    /// </summary>
    public IReadOnlyList<ChannelTarget> Channels { get; init; } = [];
}

/// <summary>a channel the next connection should restore.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "kind")]
[JsonDerivedType(typeof(PublicChannelTarget), "public")]
[JsonDerivedType(typeof(PrivateChannelTarget), "private")]
[JsonDerivedType(typeof(GroupChannelTarget), "group")]
public abstract record ChannelTarget
{
    public static ChannelTarget Public(ushort id) => new PublicChannelTarget(id);

    public static ChannelTarget Private(string name) => new PrivateChannelTarget(name);

    public static ChannelTarget Group(uint clubId) => new GroupChannelTarget(clubId);
}

public sealed record PublicChannelTarget(
    [property: JsonPropertyName("id")] ushort Id) : ChannelTarget;

public sealed record PrivateChannelTarget(
    [property: JsonPropertyName("name")] string Name) : ChannelTarget;

public sealed record GroupChannelTarget(
    [property: JsonPropertyName("club_id")] uint ClubId) : ChannelTarget;
