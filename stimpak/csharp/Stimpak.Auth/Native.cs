using System.Runtime.InteropServices;

namespace Stimpak.Auth;

internal static class Native
{
    internal const string Library = "stimpak_auth";

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate void Completion(IntPtr context, int status, IntPtr detail);

    [DllImport(Library, EntryPoint = "stimpak_auth_present", CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr Present(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string url,
        [MarshalAs(UnmanagedType.U1)] bool freshAccount,
        Completion completion,
        IntPtr context);

    [DllImport(Library, EntryPoint = "stimpak_auth_cancel", CallingConvention = CallingConvention.Cdecl)]
    internal static extern void Cancel(IntPtr session);

    [DllImport(Library, EntryPoint = "stimpak_auth_close", CallingConvention = CallingConvention.Cdecl)]
    internal static extern void Close(IntPtr session);

    [DllImport(Library, EntryPoint = "stimpak_auth_abi_version", CallingConvention = CallingConvention.Cdecl)]
    internal static extern uint AbiVersion();

    [DllImport(Library, EntryPoint = "stimpak_auth_version", CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr Version();
}
