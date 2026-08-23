using System.Runtime.InteropServices;

namespace Stimpak.Auth;

/// <summary>
/// Presents Battle.net in a native WebView inside the host process. Call from
/// a UI context; that context's AppKit or Win32 loop drives the window.
/// </summary>
public sealed class EmbeddedAuthenticator : IStimpakAuthenticator
{
    public const uint SupportedNativeAbi = 1;

    private static readonly Native.Completion Completion = Complete;
    private readonly SynchronizationContext? _uiContext;

    public EmbeddedAuthenticator(SynchronizationContext? uiContext = null)
    {
        _uiContext = uiContext;
    }

    public static uint NativeAbiVersion => Native.AbiVersion();

    public static string NativeVersion =>
        Marshal.PtrToStringUTF8(Native.Version()) ?? "unknown";

    public async ValueTask<string?> AuthenticateAsync(
        AuthenticationRequired request,
        CancellationToken cancellation = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        cancellation.ThrowIfCancellationRequested();
        EnsureCompatible();

        var uiContext = _uiContext ?? SynchronizationContext.Current ??
            throw new InvalidOperationException(
                "Embedded authentication must be started from a UI synchronization context.");
        var completion = new TaskCompletionSource<string?>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var state = new CallbackState(completion, cancellation);
        var root = GCHandle.Alloc(state);
        var session = IntPtr.Zero;

        try
        {
            session = Native.Present(
                request.Url,
                request.FreshAccount,
                Completion,
                GCHandle.ToIntPtr(root));
            if (session == IntPtr.Zero)
            {
                throw new StimpakAuthenticationException(
                    "The embedded authentication library could not create a session.");
            }

            using var registration = cancellation.Register(() =>
                uiContext.Post(_ => Native.Cancel(session), null));
            return await completion.Task;
        }
        catch (Exception error) when (
            error is DllNotFoundException or EntryPointNotFoundException or BadImageFormatException)
        {
            throw new StimpakAuthenticationException(
                $"The native embedded authentication library is missing or incompatible: {error.Message}",
                error);
        }
        finally
        {
            if (session != IntPtr.Zero)
            {
                Native.Close(session);
            }
            if (root.IsAllocated)
            {
                root.Free();
            }
        }
    }

    private static void EnsureCompatible()
    {
        uint actual;
        try
        {
            actual = Native.AbiVersion();
        }
        catch (Exception error) when (
            error is DllNotFoundException or EntryPointNotFoundException or BadImageFormatException)
        {
            throw new StimpakAuthenticationException(
                $"The native embedded authentication library is missing or incompatible: {error.Message}",
                error);
        }
        if (actual != SupportedNativeAbi)
        {
            throw new StimpakAuthenticationException(
                $"Embedded authentication ABI mismatch: managed {SupportedNativeAbi}, native {actual}.");
        }
    }

    private static void Complete(IntPtr context, int status, IntPtr detail)
    {
        if (context == IntPtr.Zero || GCHandle.FromIntPtr(context).Target is not CallbackState state)
        {
            return;
        }
        var message = detail == IntPtr.Zero ? null : Marshal.PtrToStringUTF8(detail);
        switch (status)
        {
            case 0 when !string.IsNullOrEmpty(message):
                state.Completion.TrySetResult(message);
                break;
            case 1 when state.Cancellation.IsCancellationRequested:
                state.Completion.TrySetCanceled(state.Cancellation);
                break;
            case 1:
                state.Completion.TrySetResult(null);
                break;
            default:
                state.Completion.TrySetException(new StimpakAuthenticationException(
                    message ?? "Embedded authentication failed."));
                break;
        }
    }

    private sealed record CallbackState(
        TaskCompletionSource<string?> Completion,
        CancellationToken Cancellation);
}
