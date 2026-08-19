using Stimpak;

namespace ExampleBot.Services;

/// <summary>
/// what the view model needs from a chat session, and nothing more. the real
/// one wraps <see cref="StimpakClient"/>; the sample one replays canned events,
/// which is what lets the window be previewed and tested without a battle.net
/// account behind it.
/// </summary>
public interface IChatSession : IDisposable
{
    /// <summary>false means sign-in falls to the caller.</summary>
    bool HasAuthWindow { get; }

    /// <summary>
    /// who this session knows about. lives and dies with the session, because
    /// the handles in it are only meaningful within one.
    /// </summary>
    PeopleRegistry People { get; }

    void Connect();

    void JoinPublic(ushort channelId);

    void SendMessage(byte channelIndex, string body);

    void SendWhisper(string name, string body);

    void SubmitAuth(ulong authId, string token);

    IAsyncEnumerable<SC2Event> ReadEventsAsync(CancellationToken cancellation);
}
