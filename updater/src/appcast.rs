use std::{cmp::Ordering, fmt};

use url::Url;

use crate::{Error, Result};

pub(crate) const SPARKLE_NAMESPACE: &str = "http://www.andymatuschak.org/xml-namespaces/sparkle";
pub const SUPERIORITY_NAMESPACE: &str =
    "https://superiority-sc2-updates.pages.dev/xml-namespaces/update";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    MacOs,
    WindowsX86_64,
    WindowsAarch64,
}

impl Platform {
    #[must_use]
    pub const fn current() -> Option<Self> {
        #[cfg(target_os = "macos")]
        {
            Some(Self::MacOs)
        }
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        {
            Some(Self::WindowsX86_64)
        }
        #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
        {
            Some(Self::WindowsAarch64)
        }
        #[cfg(not(any(
            target_os = "macos",
            all(target_os = "windows", target_arch = "x86_64"),
            all(target_os = "windows", target_arch = "aarch64")
        )))]
        {
            None
        }
    }

    #[must_use]
    pub const fn feed_name(self) -> &'static str {
        match self {
            Self::MacOs => "macos",
            Self::WindowsX86_64 => "windows-x86_64",
            Self::WindowsAarch64 => "windows-aarch64",
        }
    }

    pub fn from_feed_name(value: &str) -> Result<Self> {
        match value {
            "macos" => Ok(Self::MacOs),
            "windows-x86_64" => Ok(Self::WindowsX86_64),
            "windows-aarch64" => Ok(Self::WindowsAarch64),
            _ => Err(Error::UnsupportedPlatform(value.to_owned())),
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.feed_name())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Artifact {
    pub url: Url,
    pub content_length: u64,
    pub ed25519_signature: String,
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Release {
    pub title: String,
    pub version: String,
    pub build: String,
    pub minimum_system_version: Option<String>,
    pub notes: String,
    pub notes_format: String,
    pub artifact: Artifact,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Appcast {
    releases: Vec<Release>,
}

impl Appcast {
    pub fn parse(xml: &str, platform: Platform) -> Result<Self> {
        let document = roxmltree::Document::parse(xml)
            .map_err(|error| Error::InvalidAppcast(error.to_string()))?;
        let releases = document
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == "item")
            .filter_map(|item| parse_release(item, platform).transpose())
            .collect::<Result<Vec<_>>>()?;
        if releases.is_empty() {
            return Err(Error::UnsupportedPlatform(platform.to_string()));
        }
        Ok(Self { releases })
    }

    #[must_use]
    pub fn releases(&self) -> &[Release] {
        &self.releases
    }

    #[must_use]
    pub fn latest_newer_than(
        &self,
        current_build: &str,
        system_version: Option<&str>,
    ) -> Option<&Release> {
        self.releases
            .iter()
            .filter(|release| compare_versions(&release.build, current_build).is_gt())
            .filter(|release| {
                release
                    .minimum_system_version
                    .as_deref()
                    .is_none_or(|minimum| {
                        system_version
                            .is_none_or(|current| !compare_versions(current, minimum).is_lt())
                    })
            })
            .max_by(|left, right| compare_versions(&left.build, &right.build))
    }
}

fn parse_release(item: roxmltree::Node<'_, '_>, platform: Platform) -> Result<Option<Release>> {
    let artifact_node = find_artifact(item, platform);
    let Some(artifact_node) = artifact_node else {
        return Ok(None);
    };

    let child_text = |name: &str| {
        item.children()
            .find(|node| node.is_element() && node.tag_name().name() == name)
            .and_then(|node| node.text())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    let namespaced_child_text = |name: &str| {
        item.children()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == name
                    && node.tag_name().namespace() == Some(SPARKLE_NAMESPACE)
            })
            .and_then(|node| node.text())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };

    let build = namespaced_child_text("version")
        .ok_or_else(|| Error::InvalidAppcast("item is missing sparkle:version".into()))?;
    let version = namespaced_child_text("shortVersionString")
        .or_else(|| child_text("title"))
        .ok_or_else(|| Error::InvalidAppcast("item is missing a display version".into()))?;
    let title = child_text("title").unwrap_or(version);
    let description = item
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "description");
    let notes = description
        .and_then(|node| node.text())
        .unwrap_or("Release notes are not available for this update.")
        .trim()
        .to_owned();
    let notes_format = description
        .and_then(|node| attribute_by_local_name(node, "format"))
        .unwrap_or("plain-text")
        .to_owned();
    let url = required_attribute(artifact_node, "url")?;
    let content_length = required_attribute(artifact_node, "length")?
        .parse::<u64>()
        .map_err(|error| Error::InvalidAppcast(format!("invalid enclosure length: {error}")))?;
    let ed25519_signature = required_attribute(artifact_node, "edSignature")?.to_owned();
    let sha256 = attribute_by_local_name(artifact_node, "sha256").map(str::to_owned);

    Ok(Some(Release {
        title: title.to_owned(),
        version: version.to_owned(),
        build: build.to_owned(),
        minimum_system_version: namespaced_child_text("minimumSystemVersion").map(str::to_owned),
        notes,
        notes_format,
        artifact: Artifact {
            url: Url::parse(url).map_err(|error| {
                Error::InvalidAppcast(format!("invalid enclosure URL: {error}"))
            })?,
            content_length,
            ed25519_signature,
            sha256,
        },
    }))
}

fn find_artifact<'a>(
    item: roxmltree::Node<'a, 'a>,
    platform: Platform,
) -> Option<roxmltree::Node<'a, 'a>> {
    item.children().find(|node| {
        if !node.is_element() {
            return false;
        }
        match node.tag_name().name() {
            // an unqualified rss enclosure is the existing macos artifact.
            "enclosure" => {
                let declared = attribute_by_local_name(*node, "os");
                match (platform, declared) {
                    (Platform::MacOs, None | Some("macos"))
                    | (Platform::WindowsX86_64 | Platform::WindowsAarch64, Some("windows")) => true,
                    _ => declared == Some(platform.feed_name()),
                }
            }
            // rust clients also accept an extension that legacy clients ignore.
            "artifact" => {
                node.tag_name().namespace() == Some(SUPERIORITY_NAMESPACE)
                    && attribute_by_local_name(*node, "platform") == Some(platform.feed_name())
            }
            _ => false,
        }
    })
}

fn required_attribute<'a>(node: roxmltree::Node<'a, '_>, name: &str) -> Result<&'a str> {
    attribute_by_local_name(node, name)
        .ok_or_else(|| Error::InvalidAppcast(format!("artifact is missing {name}")))
}

fn attribute_by_local_name<'a>(node: roxmltree::Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|attribute| attribute.name() == name)
        .map(|attribute| attribute.value())
}

#[must_use]
pub fn compare_versions(left: &str, right: &str) -> Ordering {
    let mut left = VersionParts::new(left);
    let mut right = VersionParts::new(right);
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (Some(part), None) => {
                if !part.is_zero() {
                    return Ordering::Greater;
                }
            }
            (None, Some(part)) => {
                if !part.is_zero() {
                    return Ordering::Less;
                }
            }
            (Some(left), Some(right)) => {
                let ordering = left.cmp(&right);
                if !ordering.is_eq() {
                    return ordering;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VersionPart<'a> {
    Number(&'a str),
    Text(&'a str),
}

impl VersionPart<'_> {
    fn is_zero(self) -> bool {
        match self {
            Self::Number(value) => value.bytes().all(|byte| byte == b'0'),
            Self::Text(value) => value.is_empty(),
        }
    }
}

impl Ord for VersionPart<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (*self, *other) {
            (Self::Number(left), Self::Number(right)) => {
                let left = left.trim_start_matches('0');
                let right = right.trim_start_matches('0');
                left.len().cmp(&right.len()).then_with(|| left.cmp(right))
            }
            (Self::Number(_), Self::Text(_)) => Ordering::Greater,
            (Self::Text(_), Self::Number(_)) => Ordering::Less,
            (Self::Text(left), Self::Text(right)) => {
                left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase())
            }
        }
    }
}

impl PartialOrd for VersionPart<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct VersionParts<'a> {
    value: &'a str,
    offset: usize,
}

impl<'a> VersionParts<'a> {
    const fn new(value: &'a str) -> Self {
        Self { value, offset: 0 }
    }
}

impl<'a> Iterator for VersionParts<'a> {
    type Item = VersionPart<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.offset < self.value.len()
            && !self.value.as_bytes()[self.offset].is_ascii_alphanumeric()
        {
            self.offset += 1;
        }
        if self.offset == self.value.len() {
            return None;
        }
        let start = self.offset;
        let numeric = self.value.as_bytes()[start].is_ascii_digit();
        while self.offset < self.value.len()
            && self.value.as_bytes()[self.offset].is_ascii_digit() == numeric
            && self.value.as_bytes()[self.offset].is_ascii_alphanumeric()
        {
            self.offset += 1;
        }
        let part = &self.value[start..self.offset];
        Some(if numeric {
            VersionPart::Number(part)
        } else {
            VersionPart::Text(part)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::{Appcast, Platform, compare_versions};

    const CURRENT_FEED_SHAPE: &str = r#"<?xml version="1.0"?>
<rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle"
     xmlns:superiority="https://superiority-sc2-updates.pages.dev/xml-namespaces/update" version="2.0">
  <channel><item>
    <title>0.1.26</title>
    <sparkle:version>26</sparkle:version>
    <sparkle:shortVersionString>0.1.26</sparkle:shortVersionString>
    <sparkle:minimumSystemVersion>14.0</sparkle:minimumSystemVersion>
    <description sparkle:format="markdown"><![CDATA[# Notes]]></description>
    <enclosure url="https://example.test/mac.zip" length="42"
      sparkle:edSignature="mac-signature" />
    <superiority:artifact platform="windows-x86_64"
      url="https://example.test/windows.zip" length="84"
      sparkle:edSignature="windows-signature" />
  </item></channel>
</rss>"#;

    #[test]
    fn parses_existing_sparkle_enclosure_for_macos() {
        let appcast = Appcast::parse(CURRENT_FEED_SHAPE, Platform::MacOs).unwrap();
        let release = &appcast.releases()[0];
        assert_eq!(release.version, "0.1.26");
        assert_eq!(release.build, "26");
        assert_eq!(release.notes, "# Notes");
        assert_eq!(release.notes_format, "markdown");
        assert_eq!(
            release.artifact.url.as_str(),
            "https://example.test/mac.zip"
        );
    }

    #[test]
    fn extension_artifact_does_not_change_macos_selection() {
        let appcast = Appcast::parse(CURRENT_FEED_SHAPE, Platform::WindowsX86_64).unwrap();
        assert_eq!(
            appcast.releases()[0].artifact.url.as_str(),
            "https://example.test/windows.zip"
        );
    }

    #[test]
    fn selects_only_newer_compatible_release() {
        let appcast = Appcast::parse(CURRENT_FEED_SHAPE, Platform::MacOs).unwrap();
        assert!(appcast.latest_newer_than("25", Some("14.0")).is_some());
        assert!(appcast.latest_newer_than("26", Some("14.0")).is_none());
        assert!(appcast.latest_newer_than("25", Some("13.6")).is_none());
    }

    #[test]
    fn compares_numeric_builds_without_integer_overflow() {
        assert_eq!(compare_versions("26", "25"), Ordering::Greater);
        assert_eq!(compare_versions("1.10", "1.9"), Ordering::Greater);
        assert_eq!(compare_versions("0002", "2"), Ordering::Equal);
        assert_eq!(
            compare_versions("999999999999999999999", "99999999999999999999"),
            Ordering::Greater
        );
    }
}
