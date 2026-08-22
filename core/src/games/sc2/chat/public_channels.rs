use crate::{Error, Result, native::model::CacheStreamItems};

const PUBLIC_CHANNEL_MINIMUM: u16 = 1000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicChannel {
    pub identifier: u16,
    pub name: String,
}

pub(crate) fn load(response: &CacheStreamItems) -> Result<Vec<PublicChannel>> {
    let body = super::cache_catalog::load(response, "public-channel")?;
    parse(&body)
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

fn catalog_error(message: impl Into<String>) -> Error {
    Error::Transport(format!("public-channel catalog error: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    #[ignore = "requires the public Blizzard depot"]
    fn fetches_the_live_public_channel_catalog() {
        use crate::native::model::CacheStreamItem;

        let response = CacheStreamItems {
            items: vec![CacheStreamItem {
                publication_time: 0,
                content_handle: hex::decode(
                    "00786d6c0000555329642d083e3e78588c80bb8648c6fa2d34848bd0a6e5e292c309bbda8dad1c7d",
                )
                .unwrap()
                .try_into()
                .unwrap(),
            }],
            offset: 0,
            total_items: 1,
            token: 0,
        };
        let channels = load(&response).unwrap();
        assert!(channels.iter().any(|channel| channel.identifier == 1028));
    }
}
