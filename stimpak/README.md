# Stimpak

Stimpak is the supported StarCraft II bot binding for Superiority's Battle.net
client. It deliberately stays SC2-only. The native library exposes a small C
ABI and tagged JSON events; the managed package turns those events into typed
.NET records and an async stream.

## .NET quick start

During repository development, reference `csharp/Stimpak/Stimpak.csproj`. A
release build compiles the Rust library in Cargo's release profile and copies
it beside the managed assembly.

```csharp
using Stimpak;

using var client = new StimpakClient("com.example.MyBot");
client.Connect(); // General is joined before Connected is emitted.

await foreach (var next in client.ReadEventsAsync(stopping.Token))
{
    client.People.Apply(next);
    if (next is MessageReceived { Body: "!ping" } message)
    {
        client.SendMessage(message.ChannelIndex, "pong");
    }
}
```

Connect to a known account and restore several channels in one attempt:

```csharp
client.Connect(new StimpakConnectOptions
{
    ExpectedAccountId = savedAccountId,
    Channels =
    [
        ChannelTarget.Public(StimpakClient.DefaultPublicChannel),
        ChannelTarget.Private("Bot Workshop"),
        ChannelTarget.Group(535_241),
    ],
});
```

Authentication is always explicit. Handle `AuthenticationRequired` and call
`SubmitAuth` or `CancelAuth`. Applications that want a ready-made UI can add
the separate `Stimpak.Auth` package and answer the event in-process:

```csharp
using Stimpak.Auth;

var authenticator = new EmbeddedAuthenticator();

await foreach (var next in client.ReadEventsAsync(stopping.Token))
{
    if (next is AuthenticationRequired request)
        await client.CompleteAuthenticationAsync(request, authenticator, stopping.Token);
}
```

The optional provider must be invoked from a UI synchronization context. It
launches no child executable; the host's AppKit or Win32 loop drives its native
WebView. The application id selects a product-specific credential in the
platform's per-user application-data directory. It is a namespace, not an
encryption key. See [`AUTH.md`](AUTH.md).

An application that genuinely needs to own the path can opt out explicitly:

```csharp
using var client = new StimpakClient(new StimpakClientOptions("com.example.MyBot")
{
    CredentialPath = customPath,
});
```

`ReadEventsAsync` is bounded and coalesces unread roster snapshots for the same
channel. Subscribe to `EventOverflowed` or inspect `DroppedEventCount` if a
consumer may fall behind. `EventReceived` receives every event synchronously on
the pump thread and should only hand work off quickly.

## Native C ABI

Build with:

```sh
cargo build --release -p stimpak -p stimpak-auth
```

Include [`include/stimpak.h`](include/stimpak.h), open one client, issue
commands, and poll JSON events with `stimpak_client_poll`. Free every returned
string with `stimpak_string_free`, and close the client exactly once. The C ABI
and event vocabulary have independent integer versions so a binding can reject
an incompatible native library before opening a client.

The complete tagged event contract is in [`EVENTS.md`](EVENTS.md).

## Tests

All repository builds and tests use release profiles:

```sh
cargo test --release -p stimpak
dotnet run --project stimpak/csharp/Stimpak.Tests/Stimpak.Tests.csproj -c Release
dotnet build stimpak/csharp/ExampleBot/ExampleBot.csproj -c Release
```

## Packing

`Stimpak.csproj` and `Stimpak.Auth.csproj` are separately packable. Prebuilt,
signed native artifacts go under:

```text
stimpak/csharp/artifacts/runtimes/<rid>/native/
stimpak/csharp/Stimpak.Auth/artifacts/runtimes/<rid>/native/
```

For example, the base package contains `libstimpak.dylib` and the optional
package contains `libstimpak_auth.dylib`. Then run
`scripts/package-stimpak-nuget.zsh`.
Build Windows binaries with `scripts/build-stimpak-windows-macos.zsh`, then set
`STIMPAK_WINDOWS_CERTIFICATE` and `STIMPAK_WINDOWS_PASSWORD_FILE` and run
`scripts/sign-stage-stimpak-windows-macos.zsh`. Both operations stay on macOS;
the packaging script rejects Windows artifacts whose signatures do not verify.
