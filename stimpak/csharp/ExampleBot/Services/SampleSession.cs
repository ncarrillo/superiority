using System.Runtime.CompilerServices;
using Stimpak;

namespace ExampleBot.Services;

/// <summary>
/// a session that never touches the network, so the avalonia designer shows a
/// populated window instead of an empty shell.
/// </summary>
public sealed class SampleSession : IChatSession
{
    public bool HasAuthWindow => true;

    public PeopleRegistry People { get; } = new();

    public void Connect()
    {
    }

    public void SendMessage(byte channelIndex, string body)
    {
    }

    public void SendWhisper(string name, string body)
    {
    }

    public void SubmitAuth(ulong authId, string token)
    {
    }

    public async IAsyncEnumerable<SC2Event> ReadEventsAsync(
        [EnumeratorCancellation] CancellationToken cancellation)
    {
        foreach (var next in Script())
        {
            cancellation.ThrowIfCancellationRequested();
            yield return next;
        }
        await Task.Delay(Timeout.Infinite, cancellation).ConfigureAwait(false);
    }

    private static IEnumerable<SC2Event> Script()
    {
        yield return new StageChanged(Stage.ChatBootstrap);
        yield return new PublicChannelsReceived([new PublicChannel(1028, "General")]);
        yield return new Joined(0, new PublicChannel(1028, "General"), 1);
        yield return new RosterReceived(0, true, [
            Person("MarshalRaynor", null, Presence.InGame),
            Person("QueenOfBlades", null, Presence.Available),
            Person("Hierarch", "MDGTN", Presence.Away),
            Person("NelsonTest91", null, Presence.Available),
            Person("Tychus", "SWM", Presence.Busy),
            Person("Zeratul", null, Presence.Available),
            Person("Carlos Perez", null, Presence.Offline),
        ]);
        yield return new StageChanged(Stage.Connected);
        yield return new MessageReceived(0, Person("MarshalRaynor", null, Presence.InGame),
            "anyone up for a 2v2?");
        yield return new MessageReceived(0, Person("QueenOfBlades", null, Presence.Available),
            "queue it up, I'll grab the third");
        yield return new MemberJoined(0, Person("Sakura", null, Presence.Available));
        yield return new MessageReceived(0, Person("Hierarch", "MDGTN", Presence.Away), "!ping");
        yield return new WhisperReceived("NelsonTest91", "gg earlier, want a rematch?", false);
        yield return new MessageReceived(0, Person("Tychus", "SWM", Presence.Busy),
            "give me ten minutes and I'm in");
    }

    private static uint _handle = 1;

    private static User Person(string name, string? clan, Presence presence) =>
        new(_handle++, null, clan is null ? name : $"<{clan}> {name}", clan, presence);

    public void Dispose()
    {
    }
}
