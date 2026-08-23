using Stimpak;
using Stimpak.Auth;
using System.Reflection;

var failures = new List<string>();
var passed = 0;

Run("channel handles are scoped", ChannelHandlesAreScoped);
Run("disconnect resets identities", DisconnectResetsIdentities);
Run("rejoined channel resets identities", RejoinedChannelResetsIdentities);
Run("unknown event types survive", UnknownEventTypesSurvive);
Run("malformed events are surfaced", MalformedEventsAreSurfaced);
Run("account and group events decode", AccountAndGroupEventsDecode);
Run("connection targets serialize", ConnectionTargetsSerialize);
Run("native and managed versions agree", NativeAndManagedVersionsAgree);
Run("application ids reject paths", ApplicationIdsRejectPaths);
Run("optional auth native and managed versions agree", AuthNativeAndManagedVersionsAgree);
Run("embedded auth failures cross the FFI boundary", EmbeddedAuthFailuresCrossFfi);

if (failures.Count != 0)
{
    foreach (var failure in failures)
    {
        Console.Error.WriteLine(failure);
    }
    return 1;
}

Console.WriteLine($"Stimpak managed tests passed ({passed}/{passed}).");
return 0;

void Run(string name, Action test)
{
    try
    {
        test();
        passed++;
    }
    catch (Exception error)
    {
        failures.Add($"{name}: {error.Message}");
    }
}

static User User(uint handle, uint presence, string name) =>
    new(handle, presence, name, null, Presence.Available);

static void ChannelHandlesAreScoped()
{
    var people = new PeopleRegistry();
    people.Apply(new RosterReceived(1, true, [User(7, 100, "Alice")]));
    people.Apply(new RosterReceived(2, true, [User(7, 200, "Bob")]));

    var alice = people.Get(1, 7);
    var bob = people.Get(2, 7);
    Assert(!ReferenceEquals(alice, bob), "equal handles from different channels were merged");
    Assert(alice.Name == "Alice" && bob.Name == "Bob", "channel identities contaminated each other");
    Assert(people.FindByPresenceId(100).Single() == alice, "presence index omitted Alice");
}

static void DisconnectResetsIdentities()
{
    var people = new PeopleRegistry();
    people.Apply(new RosterReceived(1, true, [User(7, 100, "Alice")]));
    people.Apply(new StageChanged(Stage.Disconnected));

    Assert(people.People.Count == 0, "known identities survived disconnect");
    Assert(people.Channel(1).Count == 0, "channel membership survived disconnect");
    var replacement = people.Get(1, 7);
    Assert(replacement.Name == "User 7", "a reused handle inherited an earlier connection's name");
}

static void RejoinedChannelResetsIdentities()
{
    var people = new PeopleRegistry();
    people.Apply(new RosterReceived(1, true, [User(7, 100, "Alice")]));
    people.Apply(new Joined(1, new PublicChannel(1028, "General"), 9));

    Assert(people.Channel(1).Count == 0, "old roster survived a reused channel index");
    Assert(people.Get(1, 7).Name == "User 7", "reused membership inherited an old name");
}

static void UnknownEventTypesSurvive()
{
    var decoded = EventJson.Deserialize("{\"type\":\"future_event\",\"answer\":42}");
    Assert(decoded is UnknownEvent { EventType: "future_event" }, "unknown event was not preserved");
}

static void MalformedEventsAreSurfaced()
{
    var decoded = EventJson.Deserialize(
        "{\"type\":\"message\",\"channel_index\":\"not-a-byte\"}");
    Assert(decoded is EventProtocolError, "schema mismatch was silently discarded");
}

static void AccountAndGroupEventsDecode()
{
    var authentication = EventJson.Deserialize(
        "{\"type\":\"authentication_required\",\"auth_id\":4," +
        "\"url\":\"https://example.com\",\"fresh_account\":true}");
    Assert(authentication is AuthenticationRequired { AuthId: 4, FreshAccount: true },
        "authentication request did not retain fresh-account intent");

    var account = EventJson.Deserialize(
        "{\"type\":\"account\",\"account\":{\"account_id\":42," +
        "\"battle_tag\":\"Medic#1234\",\"region\":1,\"games\":[\"S2\"]}}");
    Assert(account is AccountConnected { Account.AccountId: 42 }, "account event did not decode");

    var group = EventJson.Deserialize(
        "{\"type\":\"group_search\",\"club_ids\":[7,9]}");
    Assert(group is GroupSearchReceived { ClubIds.Count: 2 }, "group search did not decode");
}

static void ConnectionTargetsSerialize()
{
    var json = EventJson.SerializeTargets([
        ChannelTarget.Public(1028),
        ChannelTarget.Private("Bots"),
        ChannelTarget.Group(7),
    ]);
    Assert(json.Contains("\"kind\":\"public\"", StringComparison.Ordinal),
        "public target omitted its discriminator");
    Assert(json.Contains("\"club_id\":7", StringComparison.Ordinal),
        "group target omitted its id");
}

static void NativeAndManagedVersionsAgree()
{
    Assert(StimpakClient.NativeAbiVersion == StimpakClient.SupportedNativeAbi, "ABI mismatch");
    Assert(StimpakClient.NativeEventSchemaVersion == StimpakClient.SupportedEventSchema,
        "event schema mismatch");

    var credential = Path.Combine(Path.GetTempPath(), $"stimpak-{Guid.NewGuid():N}.bin");
    using var client = new StimpakClient(new StimpakClientOptions("Stimpak.Tests")
    {
        CredentialPath = credential,
    });
    Assert(StimpakClient.NativeVersion.Length != 0, "native version is empty");
    AssertReleaseVersion(
        StimpakClient.NativeVersion,
        typeof(StimpakClient).Assembly,
        "Stimpak");
}

static void AuthNativeAndManagedVersionsAgree()
{
    Assert(EmbeddedAuthenticator.NativeAbiVersion == EmbeddedAuthenticator.SupportedNativeAbi,
        "authentication ABI mismatch");
    Assert(EmbeddedAuthenticator.NativeVersion.Length != 0,
        "authentication native version is empty");
    AssertReleaseVersion(
        EmbeddedAuthenticator.NativeVersion,
        typeof(EmbeddedAuthenticator).Assembly,
        "Stimpak.Auth");
}

static void AssertReleaseVersion(string nativeVersion, Assembly managedAssembly, string name)
{
    var expected = Environment.GetEnvironmentVariable("STIMPAK_PACKAGE_VERSION");
    if (string.IsNullOrEmpty(expected))
    {
        return;
    }
    var managedVersion = managedAssembly
        .GetCustomAttribute<AssemblyInformationalVersionAttribute>()?
        .InformationalVersion;
    Assert(nativeVersion == expected, $"{name} native version is {nativeVersion}, expected {expected}");
    Assert(
        managedVersion?.Split('+')[0] == expected,
        $"{name} managed version is {managedVersion}, expected {expected}");
}

static void ApplicationIdsRejectPaths()
{
    try
    {
        using var _ = new StimpakClient("../another-app");
        throw new InvalidOperationException("path-like application id was accepted");
    }
    catch (ArgumentException error)
    {
        Assert(error.Message.Contains("ApplicationId", StringComparison.Ordinal),
            "application-id validation did not explain the failure");
    }
}

static void EmbeddedAuthFailuresCrossFfi()
{
    var authenticator = new EmbeddedAuthenticator(new SynchronizationContext());
    var request = new AuthenticationRequired(9, "not a url", false);
    try
    {
        _ = authenticator.AuthenticateAsync(request).AsTask().GetAwaiter().GetResult();
        throw new InvalidOperationException("invalid URL unexpectedly authenticated");
    }
    catch (StimpakAuthenticationException error)
    {
        Assert(error.Message.Contains("invalid authentication URL", StringComparison.Ordinal),
            "native authentication error detail was lost");
    }
}

static void Assert(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidOperationException(message);
    }
}
