using Stimpak;

namespace ExampleBot.Services;

public sealed class StimpakSession(string credentialPath, string? authWindowPath = null)
    : IChatSession
{
    private readonly StimpakClient _client = new(credentialPath, authWindowPath);

    public bool HasAuthWindow => _client.HasAuthWindow;

    public PeopleRegistry People => _client.People;

    public void Connect() => _client.Connect();

    public void JoinPublic(ushort channelId) => _client.JoinPublic(channelId);

    public void SendMessage(byte channelIndex, string body) =>
        _client.SendMessage(channelIndex, body);

    public void SendWhisper(string name, string body) => _client.SendWhisper(name, body);

    public void SubmitAuth(ulong authId, string token) => _client.SubmitAuth(authId, token);

    public IAsyncEnumerable<SC2Event> ReadEventsAsync(CancellationToken cancellation) =>
        _client.ReadEventsAsync(cancellation);

    public void Dispose() => _client.Dispose();
}
