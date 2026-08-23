# Stimpak.Auth

`Stimpak.Auth` is the optional, in-process Battle.net authentication UI for
Stimpak. The base package always emits `AuthenticationRequired`; install this
package only when the application wants a native embedded WebView to answer it.

```csharp
using Stimpak;
using Stimpak.Auth;

var authenticator = new EmbeddedAuthenticator();

await foreach (var next in client.ReadEventsAsync(stopping))
{
    if (next is AuthenticationRequired request)
        await client.CompleteAuthenticationAsync(request, authenticator, stopping);
}
```

Call `CompleteAuthenticationAsync` from the UI synchronization context. The
native window is created on that thread, returns immediately, and is driven by
the AppKit or Win32 event loop the host already owns. Closing the window calls
`CancelAuth`; completing Battle.net calls `SubmitAuth` with the request id and
session token.

The package launches no child process. A distributing application includes the
native library inside its final app bundle or package and signs it together
with the rest of its nested code.

The `StimpakClient` application id selects a cache below the current user's
platform application-data directory. It only namespaces the cache; it is not a
secret or an encryption key. On Unix, newly created directories and credential
files use owner-only permissions. An explicit `CredentialPath` remains
available for applications that need to own storage policy; Stimpak does not
change the permissions of an existing caller-owned directory.
