use std::collections::BTreeMap;

use crate::{
    Error, Result,
    native::{errors, model::CacheStreamItems},
};

pub(crate) fn load(response: &CacheStreamItems) -> Result<usize> {
    let body = super::cache_catalog::load(response, "error")?;
    let entries = parse(&body)?;
    let count = entries.len();
    errors::install_localized_messages(entries);
    Ok(count)
}

fn parse(body: &[u8]) -> Result<BTreeMap<u16, String>> {
    let text = std::str::from_utf8(body)
        .map_err(|error| catalog_error(format!("catalog is not UTF-8: {error}")))?;
    let document = roxmltree::Document::parse(text)
        .map_err(|error| catalog_error(format!("catalog XML is invalid: {error}")))?;
    let entries = document
        .descendants()
        .filter(|node| node.has_tag_name("e"))
        .filter_map(|node| {
            let code = node.attribute("id")?.parse::<u16>().ok()?;
            let message = node.text()?.trim();
            (!message.is_empty()).then(|| (code, message.to_owned()))
        })
        .collect::<BTreeMap<_, _>>();
    if entries.is_empty() {
        return Err(catalog_error("catalog contains no error messages"));
    }
    Ok(entries)
}

fn catalog_error(message: impl Into<String>) -> Error {
    Error::Transport(format!(
        "Battle.net error catalog error: {}",
        message.into()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_authoritative_error_messages() {
        let entries = parse(
            br#"<?xml version="1.0"?><locale locale="enUS"><e id="301">Too many channels.</e><e id="402">Maximum number of characters reached.</e></locale>"#,
        )
        .unwrap();
        assert_eq!(entries[&301], "Too many channels.");
        assert_eq!(entries[&402], "Maximum number of characters reached.");
    }

    #[test]
    fn rejects_an_empty_catalog() {
        let error = parse(br#"<locale locale="enUS"/>"#).unwrap_err();
        assert!(error.to_string().contains("contains no error messages"));
    }
}
