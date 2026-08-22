// service and method names recovered by correlating the paired plaintext
// capture with the constructors in libClientSdk.dylib.

pub mod aurora {
    pub const CONNECTION: u32 = 0x6544_6991;
    pub const AUTHENTICATION: u32 = 0x0DEC_FC01;
    pub const GAME_UTILITIES: u32 = 0x3FC1_274D;
    pub const AUTHENTICATION_LISTENER: u32 = 0x7124_0E35;
    pub const CHALLENGE_LISTENER: u32 = 0xBBDA_171F;

    pub const CONNECT: u32 = 1;
    pub const LOGON: u32 = 1;
    pub const VERIFY_WEB_CREDENTIALS: u32 = 7;
    pub const PROCESS_CLIENT_REQUEST: u32 = 1;
    pub const ON_LOGON_COMPLETE: u32 = 5;
    pub const ON_EXTERNAL_CHALLENGE: u32 = 3;
    pub const ON_EXTERNAL_CHALLENGE_RESULT: u32 = 4;
}

pub mod service {
    pub const AUTHENTICATION: u32 = 0x17CD_FF07;
    pub const GATEWAY: u32 = 0x2FD5_9FA3;
    pub const GAME_ACCOUNT: u32 = 0x3542_52A4;
    pub const AURORA_USER: u32 = 0x9BFB_8BFC;
    pub const AURORA_CHAT: u32 = 0x924C_CFDA;
    pub const AURORA_FRIENDS: u32 = 0xAA4E_1E00;
    pub const GAME_VERSION: u32 = 0x3D93_0F0E;
    pub const URL: u32 = 0x6185_46AD;
    pub const LEGACY: u32 = 0xD0C0_F33D;
    pub const LEGACY_CHAT: u32 = 0xF4E1_1A78;
    pub const TOON_PROFILE: u32 = 0x8B95_2A18;
}

pub mod method {
    pub const AUTH_SESSION: u32 = 0x95F5_9163;
    pub const PING: u32 = 0x73F0_69BF;
    pub const GET_GATEWAY_STATS: u32 = 0xA10E_FE1A;
    pub const GET_TOONS: u32 = 0xBC18_EDE5;
    pub const UPDATE_PLAYER_STATUS: u32 = 0xD9CD_4A46;
    pub const SUBSCRIBE_TO_PLAYER_STATUS: u32 = 0xF7AC_4AC4;
    pub const PLAYER_STATUS_UPDATED: u32 = 0x4D8F_E65A;
    pub const FRIEND_UPDATED: u32 = 0xEC7E_2FD1;
    pub const SEND_WHISPER: u32 = 0x6251_CCD8;
    pub const WHISPER_RECEIVED: u32 = 0x7255_E575;
    pub const WHISPER_ECHO_RECEIVED: u32 = 0x82B8_44A8;
    pub const SET_GAME_VERSION: u32 = 0xD48D_E460;
    pub const RESOLVE_AVATAR: u32 = 0x2B63_37ED;
    pub const LEGACY_CONNECT: u32 = 0x6077_16CD;

    pub const CHAT_CONNECT: u32 = 0x78D3_F5A8;
    pub const CHAT_DISCONNECT: u32 = 0x6DEB_8B04;
    pub const SET_ONLINE: u32 = 0xD5EB_A117;
    pub const SET_AWAY: u32 = 0x9B49_45F0;
    pub const SET_DND: u32 = 0xFC78_41EC;
    pub const JOIN_CHANNEL: u32 = 0x00C4_7F9F;
    pub const JOIN_CUSTOM_CHANNEL: u32 = 0xDE7C_D6C0;
    pub const JOIN_CUSTOM_CHANNEL_BY_NAME: u32 = 0x5096_E13A;
    pub const CREATE_AND_JOIN_CUSTOM_CHANNEL: u32 = 0x0E6A_52E5;
    pub const LEAVE_CHANNEL: u32 = 0x84F6_DDA8;
    pub const SEND_MESSAGE: u32 = 0x5851_FB2D;
    pub const SEND_MESSAGE_TO_ALL_FRIENDS: u32 = 0x5ECD_5FA4;
    pub const SEND_COMMAND: u32 = 0x1FEE_1493;
    pub const CHANNELS_UPDATED: u32 = 0xC04D_AC29;
    pub const LEFT_CHANNEL: u32 = 0xB07F_D98A;
    pub const CHAT_WHISPER_MESSAGE: u32 = 0xFA88_D3E1;
    pub const CHAT_TALK_MESSAGE: u32 = 0x850B_6EE3;
    pub const CHAT_BROADCAST_MESSAGE: u32 = 0xAA69_57AA;
    pub const CHAT_INFORMATION_MESSAGE: u32 = 0x1580_B7A1;
    pub const CHAT_ERROR_MESSAGE: u32 = 0xD528_09ED;
    pub const CHAT_EMOTE_MESSAGE: u32 = 0x632D_6CFD;
    pub const FORCE_JOIN_CHANNEL: u32 = 0xC583_300A;
    pub const GET_COMMAND_WHITELIST: u32 = 0x3FDB_078E;
    pub const GET_COMMAND_BLACKLIST: u32 = 0xF4BD_E446;
    pub const COMMAND_WHITELIST_UPDATE: u32 = 0x6CAE_9859;
    pub const COMMAND_BLACKLIST_UPDATE: u32 = 0x78EB_08D1;
    pub const CHAT_FRIEND_ENTER: u32 = 0x57AE_7DBC;
    pub const CHAT_FRIEND_EXIT: u32 = 0x4A61_0CB4;
    pub const CHAT_FRIEND_NOTIFY_GAME: u32 = 0xAA25_AF75;
    pub const GET_AVATAR: u32 = 0x464D_320D;
}

const CLASSIC_SERVICES: &[(u32, &str)] = &[
    (0x17CD_FF07, "Authentication"),
    (0x6781_876B, "Setting"),
    (0x2FD5_9FA3, "Gateway"),
    (0x2B74_0BA5, "Matchmaker"),
    (0x3542_52A4, "GameAccount"),
    (0x4443_2E9B, "Network"),
    (0x9BFB_8BFC, "AuroraUser"),
    (0x3D93_0F0E, "GameVersion"),
    (0x924C_CFDA, "AuroraChat"),
    (0xAA4E_1E00, "AuroraFriends"),
    (0x5A1F_DE49, "FileStore"),
    (0x6185_46AD, "Url"),
    (0x6BC2_01FF, "WebApi"),
    (0xD0C0_F33D, "Legacy"),
    (0xF4E1_1A78, "LegacyChat"),
    (0x8B95_2A18, "ToonProfile"),
    (0x5AEF_361A, "LegacyFriends"),
];

const CLASSIC_METHODS: &[(u32, u32, &str)] = &[
    (0x17CD_FF07, 0x95F5_9163, "AuthSession"),
    (0x17CD_FF07, 0xA9CB_45FC, "SessionUpdate"),
    (0x17CD_FF07, 0x6338_556A, "SessionExpired"),
    (0x17CD_FF07, 0xF2C6_4BA6, "CookieUpdate"),
    (0x17CD_FF07, 0x8254_E3CC, "GetWebToken"),
    (0x17CD_FF07, 0x73F0_69BF, "Ping"),
    (0x6781_876B, 0xCE34_E61F, "PushSetting"),
    (0x6781_876B, 0x799C_F7BD, "ApplySetting"),
    (0x2FD5_9FA3, 0xF557_0066, "GatewayUpdate"),
    (0x2FD5_9FA3, 0xA10E_FE1A, "GetGatewayStats"),
    (0x2B74_0BA5, 0x836B_9689, "GameModeUpdate"),
    (0x3542_52A4, 0xBC18_EDE5, "GetToons"),
    (0x3542_52A4, 0xF75C_AEE5, "ToonUpdated"),
    (0x4443_2E9B, 0x7D47_C51F, "GetEchoServerList"),
    (0x3D93_0F0E, 0xD48D_E460, "SetGameVersion"),
    (0x9BFB_8BFC, 0x4EF4_E009, "UpdateGlobalRichPresence"),
    (0x9BFB_8BFC, 0x7C6A_FCBB, "UpdateLocalRichPresence"),
    (0x9BFB_8BFC, 0xD9CD_4A46, "UpdatePlayerStatus"),
    (0x9BFB_8BFC, 0xF7AC_4AC4, "SubscribeToPlayerStatus"),
    (0x9BFB_8BFC, 0x4D8F_E65A, "PlayerStatusUpdated"),
    (0x924C_CFDA, 0x6251_CCD8, "SendWhisper"),
    (0x924C_CFDA, 0x7255_E575, "WhisperReceived"),
    (0x924C_CFDA, 0x82B8_44A8, "WhisperEchoReceived"),
    (0xAA4E_1E00, 0xEC7E_2FD1, "FriendUpdated"),
    (0x5A1F_DE49, 0x294F_566B, "GetFileList"),
    (0x6185_46AD, 0xC3AE_912A, "ResolveUrl"),
    (0x6185_46AD, 0x2B63_37ED, "ResolveAvatar"),
    (0x6BC2_01FF, 0x3FF8_6DC1, "Get"),
    (0x6BC2_01FF, 0x3D92_5933, "Post"),
    (0xD0C0_F33D, 0x6077_16CD, "Connect"),
    (0xF4E1_1A78, 0x78D3_F5A8, "Connect"),
    (0xF4E1_1A78, 0x6DEB_8B04, "Disconnect"),
    (0xF4E1_1A78, 0x00C4_7F9F, "JoinChannel"),
    (0xF4E1_1A78, 0xDE7C_D6C0, "JoinCustomChannel"),
    (0xF4E1_1A78, 0x5096_E13A, "JoinCustomChannelByName"),
    (0xF4E1_1A78, 0xD5EB_A117, "SetOnline"),
    (0xF4E1_1A78, 0xC04D_AC29, "ChannelsUpdated"),
    (0xF4E1_1A78, 0x0E6A_52E5, "CreateAndJoinCustomChannel"),
    (0xF4E1_1A78, 0x84F6_DDA8, "LeaveChannel"),
    (0xF4E1_1A78, 0x5851_FB2D, "SendMessage"),
    (0xF4E1_1A78, 0x1FEE_1493, "SendCommand"),
    (0xF4E1_1A78, 0x5ECD_5FA4, "SendMessageToAllFriends"),
    (0xF4E1_1A78, 0x9B49_45F0, "SetAway"),
    (0xF4E1_1A78, 0xFC78_41EC, "SetDND"),
    (0xF4E1_1A78, 0xB07F_D98A, "LeftChannel"),
    (0xF4E1_1A78, 0xFA88_D3E1, "ChatWhisperMessage"),
    (0xF4E1_1A78, 0x850B_6EE3, "ChatTalkMessage"),
    (0xF4E1_1A78, 0xAA69_57AA, "ChatBroadcastMessage"),
    (0xF4E1_1A78, 0x1580_B7A1, "ChatInformationMessage"),
    (0xF4E1_1A78, 0xD528_09ED, "ChatErrorMessage"),
    (0xF4E1_1A78, 0x632D_6CFD, "ChatEmoteMessage"),
    (0xF4E1_1A78, 0xC583_300A, "ForceJoinChannel"),
    (0xF4E1_1A78, 0x3FDB_078E, "GetCommandWhitelist"),
    (0xF4E1_1A78, 0xF4BD_E446, "GetCommandBlacklist"),
    (0xF4E1_1A78, 0x6CAE_9859, "CommandWhitelistUpdate"),
    (0xF4E1_1A78, 0x78EB_08D1, "CommandBlacklistUpdate"),
    (0xF4E1_1A78, 0x57AE_7DBC, "ChatFriendEnter"),
    (0xF4E1_1A78, 0x4A61_0CB4, "ChatFriendExit"),
    (0xF4E1_1A78, 0xAA25_AF75, "ChatFriendNotifyGame"),
    (0x8B95_2A18, 0x464D_320D, "GetAvatar"),
    (0x5AEF_361A, 0xDADD_B5B7, "Connected"),
];

#[must_use]
pub fn service_name(service_id: u32) -> Option<&'static str> {
    CLASSIC_SERVICES
        .iter()
        .find_map(|(candidate, name)| (*candidate == service_id).then_some(*name))
}

#[must_use]
pub fn method_name(service_id: u32, method_id: u32) -> Option<&'static str> {
    CLASSIC_METHODS.iter().find_map(|(service, method, name)| {
        (*service == service_id && *method == method_id).then_some(*name)
    })
}

#[must_use]
pub fn rpc_name(service_id: u32, method_id: u32) -> Option<String> {
    Some(format!(
        "{}.{}",
        service_name(service_id)?,
        method_name(service_id, method_id)?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_agree_with_the_name_tables() {
        assert_eq!(
            rpc_name(service::AUTHENTICATION, method::AUTH_SESSION).as_deref(),
            Some("Authentication.AuthSession")
        );
        assert_eq!(
            rpc_name(service::AUTHENTICATION, method::PING).as_deref(),
            Some("Authentication.Ping")
        );
        assert_eq!(
            rpc_name(service::LEGACY_CHAT, method::CHANNELS_UPDATED).as_deref(),
            Some("LegacyChat.ChannelsUpdated")
        );
        assert_eq!(
            rpc_name(service::GAME_ACCOUNT, method::GET_TOONS).as_deref(),
            Some("GameAccount.GetToons")
        );
        assert_eq!(service_name(service::LEGACY), Some("Legacy"));
        assert_eq!(
            rpc_name(service::AURORA_USER, method::PLAYER_STATUS_UPDATED).as_deref(),
            Some("AuroraUser.PlayerStatusUpdated")
        );
        assert_eq!(
            rpc_name(service::AURORA_FRIENDS, method::FRIEND_UPDATED).as_deref(),
            Some("AuroraFriends.FriendUpdated")
        );
        assert_eq!(
            rpc_name(service::AURORA_CHAT, method::SEND_WHISPER).as_deref(),
            Some("AuroraChat.SendWhisper")
        );
        assert_eq!(
            rpc_name(service::AURORA_CHAT, method::WHISPER_RECEIVED).as_deref(),
            Some("AuroraChat.WhisperReceived")
        );
        assert_eq!(
            rpc_name(service::AURORA_CHAT, method::WHISPER_ECHO_RECEIVED).as_deref(),
            Some("AuroraChat.WhisperEchoReceived")
        );
        assert_eq!(
            rpc_name(service::TOON_PROFILE, method::GET_AVATAR).as_deref(),
            Some("ToonProfile.GetAvatar")
        );
        assert_eq!(
            rpc_name(service::URL, method::RESOLVE_AVATAR).as_deref(),
            Some("Url.ResolveAvatar")
        );
    }

    #[test]
    fn unknown_identifiers_stay_unnamed() {
        assert_eq!(rpc_name(0x1234_5678, 1), None);
        assert_eq!(method_name(service::LEGACY_CHAT, 0xDEAD_BEEF), None);
    }
}
