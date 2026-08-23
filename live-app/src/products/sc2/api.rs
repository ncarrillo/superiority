use serde::Deserialize;
use superiority_ui::products::sc2::RosterUserTone;

use crate::shell::{FeedProduct, transport::get_json};

#[derive(Clone, Deserialize)]
pub(crate) struct FeedStatus {
    pub product: FeedProduct,
    pub state: String,
    pub session: Option<FeedSession>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct FeedSession {
    pub id: String,
}

#[derive(Clone, Deserialize)]
pub(crate) struct ChannelSummary {
    pub key: String,
    pub kind: String,
    pub name: String,
    pub first_seen_at: u64,
    pub member_count: usize,
}

impl ChannelSummary {
    pub fn label(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        let tail = self
            .key
            .split_once(':')
            .map_or(self.key.as_str(), |(_, tail)| tail);
        match self.kind.as_str() {
            "public" => format!("Channel {tail}"),
            "club" => format!("Group {tail}"),
            _ => tail.to_owned(),
        }
    }
}

#[derive(Deserialize)]
struct OverviewResponse {
    status: FeedStatus,
    channels: Vec<ChannelSummary>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct Sender {
    pub handle: u32,
    pub name: Option<String>,
    pub clan_tag: Option<String>,
}

impl Sender {
    pub fn display_name(&self) -> String {
        let name = self
            .name
            .clone()
            .unwrap_or_else(|| format!("User {}", self.handle));
        self.clan_tag.as_deref().map_or_else(
            || name.clone(),
            |clan| {
                if clan.is_empty() {
                    name.clone()
                } else {
                    format!("<{clan}> {name}")
                }
            },
        )
    }
}

#[derive(Clone, Deserialize)]
pub(crate) struct Message {
    pub id: u64,
    pub ts: u64,
    pub seq: u64,
    /// legacy talk lines carry a sender; member rows and cross-product notices
    /// arrive senderless
    #[serde(default)]
    pub sender: Option<Sender>,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub body: String,
}

fn default_kind() -> String {
    "talk".to_owned()
}

#[derive(Deserialize)]
struct MessagesResponse {
    messages: Vec<Message>,
    cursor: u64,
}

#[derive(Clone, Deserialize)]
pub(crate) struct PortraitRecord {
    pub t: u8,
    pub o: u8,
}

#[derive(Clone, Deserialize)]
pub(crate) struct RosterMember {
    pub handle: u32,
    pub name: Option<String>,
    pub clan_tag: Option<String>,
    pub presence: String,
    pub portrait: Option<PortraitRecord>,
    #[serde(default)]
    pub is_local: bool,
    pub joined_order: Option<u64>,
    #[serde(skip)]
    pub tone: RosterUserTone,
    #[serde(skip)]
    pub segment_start: bool,
}

#[derive(Deserialize)]
struct RosterResponse {
    members: Vec<RosterMember>,
}

#[derive(Clone)]
pub(crate) enum StreamItem {
    Message(Message),
}

impl StreamItem {
    pub fn sort_key(&self) -> (u64, u64) {
        let Self::Message(message) = self;
        (message.ts, message.seq)
    }

    pub fn animation_id(&self) -> String {
        let Self::Message(message) = self;
        format!("message-{}", message.id)
    }

    pub fn stable_key(&self) -> (&'static str, u64) {
        let Self::Message(message) = self;
        ("message", message.id)
    }
}

pub(crate) async fn fetch_overview(
    backend: &str,
    feed_id: &str,
) -> Result<(FeedStatus, Vec<ChannelSummary>), String> {
    let base = feed_path(backend, feed_id);
    // product-scoped so a feed whose latest-synced session is another product
    // cannot make this view think it is looking at the wrong game
    let overview = get_json::<OverviewResponse>(&format!("{base}/products/sc2/overview")).await?;
    Ok((overview.status, overview.channels))
}

pub(crate) async fn fetch_stream(
    backend: &str,
    feed_id: &str,
    channel: &str,
    message_cursor: Option<u64>,
) -> Result<(Vec<StreamItem>, u64), String> {
    let base = channel_path(backend, feed_id, channel);
    let messages =
        get_json::<MessagesResponse>(&cursor_url(&format!("{base}/messages"), message_cursor))
            .await?;
    let items = messages
        .messages
        .into_iter()
        .map(StreamItem::Message)
        .collect::<Vec<_>>();
    Ok((items, messages.cursor))
}

pub(crate) async fn fetch_roster(
    backend: &str,
    feed_id: &str,
    channel: &str,
) -> Result<Vec<RosterMember>, String> {
    get_json::<RosterResponse>(&format!(
        "{}/roster",
        channel_path(backend, feed_id, channel)
    ))
    .await
    .map(|response| response.members)
}

fn feed_path(backend: &str, feed_id: &str) -> String {
    format!(
        "{}/v1/feeds/{}",
        backend.trim_end_matches('/'),
        urlencoding::encode(feed_id)
    )
}

fn channel_path(backend: &str, feed_id: &str, channel: &str) -> String {
    format!(
        "{}/channels/{}",
        feed_path(backend, feed_id),
        urlencoding::encode(channel)
    )
}

fn cursor_url(base: &str, cursor: Option<u64>) -> String {
    cursor.map_or_else(
        || base.to_owned(),
        |cursor| format!("{base}?after={cursor}"),
    )
}
