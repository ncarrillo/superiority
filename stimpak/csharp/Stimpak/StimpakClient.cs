using System.Runtime.InteropServices;

namespace Stimpak;

public sealed class StimpakException(string message, Exception? innerException = null)
    : Exception(message, innerException);

/// <summary>a starcraft ii chat session.</summary>
/// <remarks>
/// commands return as soon as the session thread accepts them; none touch the
/// network on your thread. events are pushed: one pump thread blocks on the
/// native session, waking the moment an event exists, and republishes into a
/// channel. the pump owns its own thread rather than a pool thread because it
/// spends its life blocked.
/// </remarks>
/// <example>
/// <code>
/// using var client = new StimpakClient("com.example.MyBot");
/// client.Connect();
/// await foreach (var e in client.ReadEventsAsync(cancellation))
/// {
///     if (e is MessageReceived m &amp;&amp; m.Body == "!ping")
///         client.SendMessage(m.ChannelIndex, "pong");
/// }
/// </code>
/// </example>
public sealed class StimpakClient : IDisposable
{
    public const uint SupportedNativeAbi = 3;
    public const uint SupportedEventSchema = 2;

    /// <summary>
    /// how long the pump parks per wait. not a latency budget: an event
    /// arriving mid-wait returns immediately. it only bounds how quickly the
    /// pump notices it has been disposed.
    /// </summary>
    private static readonly TimeSpan PumpSlice = TimeSpan.FromMilliseconds(250);

    /// <summary>
    /// how long dispose waits for the pump to leave native code. Beyond this
    /// the pump takes over closing, so a stalled native poll cannot hold up a
    /// host's shutdown and cannot be freed out from under either.
    /// </summary>
    private static readonly TimeSpan CloseTimeout = TimeSpan.FromSeconds(1);

    private readonly EventBuffer _events;
    private readonly Thread _pump;
    private IntPtr _handle;
    private volatile bool _stopping;
    private int _closeRequested;
    private StimpakConnectOptions? _lastConnect;

    /// <summary>
    /// raised on the pump thread. handlers must not block; anything slow
    /// belongs on <see cref="ReadEventsAsync"/>.
    /// </summary>
    public event Action<SC2Event>? EventReceived;

    /// <summary>
    /// raised on the pump thread when a full managed stream discards its oldest
    /// unread event. The argument is the lifetime total.
    /// </summary>
    public event Action<long>? EventOverflowed;

    /// <summary>
    /// who this connection knows about. Handles are issued by one connection
    /// and mean nothing in the next, so a disconnected stage clears it.
    /// </summary>
    /// <remarks>
    /// it is not fed automatically. call <see cref="PeopleRegistry.Apply"/> as
    /// you handle each event, from whichever thread you handle them on — for a
    /// ui, the one that owns the dispatcher. feeding it from the pump would put
    /// change notifications on a thread the bindings do not live on.
    /// </remarks>
    public PeopleRegistry People { get; } = new();

    /// <summary>the general channel's id.</summary>
    public static ushort DefaultPublicChannel => Native.DefaultPublicChannel();

    public static string NativeVersion =>
        Marshal.PtrToStringUTF8(Native.Version()) ?? "unknown";

    public static uint NativeAbiVersion => Native.AbiVersion();

    public static uint NativeEventSchemaVersion => Native.EventSchemaVersion();

    public long DroppedEventCount => _events.DroppedCount;

    /// <param name="applicationId">
    /// stable credential namespace, preferably reverse-DNS. Stimpak chooses
    /// the platform's per-user application-data directory.
    /// </param>
    public StimpakClient(string applicationId)
        : this(new StimpakClientOptions(applicationId))
    {
    }

    public StimpakClient(StimpakClientOptions options)
    {
        ArgumentNullException.ThrowIfNull(options);
        ValidateApplicationId(options.ApplicationId);
        if (options.CredentialPath is not null)
        {
            ArgumentException.ThrowIfNullOrWhiteSpace(options.CredentialPath);
        }
        ArgumentOutOfRangeException.ThrowIfLessThan(options.EventCapacity, 1);
        EnsureCompatible();
        _events = new EventBuffer(options.EventCapacity);
        _handle = options.CredentialPath is { } path
            ? Native.OpenAtPath(Path.GetFullPath(path))
            : Native.Open(options.ApplicationId);
        if (_handle == IntPtr.Zero)
        {
            throw new StimpakException(
                $"could not open a client for application {options.ApplicationId}");
        }
        _pump = new Thread(Pump)
        {
            Name = "sc2-events",
            IsBackground = true,
        };
        _pump.Start();
    }

    private static void ValidateApplicationId(string applicationId)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(applicationId);
        if (applicationId.Length > 128 ||
            !IsAsciiLetterOrDigit(applicationId[0]) ||
            applicationId.Any(character =>
                !IsAsciiLetterOrDigit(character) && character is not ('.' or '-' or '_')))
        {
            throw new ArgumentException(
                "ApplicationId must start with an ASCII letter or digit and contain only " +
                "letters, digits, '.', '-' or '_'.",
                nameof(applicationId));
        }
    }

    private static bool IsAsciiLetterOrDigit(char value) =>
        value is >= 'a' and <= 'z' or >= 'A' and <= 'Z' or >= '0' and <= '9';

    private static void EnsureCompatible()
    {
        uint abi;
        uint schema;
        try
        {
            abi = Native.AbiVersion();
            schema = Native.EventSchemaVersion();
        }
        catch (Exception error) when (
            error is DllNotFoundException or EntryPointNotFoundException or BadImageFormatException)
        {
            throw new StimpakException(
                $"the native Stimpak library is missing or incompatible: {error.Message}", error);
        }
        if (abi != SupportedNativeAbi || schema != SupportedEventSchema)
        {
            throw new StimpakException(
                $"native compatibility mismatch: managed ABI/schema " +
                $"{SupportedNativeAbi}/{SupportedEventSchema}, native {abi}/{schema}");
        }
    }

    private IntPtr Handle =>
        _handle != IntPtr.Zero ? _handle : throw new ObjectDisposedException(nameof(StimpakClient));

    private static void Check(int code)
    {
        if (code == Native.Ok)
        {
            return;
        }
        throw new StimpakException(code switch
        {
            Native.ErrInvalidArgument => "invalid argument",
            Native.ErrDisconnected => "the session has ended",
            Native.ErrNoSuchAuth => "no sign-in is waiting for that id",
            Native.ErrPanic => "the native library faulted",
            _ => $"unexpected error {code}",
        });
    }

    private void Pump()
    {
        var milliseconds = (uint)PumpSlice.TotalMilliseconds;
        while (!_stopping)
        {
            var handle = _handle;
            if (handle == IntPtr.Zero)
            {
                break;
            }

            // null is the idle case, not a failure.
            var raw = Native.Poll(handle, milliseconds);
            if (raw == IntPtr.Zero)
            {
                continue;
            }

            SC2Event next;
            try
            {
                var json = Marshal.PtrToStringUTF8(raw);
                next = json is null
                    ? new EventProtocolError("native event string was not UTF-8", "")
                    : EventJson.Deserialize(json);
            }
            finally
            {
                Native.FreeString(raw);
            }

            if (_events.Publish(next) != 0)
            {
                try
                {
                    EventOverflowed?.Invoke(_events.DroppedCount);
                }
                catch (Exception)
                {
                    // diagnostics obey the same isolation as event subscribers.
                }
            }
            try
            {
                EventReceived?.Invoke(next);
            }
            catch (Exception)
            {
                // a subscriber's fault must not take the session down.
            }

            if (next is SessionEnded)
            {
                break;
            }
        }
        _events.Complete();
        // dispose may have given up waiting and left the close to us, because
        // freeing the client while this thread was inside a native call would
        // have been a use-after-free.
        if (Volatile.Read(ref _closeRequested) == 1)
        {
            CloseHandle();
        }
    }

    private void CloseHandle()
    {
        var handle = Interlocked.Exchange(ref _handle, IntPtr.Zero);
        if (handle != IntPtr.Zero)
        {
            Native.Close(handle);
        }
    }

    /// <summary>
    /// progress arrives as <see cref="StageChanged"/>; usable at
    /// <see cref="Stage.Connected"/>.
    /// </summary>
    /// <param name="forceInteractive">
    /// bypass the cached credential and force the browser flow. a bot leaves
    /// this false so it can start unattended.
    /// </param>
    public void Connect(bool forceInteractive = false) =>
        Connect(new StimpakConnectOptions { ForceInteractive = forceInteractive });

    /// <summary>
    /// connects and joins every requested channel before the connected stage.
    /// Empty <see cref="StimpakConnectOptions.Channels"/> means General.
    /// </summary>
    public void Connect(StimpakConnectOptions options)
    {
        ArgumentNullException.ThrowIfNull(options);
        if (options.ExpectedAccountId == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(options), "an expected account id must be non-zero");
        }
        var channels = options.Channels?.ToArray() ??
            throw new ArgumentException("channels cannot be null", nameof(options));
        foreach (var channel in channels)
        {
            ArgumentNullException.ThrowIfNull(channel);
            switch (channel)
            {
                case PrivateChannelTarget privateChannel:
                    ArgumentException.ThrowIfNullOrWhiteSpace(privateChannel.Name);
                    break;
                case GroupChannelTarget { ClubId: 0 }:
                    throw new ArgumentOutOfRangeException(
                        nameof(options), "a group id must be non-zero");
            }
        }
        var snapshot = options with { Channels = channels };
        var json = EventJson.SerializeTargets(channels);
        Check(Native.ConnectConfigured(
            Handle,
            snapshot.ForceInteractive,
            snapshot.ExpectedAccountId ?? 0,
            json));
        _lastConnect = snapshot;
    }

    /// <summary>disconnects and restores the preceding connection's channels.</summary>
    public void Reconnect(bool forceInteractive = false)
    {
        Disconnect();
        Connect((_lastConnect ?? new StimpakConnectOptions()) with
        {
            ForceInteractive = forceInteractive,
        });
    }

    /// <summary>drops the session; the client can connect again afterwards.</summary>
    public void Disconnect() => Check(Native.Disconnect(Handle));

    /// <summary>disconnects and deletes this client's cached credential.</summary>
    public void SignOut() => Check(Native.SignOut(Handle));

    public void JoinPublic(ushort channelId) => Check(Native.JoinPublic(Handle, channelId));

    /// <summary>creates the channel if it is empty.</summary>
    public void JoinPrivate(string name)
    {
        ArgumentNullException.ThrowIfNull(name);
        Check(Native.JoinPrivate(Handle, name));
    }

    public void JoinGroup(uint clubId)
    {
        ArgumentOutOfRangeException.ThrowIfZero(clubId);
        Check(Native.JoinGroup(Handle, clubId));
    }

    public void SearchGroups(string query)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(query);
        Check(Native.SearchGroups(Handle, query));
    }

    /// <summary><paramref name="channelIndex"/> is the one the <see cref="Joined"/> event carried.</summary>
    public void Leave(byte channelIndex) => Check(Native.Leave(Handle, channelIndex));

    public void SendMessage(byte channelIndex, string body)
    {
        ArgumentNullException.ThrowIfNull(body);
        Check(Native.SendMessage(Handle, channelIndex, body));
    }

    /// <summary>pass a <see cref="WhisperReceived.Peer"/> straight back to reply.</summary>
    public void SendWhisper(string name, string body)
    {
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(body);
        Check(Native.SendWhisper(Handle, name, body));
    }

    public void AnswerGroupInvitation(uint clubId, bool accept) =>
        Check(Native.AnswerGroupInvitation(Handle, clubId, accept));

    public void AnswerPartyInvitation(byte channelIndex, bool accept) =>
        Check(Native.AnswerPartyInvitation(Handle, channelIndex, accept));

    /// <summary>completes an <see cref="AuthenticationRequired"/> sign-in.</summary>
    public void SubmitAuth(ulong authId, string token)
    {
        ArgumentNullException.ThrowIfNull(token);
        Check(Native.SubmitAuth(Handle, authId, token));
    }

    public void CancelAuth(ulong authId) => Check(Native.CancelAuth(Handle, authId));

    /// <summary>
    /// unread roster snapshots for the same channel are coalesced. The stream
    /// is bounded and suspends rather than blocking while idle.
    /// </summary>
    public IAsyncEnumerable<SC2Event> ReadEventsAsync(CancellationToken cancellation = default) =>
        _events.ReadAllAsync(cancellation);

    /// <summary>for driving the client from an existing loop instead of a consumer.</summary>
    public bool TryRead(out SC2Event? next) => _events.TryRead(out next);

    public void Dispose()
    {
        _stopping = true;
        if (Interlocked.Exchange(ref _closeRequested, 1) == 1)
        {
            return;
        }
        _events.Complete();

        // disposing from a subscriber runs on the pump itself; joining would
        // wait on this very thread. let the loop unwind and close on its way
        // out instead.
        if (Thread.CurrentThread == _pump)
        {
            return;
        }

        // the client is only freed once the pump is provably out of native
        // code. If the wait expires, the pump closes after the poll unwinds.
        if (_pump.Join(CloseTimeout))
        {
            CloseHandle();
        }
    }
}
