use std::time::Duration;

use ureq::{
    Agent,
    tls::{TlsConfig, TlsProvider},
};

use crate::{
    Error, Result,
    native::model::{CacheStreamItem, CacheStreamItems},
};

const MAX_CATALOG_BYTES: u64 = 64 * 1024;
const PUBLIC_CHANNEL_MINIMUM: u16 = 1000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicChannel {
    pub identifier: u16,
    pub name: String,
}

pub(crate) fn load(response: &CacheStreamItems) -> Result<Vec<PublicChannel>> {
    let item = response
        .items
        .iter()
        .max_by_key(|item| item.publication_time)
        .ok_or_else(|| catalog_error("Battle.net returned an empty public-channel catalog"))?;
    let location = DepotLocation::from_handle(item)?;
    let agent = depot_agent();
    let body = fetch(&agent, &location.primary_url())
        .or_else(|_| fetch(&agent, &location.storage_url()))?;
    parse(&body)
}

fn depot_agent() -> Agent {
    Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .build(),
        )
        .timeout_global(Some(Duration::from_secs(8)))
        .max_redirects(0)
        .http_status_as_error(true)
        .build()
        .new_agent()
}

fn fetch(agent: &Agent, url: &str) -> Result<Vec<u8>> {
    let mut response = agent
        .get(url)
        .header("Accept-Encoding", "identity")
        .call()
        .map_err(|error| catalog_error(format!("could not download {url}: {error}")))?;
    if !response.status().is_success() {
        return Err(catalog_error(format!(
            "could not download {url}: HTTP {}",
            response.status()
        )));
    }
    response
        .body_mut()
        .with_config()
        .limit(MAX_CATALOG_BYTES)
        .read_to_vec()
        .map_err(|error| catalog_error(format!("could not read {url}: {error}")))
}

fn parse(body: &[u8]) -> Result<Vec<PublicChannel>> {
    let text = std::str::from_utf8(body)
        .map_err(|error| catalog_error(format!("catalog is not UTF-8: {error}")))?;
    let document = roxmltree::Document::parse(text)
        .map_err(|error| catalog_error(format!("catalog XML is invalid: {error}")))?;
    let mut channels = document
        .descendants()
        .filter(|node| node.has_tag_name("e"))
        .filter_map(|node| {
            let identifier = node.attribute("id")?.parse::<u16>().ok()?;
            if identifier < PUBLIC_CHANNEL_MINIMUM {
                return None;
            }
            let name = node
                .text()?
                .trim()
                .strip_suffix("%d")
                .unwrap_or(node.text()?.trim())
                .trim();
            (!name.is_empty()).then(|| PublicChannel {
                identifier,
                name: name.to_owned(),
            })
        })
        .collect::<Vec<_>>();
    channels.sort_by_key(|channel| channel.identifier);
    channels.dedup_by_key(|channel| channel.identifier);
    if channels.is_empty() {
        return Err(catalog_error("catalog contains no public channels"));
    }
    Ok(channels)
}

struct DepotLocation {
    region: String,
    suffix: String,
    hash: String,
}

impl DepotLocation {
    fn from_handle(item: &CacheStreamItem) -> Result<Self> {
        let usage = ascii_component(&item.content_handle[..4], "usage")?;
        let region = ascii_component(&item.content_handle[4..8], "region")?.to_ascii_lowercase();
        if usage.is_empty() || region.is_empty() {
            return Err(catalog_error("content handle omits its usage or region"));
        }
        let hash = hex::encode(&item.content_handle[8..]);
        Ok(Self {
            region,
            suffix: usage,
            hash,
        })
    }

    fn primary_url(&self) -> String {
        format!(
            "https://{}-s2-depot.classic.blizzard.com/{}.{}",
            self.region, self.hash, self.suffix
        )
    }

    fn storage_url(&self) -> String {
        format!(
            "https://{}-s2-depot-storage.classic.blizzard.com/{}/{}/{}/{}.{}",
            self.region,
            &self.hash[0..2],
            &self.hash[2..4],
            &self.hash[4..6],
            self.hash,
            self.suffix
        )
    }
}

fn ascii_component(bytes: &[u8], label: &str) -> Result<String> {
    let bytes = bytes
        .iter()
        .copied()
        .filter(|byte| *byte != 0)
        .collect::<Vec<_>>();
    if bytes.is_empty()
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
    {
        return Err(catalog_error(format!(
            "content handle has an invalid {label}"
        )));
    }
    String::from_utf8(bytes)
        .map_err(|error| catalog_error(format!("content handle {label} is invalid: {error}")))
}

fn catalog_error(message: impl Into<String>) -> Error {
    Error::Transport(format!("public-channel catalog error: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_depot_location() {
        let item = CacheStreamItem {
            publication_time: 0,
            content_handle: hex::decode(
                "00786d6c0000555329642d083e3e78588c80bb8648c6fa2d34848bd0a6e5e292c309bbda8dad1c7d",
            )
            .unwrap()
            .try_into()
            .unwrap(),
        };
        let location = DepotLocation::from_handle(&item).unwrap();
        assert_eq!(location.region, "us");
        assert_eq!(location.suffix, "xml");
        assert_eq!(
            location.primary_url(),
            "https://us-s2-depot.classic.blizzard.com/29642d083e3e78588c80bb8648c6fa2d34848bd0a6e5e292c309bbda8dad1c7d.xml"
        );
    }

    #[test]
    fn parses_authoritative_channel_names() {
        let channels = parse(br#"<?xml version="1.0"?><locale locale="enUS"><e id="1">Test</e><e id="1028">General %d</e><e id="1033">Co-op &amp; Missions %d</e></locale>"#).unwrap();
        assert_eq!(
            channels,
            vec![
                PublicChannel {
                    identifier: 1028,
                    name: "General".into()
                },
                PublicChannel {
                    identifier: 1033,
                    name: "Co-op & Missions".into()
                },
            ]
        );
    }
}
