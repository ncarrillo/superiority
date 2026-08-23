use std::{
    io::{self, Read, Write},
    time::{Duration, Instant},
};

pub const SID_NULL: u8 = 0x00;
pub const SID_CLIENT_ID: u8 = 0x05;
pub const SID_START_VERSIONING: u8 = 0x06;
pub const SID_REPORT_VERSION: u8 = 0x07;
pub const SID_ENTER_CHAT: u8 = 0x0A;
pub const SID_GET_CHANNEL_LIST: u8 = 0x0B;
pub const SID_JOIN_CHANNEL: u8 = 0x0C;
pub const SID_CHAT_COMMAND: u8 = 0x0E;
pub const SID_CHAT_EVENT: u8 = 0x0F;
pub const SID_LEAVE_CHAT: u8 = 0x10;
pub const SID_LOCALE_INFO: u8 = 0x12;
pub const SID_UDP_PING_RESPONSE: u8 = 0x14;
pub const SID_LOGON_CHALLENGE_EX: u8 = 0x1D;
pub const SID_PING: u8 = 0x25;
pub const SID_READ_USER_DATA: u8 = 0x26;
pub const SID_LOGON_RESPONSE: u8 = 0x29;
pub const SID_CREATE_ACCOUNT: u8 = 0x2A;
pub const SID_GET_ICON_DATA: u8 = 0x2D;
pub const SID_GET_FILE_TIME: u8 = 0x33;
pub const SID_CD_KEY2: u8 = 0x36;
pub const SID_LOGON_RESPONSE2: u8 = 0x3A;
pub const SID_CREATE_ACCOUNT2: u8 = 0x3D;
pub const SID_QUERY_REALMS2: u8 = 0x40;
pub const SID_AUTH_INFO: u8 = 0x50;
pub const SID_AUTH_CHECK: u8 = 0x51;
pub const SID_AUTH_ACCOUNT_CREATE: u8 = 0x52;
pub const SID_AUTH_ACCOUNT_LOGON: u8 = 0x53;
pub const SID_AUTH_ACCOUNT_LOGON_PROOF: u8 = 0x54;
pub const SID_FRIENDS_LIST: u8 = 0x65;
pub const SID_FRIENDS_UPDATE: u8 = 0x66;
pub const SID_FRIENDS_ADD: u8 = 0x67;
pub const SID_FRIENDS_REMOVE: u8 = 0x68;
pub const SID_FRIENDS_POSITION: u8 = 0x69;

pub const EID_SHOW_USER: u32 = 0x01;
pub const EID_WHISPER: u32 = 0x04;
pub const EID_TALK: u32 = 0x05;
#[expect(
    dead_code,
    reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
)]
pub const EID_JOIN: u32 = 0x02;
#[expect(
    dead_code,
    reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
)]
pub const EID_LEAVE: u32 = 0x03;
pub const EID_CHANNEL: u32 = 0x07;
pub const EID_WHISPER_SENT: u32 = 0x0A;
pub const EID_CHANNEL_DOES_NOT_EXIST: u32 = 0x0E;
pub const EID_INFO: u32 = 0x12;
pub const EID_ERROR: u32 = 0x13;
#[cfg(test)]
const FLAG_OPERATOR: u32 = 0x02;
pub const GAME_PROTOCOL: u8 = 0x01;
pub const FRIEND_STATUS_MUTUAL: u8 = 0x01;
pub const FRIEND_STATUS_DND: u8 = 0x02;
pub const FRIEND_STATUS_AWAY: u8 = 0x04;
pub const FRIEND_LOCATION_OFFLINE: u8 = 0x00;
pub const FRIEND_LOCATION_NOT_IN_CHAT: u8 = 0x01;
pub const FRIEND_LOCATION_PUBLIC_GAME: u8 = 0x03;
pub const PRODUCT_STAR: u32 = u32::from_be_bytes(*b"STAR");

const HEADER_BYTES: usize = 4;
const MAX_PACKET_BYTES: usize = u16::MAX as usize;
const SYNTHETIC_ACCOUNT_NUMBER: u32 = 0xBAAD_F00D;
const NULL_INTERVAL: Duration = Duration::from_secs(60);
const PING_INTERVAL: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub id: u8,
    pub payload: Vec<u8>,
}

impl Packet {
    #[must_use]
    pub fn new(id: u8, payload: Vec<u8>) -> Self {
        Self { id, payload }
    }

    /// reads one bncs packet; a clean eof before the packet marker is `none`.
    pub fn read_from(reader: &mut impl Read) -> io::Result<Option<Self>> {
        let mut marker = [0u8; 1];
        loop {
            match reader.read(&mut marker) {
                Ok(0) => return Ok(None),
                Ok(1) => break,
                Ok(_) => unreachable!("a one-byte buffer cannot receive more than one byte"),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        if marker[0] != 0xFF {
            return Err(invalid_data(format!(
                "invalid BNCS packet marker 0x{:02X}",
                marker[0]
            )));
        }

        let mut rest = [0u8; 3];
        reader.read_exact(&mut rest)?;
        let id = rest[0];
        let length = usize::from(u16::from_le_bytes([rest[1], rest[2]]));
        if !(HEADER_BYTES..=MAX_PACKET_BYTES).contains(&length) {
            return Err(invalid_data(format!("invalid BNCS packet length {length}")));
        }
        let mut payload = vec![0; length - HEADER_BYTES];
        reader.read_exact(&mut payload)?;
        Ok(Some(Self { id, payload }))
    }

    pub fn write_to(&self, writer: &mut impl Write) -> io::Result<()> {
        let length = self
            .payload
            .len()
            .checked_add(HEADER_BYTES)
            .and_then(|value| u16::try_from(value).ok())
            .ok_or_else(|| invalid_data("BNCS packet exceeds 65535 bytes"))?;
        writer.write_all(&[0xFF, self.id])?;
        writer.write_all(&length.to_le_bytes())?;
        writer.write_all(&self.payload)?;
        Ok(())
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self.id {
            SID_NULL => "sid_null",
            SID_CLIENT_ID => "sid_clientid",
            SID_START_VERSIONING => "sid_startversioning",
            SID_REPORT_VERSION => "sid_reportversion",
            SID_ENTER_CHAT => "sid_enterchat",
            SID_GET_CHANNEL_LIST => "sid_getchannellist",
            SID_JOIN_CHANNEL => "sid_joinchannel",
            SID_CHAT_COMMAND => "sid_chatcommand",
            SID_CHAT_EVENT => "sid_chatevent",
            SID_LEAVE_CHAT => "sid_leavechat",
            SID_LOCALE_INFO => "sid_localeinfo",
            SID_UDP_PING_RESPONSE => "sid_udppingresponse",
            SID_LOGON_CHALLENGE_EX => "sid_logonchallengeex",
            SID_PING => "sid_ping",
            SID_READ_USER_DATA => "sid_readuserdata",
            SID_LOGON_RESPONSE => "sid_logonresponse",
            SID_CREATE_ACCOUNT => "sid_createaccount",
            SID_GET_ICON_DATA => "sid_geticondata",
            SID_GET_FILE_TIME => "sid_getfiletime",
            SID_CD_KEY2 => "sid_cdkey2",
            SID_LOGON_RESPONSE2 => "sid_logonresponse2",
            SID_CREATE_ACCOUNT2 => "sid_createaccount2",
            SID_QUERY_REALMS2 => "sid_queryrealms2",
            SID_AUTH_INFO => "sid_auth_info",
            SID_AUTH_CHECK => "sid_auth_check",
            SID_AUTH_ACCOUNT_CREATE => "sid_auth_accountcreate",
            SID_AUTH_ACCOUNT_LOGON => "sid_auth_accountlogon",
            SID_AUTH_ACCOUNT_LOGON_PROOF => "sid_auth_accountlogonproof",
            SID_FRIENDS_LIST => "sid_friendslist",
            SID_FRIENDS_UPDATE => "sid_friendsupdate",
            SID_FRIENDS_ADD => "sid_friendsadd",
            SID_FRIENDS_REMOVE => "sid_friendsremove",
            SID_FRIENDS_POSITION => "sid_friendsposition",
            _ => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    EnterChat { requested_name: String },
    RequestChannelList,
    JoinChannel { flags: u32, channel: String },
    LeaveChat,
    ChatCommand(String),
    RequestFriendsList,
    Pong { round_trip: Option<Duration> },
    UdpCapability { supported: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendEntry {
    pub name: String,
    pub status: u8,
    pub location: u8,
    pub product: u32,
    pub location_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogonStyle {
    Legacy,
    Old,
    New,
}

pub struct LegacyPeer<W> {
    writer: W,
    expected_account: String,
    account_name: Option<String>,
    logon_style: Option<LogonStyle>,
    account_accepted: bool,
    logged_on: bool,
    keepalives_active: bool,
    next_null: Instant,
    next_ping: Instant,
    pending_ping: Option<(u32, Instant)>,
}

impl<W: Write> LegacyPeer<W> {
    pub fn new(writer: W, expected_account: impl Into<String>) -> Self {
        let now = Instant::now();
        Self {
            writer,
            expected_account: expected_account.into(),
            account_name: None,
            logon_style: None,
            account_accepted: false,
            logged_on: false,
            keepalives_active: false,
            next_null: now + NULL_INTERVAL,
            next_ping: now + PING_INTERVAL,
            pending_ping: None,
        }
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    pub fn handle(&mut self, packet: &Packet) -> io::Result<Vec<Action>> {
        match packet.id {
            SID_NULL => require_length(packet, 0)?,
            SID_AUTH_INFO => self.handle_auth_info()?,
            SID_AUTH_CHECK => self.send_auth_check_success()?,
            SID_AUTH_ACCOUNT_LOGON => self.handle_new_logon(&packet.payload)?,
            SID_AUTH_ACCOUNT_LOGON_PROOF => {
                self.logon_style = Some(LogonStyle::New);
                if self.account_accepted {
                    self.send_logon_success()?;
                } else {
                    self.send_new_logon_proof_failure()?;
                }
            }
            SID_LOGON_RESPONSE2 => self.handle_old_logon(&packet.payload, LogonStyle::Old)?,
            SID_LOGON_RESPONSE => self.handle_old_logon(&packet.payload, LogonStyle::Legacy)?,
            SID_CREATE_ACCOUNT => self.send_legacy_creation_rejected()?,
            SID_CREATE_ACCOUNT2 => self.send_old_creation_rejected()?,
            SID_AUTH_ACCOUNT_CREATE => self.send_new_creation_rejected()?,
            SID_START_VERSIONING => self.handle_start_versioning()?,
            SID_REPORT_VERSION => self.send_report_version_success()?,
            SID_CD_KEY2 => self.send_cd_key_success()?,
            SID_QUERY_REALMS2 => self.send_query_realms_response()?,
            SID_GET_FILE_TIME => self.send_file_time_response(&packet.payload)?,
            SID_GET_ICON_DATA => self.send_icon_response()?,
            SID_ENTER_CHAT => {
                if self.logged_on {
                    validate_enter_chat(packet)?;
                    return Ok(vec![Action::EnterChat {
                        requested_name: self.expected_account.clone(),
                    }]);
                }
            }
            SID_GET_CHANNEL_LIST => {
                require_length(packet, 4)?;
                return Ok(vec![Action::RequestChannelList]);
            }
            SID_JOIN_CHANNEL => {
                if self.logged_on {
                    let mut payload = PayloadReader::new(&packet.payload);
                    let flags = payload
                        .dword()
                        .ok_or_else(|| invalid_data("sid_joinchannel is missing its flags"))?;
                    let channel = payload
                        .c_string()
                        .ok_or_else(|| invalid_data("sid_joinchannel has no channel terminator"))?;
                    require_consumed(&payload, "sid_joinchannel")?;
                    return Ok(vec![Action::JoinChannel { flags, channel }]);
                }
            }
            SID_LEAVE_CHAT if self.logged_on => {
                require_length(packet, 0)?;
                return Ok(vec![Action::LeaveChat]);
            }
            SID_CHAT_COMMAND => {
                if let Some(command) = validate_chat_command(packet)? {
                    return Ok(vec![Action::ChatCommand(command)]);
                }
            }
            SID_FRIENDS_LIST if self.logged_on => {
                require_length(packet, 0)?;
                return Ok(vec![Action::RequestFriendsList]);
            }
            SID_PING => return self.handle_ping_reply(packet),
            SID_UDP_PING_RESPONSE => {
                require_length(packet, 4)?;
                let token = u32::from_le_bytes(packet.payload[..4].try_into().expect("four bytes"));
                return Ok(vec![Action::UdpCapability {
                    supported: token == u32::from_be_bytes(*b"bnet"),
                }]);
            }
            SID_LOCALE_INFO => {
                if packet.payload.len() < 36 {
                    return Err(invalid_data("sid_localeinfo must be at least 36 bytes"));
                }
            }
            SID_READ_USER_DATA if self.logged_on => self.handle_read_user_data(packet)?,
            _ => {}
        }
        Ok(Vec::new())
    }

    pub fn service_keepalive(&mut self, now: Instant) -> io::Result<bool> {
        if !self.keepalives_active {
            return Ok(false);
        }
        let mut sent = false;
        if now >= self.next_null {
            self.send(SID_NULL, Vec::new())?;
            self.next_null = now + NULL_INTERVAL;
            sent = true;
        }
        if now >= self.next_ping {
            self.send_ping(now)?;
            sent = true;
        }
        Ok(sent)
    }

    pub fn send_enter_chat(&mut self, username: &str, stats: &str) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        payload.c_string(username);
        payload.c_string(stats);
        payload.c_string(username);
        self.send(SID_ENTER_CHAT, payload.finish())
    }

    pub fn send_chat_event(
        &mut self,
        event_id: u32,
        username: &str,
        text: &str,
        flags: u32,
        ping: u32,
    ) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        payload.dword(event_id);
        payload.dword(flags);
        payload.dword(ping);
        payload.dword(0); // ip address is intentionally not disclosed.
        payload.dword(SYNTHETIC_ACCOUNT_NUMBER);
        payload.dword(SYNTHETIC_ACCOUNT_NUMBER);
        payload.c_string(username);
        payload.c_string(text);
        self.send(SID_CHAT_EVENT, payload.finish())
    }

    pub fn send_error(&mut self, message: &str) -> io::Result<()> {
        self.send_chat_event(EID_ERROR, "SC2 Gateway", message, 0, 0)
    }

    pub fn send_friends_list(&mut self, friends: &[FriendEntry]) -> io::Result<()> {
        let count = u8::try_from(friends.len())
            .map_err(|_| invalid_data("BNCS friend lists cannot exceed 255 entries"))?;
        let mut payload = PayloadWriter::new();
        payload.byte(count);
        for friend in friends {
            write_friend(&mut payload, friend, true);
        }
        self.send(SID_FRIENDS_LIST, payload.finish())
    }

    pub fn send_channel_list(&mut self, channels: &[String]) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        for channel in channels {
            payload.c_string(channel);
        }
        payload.byte(0);
        self.send(SID_GET_CHANNEL_LIST, payload.finish())
    }

    fn handle_auth_info(&mut self) -> io::Result<()> {
        self.begin_keepalives(Instant::now())?;
        self.send_auth_check_success()
    }

    fn begin_keepalives(&mut self, now: Instant) -> io::Result<()> {
        if self.keepalives_active {
            return Ok(());
        }
        self.keepalives_active = true;
        self.next_null = now + NULL_INTERVAL;
        self.send_ping(now)
    }

    fn send_ping(&mut self, now: Instant) -> io::Result<()> {
        let token = rand::random();
        let mut payload = PayloadWriter::new();
        payload.dword(token);
        self.send(SID_PING, payload.finish())?;
        self.pending_ping = Some((token, now));
        self.next_ping = now + PING_INTERVAL;
        Ok(())
    }

    fn handle_ping_reply(&mut self, packet: &Packet) -> io::Result<Vec<Action>> {
        require_length(packet, 4)?;
        let token = u32::from_le_bytes(packet.payload[..4].try_into().expect("four bytes"));
        let round_trip = self
            .pending_ping
            .take()
            .filter(|(expected, _)| *expected == token)
            .map(|(_, sent_at)| sent_at.elapsed());
        Ok(vec![Action::Pong { round_trip }])
    }

    fn handle_read_user_data(&mut self, packet: &Packet) -> io::Result<()> {
        let mut reader = PayloadReader::new(&packet.payload);
        let account_count = reader
            .dword()
            .ok_or_else(|| invalid_data("sid_readuserdata is missing the account count"))?;
        let key_count = reader
            .dword()
            .ok_or_else(|| invalid_data("sid_readuserdata is missing the key count"))?;
        let request_id = reader
            .dword()
            .ok_or_else(|| invalid_data("sid_readuserdata is missing the request id"))?;
        if key_count > 31 {
            return Err(invalid_data(
                "sid_readuserdata cannot request more than 31 keys",
            ));
        }
        let string_count = account_count
            .checked_add(key_count)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or_else(|| invalid_data("sid_readuserdata string count is too large"))?;
        if string_count > reader.remaining() {
            return Err(invalid_data(
                "sid_readuserdata contains impossible string counts",
            ));
        }
        for _ in 0..string_count {
            reader
                .c_string()
                .ok_or_else(|| invalid_data("sid_readuserdata has an unterminated string"))?;
        }
        require_consumed(&reader, "sid_readuserdata")?;

        let (account_count, key_count) = if account_count > 1 {
            (0, 0)
        } else {
            (account_count, key_count)
        };
        let value_count = account_count
            .checked_mul(key_count)
            .ok_or_else(|| invalid_data("sid_readuserdata value count is too large"))?;
        let mut response = PayloadWriter::new();
        response.dword(account_count);
        response.dword(key_count);
        response.dword(request_id);
        for _ in 0..value_count {
            response.byte(0);
        }
        self.send(SID_READ_USER_DATA, response.finish())
    }

    fn send_auth_check_success(&mut self) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        payload.dword(0);
        payload.c_string("");
        self.send(SID_AUTH_CHECK, payload.finish())
    }

    fn handle_new_logon(&mut self, payload: &[u8]) -> io::Result<()> {
        self.logon_style = Some(LogonStyle::New);
        self.account_name = c_string_at(payload, 32);
        self.account_accepted = self.account_matches();
        self.logged_on = false;

        let mut response = PayloadWriter::new();
        response.dword(u32::from(!self.account_accepted));
        response.bytes(&[0; 32]);
        response.bytes(&[0; 32]);
        self.send(SID_AUTH_ACCOUNT_LOGON, response.finish())
    }

    fn handle_old_logon(&mut self, payload: &[u8], style: LogonStyle) -> io::Result<()> {
        self.logon_style = Some(style);
        self.account_name = c_string_at(payload, 28);
        self.account_accepted = self.account_matches();
        self.logged_on = false;
        if self.account_accepted {
            self.send_logon_success()
        } else {
            self.send_old_logon_failure(style)
        }
    }

    fn send_logon_success(&mut self) -> io::Result<()> {
        if !self.account_accepted {
            return Err(invalid_data(
                "cannot accept a BNCS account that is not the selected SC2 toon",
            ));
        }
        self.logged_on = true;
        let mut payload = PayloadWriter::new();
        match self.logon_style.unwrap_or(LogonStyle::Old) {
            LogonStyle::Legacy => {
                payload.dword(1);
                self.send(SID_LOGON_RESPONSE, payload.finish())
            }
            LogonStyle::Old => {
                payload.dword(0);
                self.send(SID_LOGON_RESPONSE2, payload.finish())
            }
            LogonStyle::New => {
                payload.dword(0);
                payload.bytes(&[0; 20]);
                payload.c_string("");
                self.send(SID_AUTH_ACCOUNT_LOGON_PROOF, payload.finish())
            }
        }
    }

    fn account_matches(&self) -> bool {
        self.account_name
            .as_deref()
            .is_some_and(|account| account.eq_ignore_ascii_case(&self.expected_account))
    }

    fn send_old_logon_failure(&mut self, style: LogonStyle) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        match style {
            LogonStyle::Legacy => {
                payload.dword(0);
                self.send(SID_LOGON_RESPONSE, payload.finish())
            }
            LogonStyle::Old => {
                // sid_logonresponse2 status 1 means the account does not exist.
                payload.dword(1);
                self.send(SID_LOGON_RESPONSE2, payload.finish())
            }
            LogonStyle::New => unreachable!("new logon has a separate challenge response"),
        }
    }

    fn send_new_logon_proof_failure(&mut self) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        // this is reached only if a client ignores the preceding unknown-account
        // response and sends a proof anyway.
        payload.dword(2); // sid_auth_accountlogonproof: incorrect password
        payload.bytes(&[0; 20]);
        payload.c_string("");
        self.send(SID_AUTH_ACCOUNT_LOGON_PROOF, payload.finish())
    }

    fn send_legacy_creation_rejected(&mut self) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        payload.dword(0); // sid_createaccount: failure
        self.send(SID_CREATE_ACCOUNT, payload.finish())
    }

    fn send_old_creation_rejected(&mut self) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        payload.dword(4); // sid_createaccount2: account already exists
        payload.c_string("");
        self.send(SID_CREATE_ACCOUNT2, payload.finish())
    }

    fn send_new_creation_rejected(&mut self) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        payload.dword(4); // sid_auth_accountcreate: account already exists
        self.send(SID_AUTH_ACCOUNT_CREATE, payload.finish())
    }

    fn handle_start_versioning(&mut self) -> io::Result<()> {
        let mut client_id = PayloadWriter::new();
        for _ in 0..4 {
            client_id.dword(0);
        }
        self.send(SID_CLIENT_ID, client_id.finish())?;

        let mut challenge = PayloadWriter::new();
        challenge.dword(0);
        challenge.dword(rand::random());
        self.send(SID_LOGON_CHALLENGE_EX, challenge.finish())?;
        self.send_report_version_success()?;
        self.begin_keepalives(Instant::now())
    }

    fn send_report_version_success(&mut self) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        payload.dword(2);
        payload.c_string("");
        self.send(SID_REPORT_VERSION, payload.finish())
    }

    fn send_cd_key_success(&mut self) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        payload.dword(1);
        payload.c_string("");
        self.send(SID_CD_KEY2, payload.finish())
    }

    fn send_query_realms_response(&mut self) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        payload.dword(0);
        payload.dword(0);
        self.send(SID_QUERY_REALMS2, payload.finish())
    }

    fn send_file_time_response(&mut self, request: &[u8]) -> io::Result<()> {
        let mut reader = PayloadReader::new(request);
        let request_id = reader.dword().unwrap_or(0);
        let unknown = reader.dword().unwrap_or(0);
        let filename = reader.c_string().unwrap_or_default();

        let mut payload = PayloadWriter::new();
        payload.dword(request_id);
        payload.dword(unknown);
        payload.qword(0);
        payload.c_string(&filename);
        self.send(SID_GET_FILE_TIME, payload.finish())
    }

    fn send_icon_response(&mut self) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        payload.qword(0);
        payload.c_string("icons.bni");
        self.send(SID_GET_ICON_DATA, payload.finish())
    }

    fn send(&mut self, id: u8, payload: Vec<u8>) -> io::Result<()> {
        Packet::new(id, payload).write_to(&mut self.writer)
    }

    #[expect(
        dead_code,
        reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
    )]
    pub fn send_friend_update(&mut self, index: u8, friend: &FriendEntry) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        payload.byte(index);
        write_friend(&mut payload, friend, false);
        self.send(SID_FRIENDS_UPDATE, payload.finish())
    }

    #[expect(
        dead_code,
        reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
    )]
    pub fn send_friend_add(&mut self, friend: &FriendEntry) -> io::Result<()> {
        let mut payload = PayloadWriter::new();
        write_friend(&mut payload, friend, true);
        self.send(SID_FRIENDS_ADD, payload.finish())
    }

    #[expect(
        dead_code,
        reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
    )]
    pub fn send_friend_remove(&mut self, index: u8) -> io::Result<()> {
        self.send(SID_FRIENDS_REMOVE, vec![index])
    }

    #[expect(
        dead_code,
        reason = "friend sync for legacy clients is written and tested but not yet driven from the relay loop"
    )]
    pub fn send_friend_position(&mut self, old: u8, new: u8) -> io::Result<()> {
        self.send(SID_FRIENDS_POSITION, vec![old, new])
    }
}

struct PayloadReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> PayloadReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn dword(&mut self) -> Option<u32> {
        let bytes: [u8; 4] = self
            .data
            .get(self.offset..self.offset + 4)?
            .try_into()
            .ok()?;
        self.offset += 4;
        Some(u32::from_le_bytes(bytes))
    }

    #[cfg(test)]
    fn byte(&mut self) -> Option<u8> {
        let value = *self.data.get(self.offset)?;
        self.offset += 1;
        Some(value)
    }

    fn c_string(&mut self) -> Option<String> {
        let tail = self.data.get(self.offset..)?;
        let length = tail.iter().position(|byte| *byte == 0)?;
        self.offset += length + 1;
        Some(String::from_utf8_lossy(&tail[..length]).into_owned())
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }
}

struct PayloadWriter {
    data: Vec<u8>,
}

impl PayloadWriter {
    fn new() -> Self {
        Self { data: Vec::new() }
    }

    fn dword(&mut self, value: u32) {
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    fn byte(&mut self, value: u8) {
        self.data.push(value);
    }

    fn qword(&mut self, value: u64) {
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.data.extend_from_slice(value);
    }

    fn c_string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
        self.data.push(0);
    }

    fn finish(self) -> Vec<u8> {
        self.data
    }
}

fn write_friend(payload: &mut PayloadWriter, friend: &FriendEntry, include_name: bool) {
    if include_name {
        payload.c_string(&friend.name);
    }
    payload.byte(friend.status);
    payload.byte(friend.location);
    payload.dword(friend.product);
    payload.c_string(&friend.location_name);
}

fn require_length(packet: &Packet, expected: usize) -> io::Result<()> {
    if packet.payload.len() == expected {
        Ok(())
    } else {
        Err(invalid_data(format!(
            "{} must contain exactly {expected} payload bytes, received {}",
            packet.name(),
            packet.payload.len()
        )))
    }
}

fn require_consumed(reader: &PayloadReader<'_>, packet_name: &str) -> io::Result<()> {
    if reader.remaining() == 0 {
        Ok(())
    } else {
        Err(invalid_data(format!(
            "{packet_name} contains {} trailing bytes",
            reader.remaining()
        )))
    }
}

fn validate_enter_chat(packet: &Packet) -> io::Result<()> {
    let mut reader = PayloadReader::new(&packet.payload);
    reader
        .c_string()
        .ok_or_else(|| invalid_data("sid_enterchat has no username terminator"))?;
    let statstring = reader
        .c_string()
        .ok_or_else(|| invalid_data("sid_enterchat has no statstring terminator"))?;
    if statstring.len() > 128 {
        return Err(invalid_data("sid_enterchat statstring exceeds 128 bytes"));
    }
    require_consumed(&reader, "sid_enterchat")
}

fn validate_chat_command(packet: &Packet) -> io::Result<Option<String>> {
    if !(2..=224).contains(&packet.payload.len()) {
        return Err(invalid_data(
            "sid_chatcommand must contain between 2 and 224 payload bytes",
        ));
    }
    let Some((&terminator, body)) = packet.payload.split_last() else {
        return Err(invalid_data("sid_chatcommand is empty"));
    };
    if terminator != 0 {
        return Err(invalid_data("sid_chatcommand is not null terminated"));
    }
    if body.iter().any(|byte| *byte < 32) {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(body).into_owned()))
}

fn c_string_at(data: &[u8], offset: usize) -> Option<String> {
    let tail = data.get(offset..)?;
    let length = tail.iter().position(|byte| *byte == 0)?;
    Some(String::from_utf8_lossy(&tail[..length]).into_owned())
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn packets(data: Vec<u8>) -> Vec<Packet> {
        let mut input = Cursor::new(data);
        let mut output = Vec::new();
        while let Some(packet) = Packet::read_from(&mut input).expect("valid packet") {
            output.push(packet);
        }
        output
    }

    #[test]
    fn packet_round_trips() {
        let original = Packet::new(SID_CHAT_COMMAND, b"hello\0".to_vec());
        let mut data = Vec::new();
        original.write_to(&mut data).expect("write");
        assert_eq!(
            Packet::read_from(&mut data.as_slice()).expect("read"),
            Some(original)
        );
    }

    #[test]
    fn packet_rejects_bad_marker_and_short_length() {
        assert!(Packet::read_from(&mut &[0, 1, 4, 0][..]).is_err());
        assert!(Packet::read_from(&mut &[0xFF, 1, 3, 0][..]).is_err());
    }

    #[test]
    fn modern_logon_is_synthetic_but_sequenced() {
        let mut peer = LegacyPeer::new(Vec::new(), "ExampleAccount");
        peer.handle(&Packet::new(SID_AUTH_INFO, Vec::new()))
            .expect("auth info");

        let mut logon = vec![0; 32];
        logon.extend_from_slice(b"ExampleAccount\0");
        peer.handle(&Packet::new(SID_AUTH_ACCOUNT_LOGON, logon))
            .expect("account logon");
        peer.handle(&Packet::new(SID_AUTH_ACCOUNT_LOGON_PROOF, vec![0; 20]))
            .expect("proof");

        let ids: Vec<_> = packets(peer.writer)
            .into_iter()
            .map(|packet| packet.id)
            .collect();
        assert_eq!(
            ids,
            [
                SID_PING,
                SID_AUTH_CHECK,
                SID_AUTH_ACCOUNT_LOGON,
                SID_AUTH_ACCOUNT_LOGON_PROOF
            ]
        );
    }

    #[test]
    fn chat_event_uses_the_bncs_layout() {
        let mut peer = LegacyPeer::new(Vec::new(), "Raynor");
        peer.send_chat_event(EID_TALK, "Raynor", "hello", FLAG_OPERATOR, 42)
            .expect("event");
        let packet = packets(peer.writer).pop().expect("packet");
        assert_eq!(packet.id, SID_CHAT_EVENT);

        let mut payload = PayloadReader::new(&packet.payload);
        assert_eq!(payload.dword(), Some(EID_TALK));
        assert_eq!(payload.dword(), Some(FLAG_OPERATOR));
        assert_eq!(payload.dword(), Some(42));
        assert_eq!(payload.dword(), Some(0));
        assert_eq!(payload.dword(), Some(SYNTHETIC_ACCOUNT_NUMBER));
        assert_eq!(payload.dword(), Some(SYNTHETIC_ACCOUNT_NUMBER));
        assert_eq!(payload.c_string().as_deref(), Some("Raynor"));
        assert_eq!(payload.c_string().as_deref(), Some("hello"));
    }

    #[test]
    fn friends_list_uses_the_classic_bncs_layout_and_product_byte_order() {
        let mut peer = LegacyPeer::new(Vec::new(), "Raynor");
        peer.send_friends_list(&[FriendEntry {
            name: "Kerrigan#1234".into(),
            status: FRIEND_STATUS_MUTUAL | FRIEND_STATUS_AWAY,
            location: FRIEND_LOCATION_NOT_IN_CHAT,
            product: PRODUCT_STAR,
            location_name: "Battle.net (Away)".into(),
        }])
        .expect("friends list");

        let packet = packets(peer.writer).pop().expect("packet");
        assert_eq!(packet.id, SID_FRIENDS_LIST);
        let mut payload = PayloadReader::new(&packet.payload);
        assert_eq!(payload.byte(), Some(1));
        assert_eq!(payload.c_string().as_deref(), Some("Kerrigan#1234"));
        assert_eq!(
            payload.byte(),
            Some(FRIEND_STATUS_MUTUAL | FRIEND_STATUS_AWAY)
        );
        assert_eq!(payload.byte(), Some(FRIEND_LOCATION_NOT_IN_CHAT));
        assert_eq!(payload.dword(), Some(PRODUCT_STAR));
        assert_eq!(PRODUCT_STAR.to_be_bytes(), *b"STAR");
        assert_eq!(payload.c_string().as_deref(), Some("Battle.net (Away)"));
    }

    #[test]
    fn logged_on_clients_can_request_the_friends_snapshot() {
        let mut peer = LegacyPeer::new(Vec::new(), "Raynor");
        assert!(
            peer.handle(&Packet::new(SID_FRIENDS_LIST, Vec::new()))
                .expect("request before logon")
                .is_empty()
        );
        peer.account_accepted = true;
        peer.send_logon_success().expect("logon");
        assert_eq!(
            peer.handle(&Packet::new(SID_FRIENDS_LIST, Vec::new()))
                .expect("friends request"),
            vec![Action::RequestFriendsList]
        );
    }

    #[test]
    fn modern_logon_rejects_any_name_other_than_the_selected_toon() {
        let mut peer = LegacyPeer::new(Vec::new(), "Raynor");
        let mut logon = vec![0; 32];
        logon.extend_from_slice(b"Kerrigan\0");

        peer.handle(&Packet::new(SID_AUTH_ACCOUNT_LOGON, logon))
            .expect("account logon");
        assert!(
            peer.handle(&Packet::new(SID_ENTER_CHAT, b"Kerrigan\0".to_vec()))
                .expect("enter chat")
                .is_empty()
        );

        let response = packets(peer.writer).pop().expect("response");
        assert_eq!(response.id, SID_AUTH_ACCOUNT_LOGON);
        assert_eq!(&response.payload[..4], &1u32.to_le_bytes());
    }

    #[test]
    fn account_creation_is_always_rejected() {
        let mut peer = LegacyPeer::new(Vec::new(), "Raynor");
        for id in [
            SID_CREATE_ACCOUNT,
            SID_CREATE_ACCOUNT2,
            SID_AUTH_ACCOUNT_CREATE,
        ] {
            peer.handle(&Packet::new(id, Vec::new()))
                .expect("creation response");
        }

        let responses = packets(peer.writer);
        assert_eq!(responses.len(), 3);
        assert_eq!(&responses[0].payload[..4], &0u32.to_le_bytes());
        assert_eq!(&responses[1].payload[..4], &4u32.to_le_bytes());
        assert_eq!(&responses[2].payload[..4], &4u32.to_le_bytes());
    }

    #[test]
    fn join_channel_preserves_the_clients_join_semantics() {
        let mut peer = LegacyPeer::new(Vec::new(), "Raynor");
        peer.account_accepted = true;
        peer.send_logon_success().expect("logon");
        let mut payload = PayloadWriter::new();
        payload.dword(1);
        payload.c_string("StarCraft");

        let actions = peer
            .handle(&Packet::new(SID_JOIN_CHANNEL, payload.finish()))
            .expect("join");

        assert_eq!(
            actions,
            vec![Action::JoinChannel {
                flags: 1,
                channel: "StarCraft".into(),
            }]
        );
    }

    #[test]
    fn channel_list_request_and_response_use_the_classic_layout() {
        let mut peer = LegacyPeer::new(Vec::new(), "Raynor");
        assert_eq!(
            peer.handle(&Packet::new(
                SID_GET_CHANNEL_LIST,
                PRODUCT_STAR.to_le_bytes().to_vec(),
            ))
            .expect("channel list request"),
            vec![Action::RequestChannelList]
        );
        peer.send_channel_list(&["General".into(), "Co-op & Missions".into()])
            .expect("channel list response");

        let packet = packets(peer.writer).pop().expect("response");
        assert_eq!(packet.id, SID_GET_CHANNEL_LIST);
        assert_eq!(packet.payload, b"General\0Co-op & Missions\0\0");
    }

    #[test]
    fn read_user_data_returns_empty_values_with_the_request_id() {
        let mut peer = LegacyPeer::new(Vec::new(), "Raynor");
        peer.logged_on = true;
        let mut request = PayloadWriter::new();
        request.dword(1);
        request.dword(2);
        request.dword(0x1234_5678);
        request.c_string("Raynor");
        request.c_string("profile\\description");
        request.c_string("record\\STAR\\0\\wins");

        assert!(
            peer.handle(&Packet::new(SID_READ_USER_DATA, request.finish()))
                .expect("user data request")
                .is_empty()
        );

        let packet = packets(peer.writer).pop().expect("response");
        assert_eq!(packet.id, SID_READ_USER_DATA);
        let mut payload = PayloadReader::new(&packet.payload);
        assert_eq!(payload.dword(), Some(1));
        assert_eq!(payload.dword(), Some(2));
        assert_eq!(payload.dword(), Some(0x1234_5678));
        assert_eq!(payload.c_string().as_deref(), Some(""));
        assert_eq!(payload.c_string().as_deref(), Some(""));
        assert_eq!(payload.remaining(), 0);
    }

    #[test]
    fn leave_chat_is_validated_and_forwarded() {
        let mut peer = LegacyPeer::new(Vec::new(), "Raynor");
        peer.logged_on = true;
        assert_eq!(
            peer.handle(&Packet::new(SID_LEAVE_CHAT, Vec::new()))
                .expect("leave chat"),
            vec![Action::LeaveChat]
        );
        assert!(peer.handle(&Packet::new(SID_LEAVE_CHAT, vec![0])).is_err());
    }

    #[test]
    fn friend_delta_packets_use_entry_indices() {
        let friend = FriendEntry {
            name: "Kerrigan".into(),
            status: FRIEND_STATUS_MUTUAL,
            location: FRIEND_LOCATION_NOT_IN_CHAT,
            product: PRODUCT_STAR,
            location_name: "Battle.net".into(),
        };
        let mut peer = LegacyPeer::new(Vec::new(), "Raynor");
        peer.send_friend_add(&friend).expect("add");
        peer.send_friend_update(3, &friend).expect("update");
        peer.send_friend_position(3, 1).expect("move");
        peer.send_friend_remove(1).expect("remove");

        let packets = packets(peer.writer);
        assert_eq!(
            packets.iter().map(|packet| packet.id).collect::<Vec<_>>(),
            [
                SID_FRIENDS_ADD,
                SID_FRIENDS_UPDATE,
                SID_FRIENDS_POSITION,
                SID_FRIENDS_REMOVE,
            ]
        );
        assert_eq!(packets[1].payload[0], 3);
        assert_eq!(packets[2].payload, [3, 1]);
        assert_eq!(packets[3].payload, [1]);
    }

    #[test]
    fn keepalive_tracks_ping_replies_and_emits_null_packets() {
        let mut peer = LegacyPeer::new(Vec::new(), "Raynor");
        peer.handle(&Packet::new(SID_AUTH_INFO, Vec::new()))
            .expect("auth info");
        let initial = packets(std::mem::take(&mut peer.writer));
        let ping = initial
            .iter()
            .find(|packet| packet.id == SID_PING)
            .expect("initial ping");
        let token = u32::from_le_bytes(ping.payload[..4].try_into().expect("token"));
        assert!(matches!(
            peer.handle(&Packet::new(SID_PING, token.to_le_bytes().to_vec()))
                .expect("pong")
                .as_slice(),
            [Action::Pong { .. }]
        ));

        let now = Instant::now();
        peer.next_null = now;
        peer.next_ping = now + PING_INTERVAL;
        assert!(peer.service_keepalive(now).expect("keepalive"));
        assert_eq!(
            packets(peer.writer).as_slice(),
            [Packet::new(SID_NULL, Vec::new())]
        );
    }
}
