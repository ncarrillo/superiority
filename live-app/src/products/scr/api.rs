//! StarCraft: Remastered Live feed DTOs and product-scoped transport.

use serde::Deserialize;

use crate::shell::transport::get_json;

#[derive(Clone, Deserialize)]
pub(crate) struct FeedStatus {
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
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
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
            _ => tail.to_owned(),
        }
    }
}

#[derive(Deserialize)]
struct OverviewResponse {
    status: FeedStatus,
    #[serde(default)]
    channels: Vec<ChannelSummary>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct Sender {
    pub handle: u32,
    pub name: Option<String>,
}

impl Sender {
    pub fn display_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("User {}", self.handle))
    }
}

#[derive(Clone, Deserialize)]
pub(crate) struct Message {
    pub id: u64,
    pub ts: u64,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub sender: Option<Sender>,
    #[serde(default)]
    pub body: String,
}

fn default_kind() -> String {
    "talk".to_owned()
}

#[derive(Deserialize)]
struct MessagesResponse {
    #[serde(default)]
    messages: Vec<Message>,
    #[serde(default)]
    cursor: u64,
}

#[derive(Clone, Deserialize)]
pub(crate) struct RosterMember {
    pub handle: u32,
    pub name: Option<String>,
    #[serde(default)]
    pub presence: String,
    #[serde(default)]
    pub is_operator: bool,
    #[serde(default)]
    pub avatar: Option<String>,
}

#[derive(Deserialize)]
struct RosterResponse {
    #[serde(default)]
    members: Vec<RosterMember>,
}

pub(crate) async fn fetch_overview(
    backend: &str,
    feed_id: &str,
) -> Result<(FeedStatus, Vec<ChannelSummary>), String> {
    let overview =
        get_json::<OverviewResponse>(&format!("{}/overview", product_path(backend, feed_id)))
            .await?;
    Ok((overview.status, overview.channels))
}

pub(crate) async fn fetch_stream(
    backend: &str,
    feed_id: &str,
    channel: &str,
    cursor: Option<u64>,
) -> Result<(Vec<Message>, u64), String> {
    let base = format!("{}/messages", channel_path(backend, feed_id, channel));
    let messages = get_json::<MessagesResponse>(&cursor_url(&base, cursor)).await?;
    Ok((messages.messages, messages.cursor))
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

fn product_path(backend: &str, feed_id: &str) -> String {
    format!(
        "{}/v1/feeds/{}/products/scr",
        backend.trim_end_matches('/'),
        urlencoding::encode(feed_id)
    )
}

fn channel_path(backend: &str, feed_id: &str, channel: &str) -> String {
    format!(
        "{}/channels/{}",
        product_path(backend, feed_id),
        urlencoding::encode(channel)
    )
}

fn cursor_url(base: &str, cursor: Option<u64>) -> String {
    cursor.map_or_else(
        || base.to_owned(),
        |cursor| format!("{base}?after={cursor}"),
    )
}
