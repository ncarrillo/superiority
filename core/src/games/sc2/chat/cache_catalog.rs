use std::time::Duration;

use ureq::{
    Agent,
    tls::{RootCerts, TlsConfig, TlsProvider},
};

use crate::{
    Error, Result,
    native::model::{CacheStreamItem, CacheStreamItems},
};

const MAX_CACHE_ITEM_BYTES: u64 = 1024 * 1024;

pub(crate) fn load(response: &CacheStreamItems, label: &str) -> Result<Vec<u8>> {
    let item = response
        .items
        .iter()
        .max_by_key(|item| item.publication_time)
        .ok_or_else(|| catalog_error(label, "Battle.net returned an empty catalog"))?;
    let location = DepotLocation::from_handle(item, label)?;
    let agent = depot_agent();
    fetch(&agent, &location.primary_url(), label)
        .or_else(|_| fetch(&agent, &location.storage_url(), label))
}

fn depot_agent() -> Agent {
    Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .timeout_global(Some(Duration::from_secs(8)))
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .new_agent()
}

fn fetch(agent: &Agent, url: &str, label: &str) -> Result<Vec<u8>> {
    let request_headers = vec![("Accept-Encoding".to_owned(), "identity".to_owned())];
    crate::native::inspect::capture_http_request("GET", url, &request_headers, &[]);
    let mut response = agent
        .get(url)
        .header("Accept-Encoding", "identity")
        .call()
        .map_err(|error| catalog_error(label, format!("could not download {url}: {error}")))?;
    let status = response.status().as_u16();
    let response_headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                value
                    .to_str()
                    .map_or_else(|_| "<binary>".to_owned(), str::to_owned),
            )
        })
        .collect::<Vec<_>>();
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_CACHE_ITEM_BYTES)
        .read_to_vec()
        .map_err(|error| catalog_error(label, format!("could not read {url}: {error}")))?;
    crate::native::inspect::capture_http_response("GET", url, status, &response_headers, &body);
    if !(200..=299).contains(&status) {
        return Err(catalog_error(
            label,
            format!("could not download {url}: HTTP {status}"),
        ));
    }
    Ok(body)
}

struct DepotLocation {
    region: String,
    suffix: String,
    hash: String,
}

impl DepotLocation {
    fn from_handle(item: &CacheStreamItem, label: &str) -> Result<Self> {
        let usage = ascii_component(&item.content_handle[..4], "usage", label)?;
        let region =
            ascii_component(&item.content_handle[4..8], "region", label)?.to_ascii_lowercase();
        if usage.is_empty() || region.is_empty() {
            return Err(catalog_error(
                label,
                "content handle omits its usage or region",
            ));
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

fn ascii_component(bytes: &[u8], component: &str, label: &str) -> Result<String> {
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
        return Err(catalog_error(
            label,
            format!("content handle has an invalid {component}"),
        ));
    }
    String::from_utf8(bytes).map_err(|error| {
        catalog_error(
            label,
            format!("content handle {component} is invalid: {error}"),
        )
    })
}

fn catalog_error(label: &str, message: impl Into<String>) -> Error {
    Error::Transport(format!(
        "Battle.net {label} catalog error: {}",
        message.into()
    ))
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
        let location = DepotLocation::from_handle(&item, "test").unwrap();
        assert_eq!(location.region, "us");
        assert_eq!(location.suffix, "xml");
        assert_eq!(
            location.primary_url(),
            "https://us-s2-depot.classic.blizzard.com/29642d083e3e78588c80bb8648c6fa2d34848bd0a6e5e292c309bbda8dad1c7d.xml"
        );
    }

    #[test]
    fn depot_agent_uses_native_platform_trust() {
        let agent = depot_agent();
        let tls = agent.config().tls_config();
        assert_eq!(tls.provider(), TlsProvider::NativeTls);
        assert!(matches!(tls.root_certs(), RootCerts::PlatformVerifier));
    }
}
