using System.Text.Json.Serialization;

namespace Stimpak;

/// <summary>construction settings that do not change between connections.</summary>
/// <param name="ApplicationId">
/// stable credential namespace, preferably reverse-DNS, such as
/// <c>com.example.ExampleBot</c>. It is an identifier, not a secret.
/// </param>
public sealed record StimpakClientOptions(string ApplicationId)
{
    /// <summary>
    /// advanced override for the derived per-user credential file. Most
    /// applications should leave this null.
    /// </summary>
    public string? CredentialPath { get; init; }
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
