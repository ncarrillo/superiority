namespace Stimpak.Auth;

/// <summary>obtains the Battle.net session token for one explicit request.</summary>
public interface IStimpakAuthenticator
{
    ValueTask<string?> AuthenticateAsync(
        AuthenticationRequired request,
        CancellationToken cancellation = default);
}

public sealed class StimpakAuthenticationException(string message, Exception? inner = null)
    : Exception(message, inner);
