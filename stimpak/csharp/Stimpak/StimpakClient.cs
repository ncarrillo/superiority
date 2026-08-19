using System.Runtime.InteropServices;
using System.Text.Json;
using System.Threading.Channels;

namespace Stimpak;

public sealed class StimpakException(string message) : Exception(message);

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
/// using var client = new StimpakClient("bot-credentials.bin");
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
    /// <summary>
    /// how long the pump parks per wait. not a latency budget: an event
    /// arriving mid-wait returns immediately. it only bounds how quickly the
    /// pump notices it has been disposed.
    /// </summary>
    private static readonly TimeSpan PumpSlice = TimeSpan.FromMilliseconds(250);

    /// <summary>
    /// how long dispose waits for the pump to leave native code. beyond this
    /// the pump takes over closing, so a stuck sign-in cannot hold up a host's
    /// shutdown and cannot be freed out from under either.
    /// </summary>
    private static readonly TimeSpan CloseTimeout = TimeSpan.FromSeconds(1);

    private static readonly JsonSerializerOptions Options = new();

    private readonly Channel<SC2Event> _events = Channel.CreateUnbounded<SC2Event>(
        new UnboundedChannelOptions { SingleWriter = true, SingleReader = false });

    private readonly Thread _pump;
    private IntPtr _handle;
    private volatile bool _stopping;
    private int _closeRequested;

    /// <summary>
    /// raised on the pump thread. handlers must not block; anything slow
    /// belongs on <see cref="ReadEventsAsync"/>.
    /// </summary>
    public event Action<SC2Event>? EventReceived;

    /// <summary>
    /// who this session knows about. owned here because handles are issued by
    /// one connection and mean nothing in the next, so the registry has to die
    /// with the session that filled it.
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

    /// <summary>
    /// false means sign-in falls to you: handle <see cref="AuthenticationRequired"/>
    /// and call <see cref="SubmitAuth"/>.
    /// </summary>
    public bool HasAuthWindow => Native.HasAuthWindow(Handle);

    /// <param name="credentialPath">
    /// where this client caches its signed-in session. required: the
    /// superiority app owns its own cache, and two programs sharing one file
    /// means either can sign the other out.
    /// </param>
    /// <param name="authWindowPath">
    /// the stimpak-auth-window executable. when this, STIMPAK_AUTH_WINDOW, or a sibling
    /// of the running executable finds it, sign-in happens in a window the
    /// library opens and <see cref="AuthenticationRequired"/> never arrives.
    /// </param>
    public StimpakClient(string credentialPath, string? authWindowPath = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(credentialPath);
        _handle = Native.Open(credentialPath, authWindowPath);
        if (_handle == IntPtr.Zero)
        {
            throw new StimpakException($"could not open a client against {credentialPath}");
        }
        _pump = new Thread(Pump)
        {
            Name = "sc2-events",
            IsBackground = true,
        };
        _pump.Start();
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
            Native.ErrAuthFailed => "the sign-in window did not return a token",
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

            SC2Event? next;
            try
            {
                var json = Marshal.PtrToStringUTF8(raw);
                next = json is null ? null : JsonSerializer.Deserialize<SC2Event>(json, Options);
            }
            catch (JsonException)
            {
                next = null;
            }
            finally
            {
                Native.FreeString(raw);
            }

            if (next is null)
            {
                continue;
            }

            _events.Writer.TryWrite(next);
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
        _events.Writer.TryComplete();
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
        Check(Native.Connect(Handle, forceInteractive));

    /// <summary>drops the session; the client can connect again afterwards.</summary>
    public void Disconnect() => Check(Native.Disconnect(Handle));

    public void JoinPublic(ushort channelId) => Check(Native.JoinPublic(Handle, channelId));

    /// <summary>creates the channel if it is empty.</summary>
    public void JoinPrivate(string name)
    {
        ArgumentNullException.ThrowIfNull(name);
        Check(Native.JoinPrivate(Handle, name));
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

    /// <summary>completes an <see cref="AuthenticationRequired"/> sign-in.</summary>
    public void SubmitAuth(ulong authId, string token)
    {
        ArgumentNullException.ThrowIfNull(token);
        Check(Native.SubmitAuth(Handle, authId, token));
    }

    /// <summary>
    /// every event as it arrives. suspends rather than blocking, so it costs
    /// no thread while idle.
    /// </summary>
    public IAsyncEnumerable<SC2Event> ReadEventsAsync(CancellationToken cancellation = default) =>
        _events.Reader.ReadAllAsync(cancellation);

    /// <summary>for driving the client from an existing loop instead of a consumer.</summary>
    public bool TryRead(out SC2Event? next) => _events.Reader.TryRead(out next);

    public void Dispose()
    {
        _stopping = true;
        if (Interlocked.Exchange(ref _closeRequested, 1) == 1)
        {
            return;
        }
        _events.Writer.TryComplete();

        // disposing from a subscriber runs on the pump itself; joining would
        // wait on this very thread. let the loop unwind and close on its way
        // out instead.
        if (Thread.CurrentThread == _pump)
        {
            return;
        }

        // the client is only freed once the pump is provably out of native
        // code. a sign-in window can hold it for minutes, and freeing under it
        // would be a use-after-free — so if the wait expires, the pump closes.
        if (_pump.Join(CloseTimeout))
        {
            CloseHandle();
        }
    }
}
