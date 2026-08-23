namespace Stimpak.Auth;

public static class AuthenticationExtensions
{
    /// <summary>
    /// Lets an optional provider answer one <see cref="AuthenticationRequired"/>
    /// event, including the request id and cancellation bookkeeping.
    /// </summary>
    public static async ValueTask CompleteAuthenticationAsync(
        this StimpakClient client,
        AuthenticationRequired request,
        IStimpakAuthenticator authenticator,
        CancellationToken cancellation = default)
    {
        ArgumentNullException.ThrowIfNull(client);
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(authenticator);

        string? token;
        try
        {
            token = await authenticator.AuthenticateAsync(request, cancellation);
        }
        catch
        {
            TryCancel(client, request.AuthId);
            throw;
        }

        if (string.IsNullOrWhiteSpace(token))
        {
            client.CancelAuth(request.AuthId);
        }
        else
        {
            client.SubmitAuth(request.AuthId, token);
        }
    }

    private static void TryCancel(StimpakClient client, ulong authId)
    {
        try
        {
            client.CancelAuth(authId);
        }
        catch (StimpakException)
        {
            // Preserve the provider or cancellation failure that brought us here.
        }
    }
}
