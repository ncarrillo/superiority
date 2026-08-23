# Stimpak

Stimpak is a .NET client for StarCraft II Battle.net chat.

## Install

```sh
dotnet add package Stimpak
```

Add `Stimpak.Auth` for the optional in-process authentication UI.

```sh
dotnet add package Stimpak.Auth
```

## Example

```csharp
using Stimpak;

using var client = new StimpakClient("com.example.MyBot");
client.Connect();

await foreach (var next in client.ReadEventsAsync())
{
    client.People.Apply(next);

    if (next is MessageReceived { Body: "!ping" } message)
        client.SendMessage(message.ChannelIndex, "pong");
}
```

A new account emits `AuthenticationRequired`. Complete it with `SubmitAuth`,
or use [`Stimpak.Auth`](AUTH.md) from a UI application.

Event reference: [`EVENTS.md`](EVENTS.md).

C header: [`include/stimpak.h`](include/stimpak.h).

## Development

```sh
cargo test --release -p stimpak -p stimpak-auth
dotnet run --project stimpak/csharp/Stimpak.Tests/Stimpak.Tests.csproj -c Release
dotnet build stimpak/csharp/ExampleBot/ExampleBot.csproj -c Release
```
