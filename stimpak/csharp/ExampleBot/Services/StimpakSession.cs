using Stimpak;
using Stimpak.Auth;

namespace ExampleBot.Services;

public sealed class StimpakSession(StimpakClientOptions options)
    : IChatSession
{
    private readonly StimpakClient _client = new(options);
    private readonly EmbeddedAuthenticator _authenticator = new();

    public PeopleRegistry People => _client.People;

    public void Connect() => _client.Connect();

    public void SendMessage(byte channelIndex, string body) =>
        _client.SendMessage(channelIndex, body);

    public void SendWhisper(string name, string body) => _client.SendWhisper(name, body);

    public ValueTask CompleteAuthenticationAsync(
        AuthenticationRequired request,
        CancellationToken cancellation) =>
        _client.CompleteAuthenticationAsync(request, _authenticator, cancellation);

    public IAsyncEnumerable<SC2Event> ReadEventsAsync(CancellationToken cancellation) =>
        _client.ReadEventsAsync(cancellation);

    public void Dispose() => _client.Dispose();
}
