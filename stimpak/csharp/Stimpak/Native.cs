using System.Runtime.InteropServices;

namespace Stimpak;

/// <summary>raw c entry points; <see cref="StimpakClient"/> is the surface to use.</summary>
internal static partial class Native
{
    internal const string Library = "stimpak";

    internal const int Ok = 0;
    internal const int ErrInvalidArgument = -1;
    internal const int ErrDisconnected = -2;
    internal const int ErrNoSuchAuth = -3;
    internal const int ErrAuthFailed = -4;
    internal const int ErrPanic = -99;

    [LibraryImport(Library, EntryPoint = "stimpak_client_open", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr Open(string credentialPath, string? authWindowPath);

    [LibraryImport(Library, EntryPoint = "stimpak_client_has_auth_window")]
    [return: MarshalAs(UnmanagedType.U1)]
    internal static partial bool HasAuthWindow(IntPtr client);

    [LibraryImport(Library, EntryPoint = "stimpak_client_close")]
    internal static partial void Close(IntPtr client);

    [LibraryImport(Library, EntryPoint = "stimpak_client_connect")]
    [return: MarshalAs(UnmanagedType.I4)]
    internal static partial int Connect(IntPtr client, [MarshalAs(UnmanagedType.U1)] bool forceInteractive);

    [LibraryImport(Library, EntryPoint = "stimpak_client_connect_configured", StringMarshalling = StringMarshalling.Utf8)]
    [return: MarshalAs(UnmanagedType.I4)]
    internal static partial int ConnectConfigured(
        IntPtr client,
        [MarshalAs(UnmanagedType.U1)] bool forceInteractive,
        ulong expectedAccountId,
        string channelsJson);

    [LibraryImport(Library, EntryPoint = "stimpak_client_disconnect")]
    internal static partial int Disconnect(IntPtr client);

    [LibraryImport(Library, EntryPoint = "stimpak_client_sign_out")]
    internal static partial int SignOut(IntPtr client);

    [LibraryImport(Library, EntryPoint = "stimpak_client_join_public")]
    internal static partial int JoinPublic(IntPtr client, ushort channelId);

    [LibraryImport(Library, EntryPoint = "stimpak_client_join_private", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int JoinPrivate(IntPtr client, string name);

    [LibraryImport(Library, EntryPoint = "stimpak_client_join_group")]
    internal static partial int JoinGroup(IntPtr client, uint clubId);

    [LibraryImport(Library, EntryPoint = "stimpak_client_search_groups", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int SearchGroups(IntPtr client, string query);

    [LibraryImport(Library, EntryPoint = "stimpak_client_leave")]
    internal static partial int Leave(IntPtr client, byte channelIndex);

    [LibraryImport(Library, EntryPoint = "stimpak_client_send_message", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int SendMessage(IntPtr client, byte channelIndex, string body);

    [LibraryImport(Library, EntryPoint = "stimpak_client_send_whisper", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int SendWhisper(IntPtr client, string name, string body);

    [LibraryImport(Library, EntryPoint = "stimpak_client_answer_group_invitation")]
    internal static partial int AnswerGroupInvitation(
        IntPtr client,
        uint clubId,
        [MarshalAs(UnmanagedType.U1)] bool accept);

    [LibraryImport(Library, EntryPoint = "stimpak_client_answer_party_invitation")]
    internal static partial int AnswerPartyInvitation(
        IntPtr client,
        byte channelIndex,
        [MarshalAs(UnmanagedType.U1)] bool accept);

    [LibraryImport(Library, EntryPoint = "stimpak_client_submit_auth", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int SubmitAuth(IntPtr client, ulong authId, string token);

    [LibraryImport(Library, EntryPoint = "stimpak_client_cancel_auth")]
    internal static partial int CancelAuth(IntPtr client, ulong authId);

    /// <summary>owned utf-8 string, or null when nothing arrived. free with <see cref="FreeString"/>.</summary>
    [LibraryImport(Library, EntryPoint = "stimpak_client_poll")]
    internal static partial IntPtr Poll(IntPtr client, uint timeoutMs);

    [LibraryImport(Library, EntryPoint = "stimpak_string_free")]
    internal static partial void FreeString(IntPtr value);

    [LibraryImport(Library, EntryPoint = "stimpak_default_public_channel")]
    internal static partial ushort DefaultPublicChannel();

    [LibraryImport(Library, EntryPoint = "stimpak_abi_version")]
    internal static partial uint AbiVersion();

    [LibraryImport(Library, EntryPoint = "stimpak_event_schema_version")]
    internal static partial uint EventSchemaVersion();

    [LibraryImport(Library, EntryPoint = "stimpak_version")]
    internal static partial IntPtr Version();
}
