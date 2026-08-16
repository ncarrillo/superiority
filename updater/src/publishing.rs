use std::fmt::Write as _;

use crate::{
    Appcast, Artifact, Error, Platform, Result,
    appcast::{SPARKLE_NAMESPACE, SUPERIORITY_NAMESPACE},
};

/// adds or replaces the rust updater artifact for one release and platform.
///
/// the existing sparkle enclosure and bytes outside the extension remain unchanged.
pub fn add_platform_artifact(
    xml: &str,
    build: &str,
    platform: Platform,
    artifact: &Artifact,
) -> Result<String> {
    if platform == Platform::MacOs {
        return Err(Error::InvalidAppcast(
            "macOS must continue to use the existing Sparkle enclosure".into(),
        ));
    }

    let mut output = ensure_namespace(xml)?;
    let document = parse_document(&output)?;
    let release = find_release_item(&document, build)?;
    let existing = release
        .children()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "artifact"
                && node.tag_name().namespace() == Some(SUPERIORITY_NAMESPACE)
                && node.attribute("platform") == Some(platform.feed_name())
        })
        .map(|node| node.range())
        .collect::<Vec<_>>();
    drop(document);
    for range in existing.into_iter().rev() {
        output.replace_range(range, "");
    }

    let document = parse_document(&output)?;
    let release = find_release_item(&document, build)?;
    let closing = release
        .range()
        .end
        .checked_sub("</item>".len())
        .ok_or_else(|| Error::InvalidAppcast("release item has an invalid range".into()))?;
    let element = artifact_element(platform, artifact);
    output.insert_str(closing, &element);

    let parsed = Appcast::parse(&output, platform)?;
    let inserted = parsed
        .releases()
        .iter()
        .find(|release| release.build == build)
        .ok_or_else(|| Error::InvalidAppcast("inserted artifact could not be read back".into()))?;
    if inserted.artifact != *artifact {
        return Err(Error::InvalidAppcast(
            "inserted artifact did not round-trip through the appcast".into(),
        ));
    }
    Ok(output)
}

/// restores windows extensions from an older copy after another tool
/// regenerates the appcast. only builds that are still present in the
/// generated feed are carried forward.
pub fn preserve_platform_artifacts(generated: &str, previous: &str) -> Result<String> {
    let builds = release_builds(generated)?;
    let mut output = generated.to_owned();
    for platform in [Platform::WindowsX86_64, Platform::WindowsAarch64] {
        let Ok(previous) = Appcast::parse(previous, platform) else {
            continue;
        };
        for release in previous.releases() {
            if builds.iter().any(|build| build == &release.build) {
                output =
                    add_platform_artifact(&output, &release.build, platform, &release.artifact)?;
            }
        }
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
pub fn publish_macos_release(
    existing: Option<&str>,
    feed_url: &str,
    title: &str,
    version: &str,
    build: &str,
    published_at: &str,
    minimum_system_version: &str,
    notes: &str,
    artifact: &Artifact,
    maximum_releases: usize,
) -> Result<String> {
    if maximum_releases == 0 {
        return Err(Error::InvalidAppcast(
            "the feed must retain at least one release".into(),
        ));
    }
    if notes.contains("]]>") {
        return Err(Error::InvalidAppcast(
            "release notes cannot contain the CDATA terminator ]]>".into(),
        ));
    }
    let mut output = existing.map_or_else(
        || empty_appcast(feed_url),
        |xml| xml.trim_end().to_owned() + "\n",
    );
    let document = parse_document(&output)?;
    let channel = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "channel")
        .ok_or_else(|| Error::InvalidAppcast("feed is missing its channel".into()))?;
    let items = channel
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "item")
        .collect::<Vec<_>>();
    let mut retained = 0_usize;
    let mut remove = Vec::new();
    for item in items {
        if item_build(item).is_some_and(|item_build| item_build == build) {
            remove.push(item.range());
        } else if retained < maximum_releases - 1 {
            retained += 1;
        } else {
            remove.push(item.range());
        }
    }
    drop(document);
    for range in remove.into_iter().rev() {
        output.replace_range(range, "");
    }

    let document = parse_document(&output)?;
    let channel = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "channel")
        .ok_or_else(|| Error::InvalidAppcast("feed is missing its channel".into()))?;
    let first_item = channel
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "item");
    let insertion = if let Some(item) = first_item {
        insertion_line_start(&output, item.range().start)
    } else {
        let closing = channel
            .range()
            .end
            .checked_sub("</channel>".len())
            .ok_or_else(|| Error::InvalidAppcast("channel has an invalid range".into()))?;
        insertion_line_start(&output, closing)
    };
    let item = macos_item(
        title,
        version,
        build,
        published_at,
        minimum_system_version,
        notes,
        artifact,
    );
    output.insert_str(insertion, &item);

    let parsed = Appcast::parse(&output, Platform::MacOs)?;
    let published = parsed
        .releases()
        .iter()
        .find(|release| release.build == build)
        .ok_or_else(|| {
            Error::InvalidAppcast("published macOS release could not be read back".into())
        })?;
    if published.artifact != *artifact || published.version != version {
        return Err(Error::InvalidAppcast(
            "published macOS release did not round-trip through the appcast".into(),
        ));
    }
    if parsed.releases().len() > maximum_releases {
        return Err(Error::InvalidAppcast(
            "published appcast retained too many releases".into(),
        ));
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
pub fn publish_platform_release(
    existing: Option<&str>,
    feed_url: &str,
    title: &str,
    version: &str,
    build: &str,
    published_at: &str,
    notes: &str,
    platform: Platform,
    artifact: &Artifact,
    maximum_releases: usize,
) -> Result<String> {
    if platform == Platform::MacOs {
        return Err(Error::InvalidAppcast(
            "macOS releases must use the existing Sparkle enclosure".into(),
        ));
    }
    if maximum_releases == 0 {
        return Err(Error::InvalidAppcast(
            "the feed must retain at least one release".into(),
        ));
    }
    if notes.contains("]]>") {
        return Err(Error::InvalidAppcast(
            "release notes cannot contain the CDATA terminator ]]>".into(),
        ));
    }
    let initial = existing.map_or_else(
        || empty_appcast(feed_url),
        |xml| xml.trim_end().to_owned() + "\n",
    );
    let mut output = ensure_namespace(&initial)?;
    let document = parse_document(&output)?;
    let channel = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "channel")
        .ok_or_else(|| Error::InvalidAppcast("feed is missing its channel".into()))?;
    let items = channel
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "item")
        .collect::<Vec<_>>();
    let mut retained = 0_usize;
    let mut remove = Vec::new();
    for item in items {
        if item_build(item).is_some_and(|item_build| item_build == build) {
            remove.push(item.range());
        } else if retained < maximum_releases - 1 {
            retained += 1;
        } else {
            remove.push(item.range());
        }
    }
    drop(document);
    for range in remove.into_iter().rev() {
        output.replace_range(range, "");
    }

    let document = parse_document(&output)?;
    let channel = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "channel")
        .ok_or_else(|| Error::InvalidAppcast("feed is missing its channel".into()))?;
    let first_item = channel
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "item");
    let insertion = if let Some(item) = first_item {
        insertion_line_start(&output, item.range().start)
    } else {
        let closing = channel
            .range()
            .end
            .checked_sub("</channel>".len())
            .ok_or_else(|| Error::InvalidAppcast("channel has an invalid range".into()))?;
        insertion_line_start(&output, closing)
    };
    output.insert_str(
        insertion,
        &platform_item(
            title,
            version,
            build,
            published_at,
            notes,
            platform,
            artifact,
        ),
    );

    let parsed = Appcast::parse(&output, platform)?;
    let published = parsed
        .releases()
        .iter()
        .find(|release| release.build == build)
        .ok_or_else(|| {
            Error::InvalidAppcast("published platform release could not be read back".into())
        })?;
    if published.artifact != *artifact || published.version != version {
        return Err(Error::InvalidAppcast(
            "published platform release did not round-trip through the appcast".into(),
        ));
    }
    if parsed.releases().len() > maximum_releases {
        return Err(Error::InvalidAppcast(
            "published appcast retained too many releases".into(),
        ));
    }
    Ok(output)
}

fn ensure_namespace(xml: &str) -> Result<String> {
    let declaration = format!("xmlns:superiority=\"{SUPERIORITY_NAMESPACE}\"");
    if xml.contains(&declaration) {
        return Ok(xml.to_owned());
    }
    if xml.contains("xmlns:superiority=") {
        return Err(Error::InvalidAppcast(
            "the superiority namespace is already bound to another URL".into(),
        ));
    }
    let start = xml
        .find("<rss")
        .ok_or_else(|| Error::InvalidAppcast("feed is missing its rss root".into()))?;
    let end = xml[start..]
        .find('>')
        .map(|offset| start + offset)
        .ok_or_else(|| Error::InvalidAppcast("feed has an unterminated rss root".into()))?;
    let mut output = xml.to_owned();
    output.insert_str(end, &format!(" {declaration}"));
    Ok(output)
}

fn insertion_line_start(xml: &str, offset: usize) -> usize {
    let start = xml[..offset]
        .rfind('\n')
        .map_or(offset, |newline| newline + 1);
    if xml[start..offset].chars().all(char::is_whitespace) {
        start
    } else {
        offset
    }
}

fn parse_document(xml: &str) -> Result<roxmltree::Document<'_>> {
    roxmltree::Document::parse(xml).map_err(|error| Error::InvalidAppcast(error.to_string()))
}

fn release_builds(xml: &str) -> Result<Vec<String>> {
    let document = parse_document(xml)?;
    Ok(document
        .descendants()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "version"
                && node.tag_name().namespace() == Some(SPARKLE_NAMESPACE)
        })
        .filter_map(|node| node.text().map(str::trim).map(str::to_owned))
        .collect())
}

fn item_build<'a>(item: roxmltree::Node<'a, '_>) -> Option<&'a str> {
    item.children()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "version"
                && node.tag_name().namespace() == Some(SPARKLE_NAMESPACE)
        })
        .and_then(|node| node.text())
        .map(str::trim)
}

fn find_release_item<'a>(
    document: &'a roxmltree::Document<'a>,
    build: &str,
) -> Result<roxmltree::Node<'a, 'a>> {
    document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "item")
        .find(|item| {
            item.children().any(|child| {
                child.is_element()
                    && child.tag_name().name() == "version"
                    && child.tag_name().namespace() == Some(SPARKLE_NAMESPACE)
                    && child.text().is_some_and(|value| value.trim() == build)
            })
        })
        .ok_or_else(|| Error::InvalidAppcast(format!("feed has no release with build {build}")))
}

fn artifact_element(platform: Platform, artifact: &Artifact) -> String {
    let mut element = format!(
        "\n            <superiority:artifact platform=\"{}\" url=\"{}\" length=\"{}\" sparkle:edSignature=\"{}\"",
        escape_attribute(platform.feed_name()),
        escape_attribute(artifact.url.as_str()),
        artifact.content_length,
        escape_attribute(&artifact.ed25519_signature),
    );
    if let Some(sha256) = &artifact.sha256 {
        let _ = write!(element, " sha256=\"{}\"", escape_attribute(sha256));
    }
    element.push_str(" />");
    element
}

fn empty_appcast(feed_url: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\n<rss xmlns:sparkle=\"{SPARKLE_NAMESPACE}\" version=\"2.0\">\n    <channel>\n        <title>Superiority updates</title>\n        <link>{}</link>\n        <description>Signed Superiority application updates.</description>\n        <language>en</language>\n    </channel>\n</rss>\n",
        escape_text(feed_url)
    )
}

#[allow(clippy::too_many_arguments)]
fn macos_item(
    title: &str,
    version: &str,
    build: &str,
    published_at: &str,
    minimum_system_version: &str,
    notes: &str,
    artifact: &Artifact,
) -> String {
    format!(
        "        <item>\n            <title>{}</title>\n            <pubDate>{}</pubDate>\n            <sparkle:version>{}</sparkle:version>\n            <sparkle:shortVersionString>{}</sparkle:shortVersionString>\n            <sparkle:minimumSystemVersion>{}</sparkle:minimumSystemVersion>\n            <description sparkle:format=\"markdown\"><![CDATA[{}]]></description>\n            <enclosure url=\"{}\" length=\"{}\" type=\"application/octet-stream\" sparkle:edSignature=\"{}\"/>\n        </item>\n",
        escape_text(title),
        escape_text(published_at),
        escape_text(build),
        escape_text(version),
        escape_text(minimum_system_version),
        notes.trim(),
        escape_attribute(artifact.url.as_str()),
        artifact.content_length,
        escape_attribute(&artifact.ed25519_signature),
    )
}

#[allow(clippy::too_many_arguments)]
fn platform_item(
    title: &str,
    version: &str,
    build: &str,
    published_at: &str,
    notes: &str,
    platform: Platform,
    artifact: &Artifact,
) -> String {
    format!(
        "        <item>\n            <title>{}</title>\n            <pubDate>{}</pubDate>\n            <sparkle:version>{}</sparkle:version>\n            <sparkle:shortVersionString>{}</sparkle:shortVersionString>\n            <description sparkle:format=\"markdown\"><![CDATA[{}]]></description>{}\n        </item>\n",
        escape_text(title),
        escape_text(published_at),
        escape_text(build),
        escape_text(version),
        notes.trim(),
        artifact_element(platform, artifact),
    )
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::{add_platform_artifact, preserve_platform_artifacts, publish_platform_release};
    use crate::{Appcast, Artifact, Platform, SUPERIORITY_NAMESPACE};

    const MAC_ONLY: &str = r#"<?xml version="1.0"?>
<rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" version="2.0">
  <channel>
    <item>
      <title>Version 0.1.26</title>
      <sparkle:version>26</sparkle:version>
      <sparkle:shortVersionString>0.1.26</sparkle:shortVersionString>
      <description sparkle:format="markdown"><![CDATA[# Notes & details]]></description>
      <enclosure url="https://example.test/mac.zip" length="42" sparkle:edSignature="mac" />
    </item>
  </channel>
</rss>
"#;

    fn windows_artifact(url: &str) -> Artifact {
        Artifact {
            url: Url::parse(url).unwrap(),
            content_length: 84,
            ed25519_signature: "windows-signature".into(),
            sha256: Some("0123456789abcdef".into()),
        }
    }

    #[test]
    fn augments_the_existing_feed_without_changing_the_macos_artifact() {
        let artifact = windows_artifact("https://example.test/windows.zip?one=1&two=2");
        let updated =
            add_platform_artifact(MAC_ONLY, "26", Platform::WindowsX86_64, &artifact).unwrap();

        assert!(updated.contains(&format!("xmlns:superiority=\"{SUPERIORITY_NAMESPACE}\"")));
        assert!(updated.contains("<![CDATA[# Notes & details]]>"));
        let mac = Appcast::parse(&updated, Platform::MacOs).unwrap();
        assert_eq!(
            mac.releases()[0].artifact.url.as_str(),
            "https://example.test/mac.zip"
        );
        let windows = Appcast::parse(&updated, Platform::WindowsX86_64).unwrap();
        assert_eq!(windows.releases()[0].artifact, artifact);
    }

    #[test]
    fn replaces_an_existing_platform_artifact() {
        let first = add_platform_artifact(
            MAC_ONLY,
            "26",
            Platform::WindowsX86_64,
            &windows_artifact("https://example.test/old.zip"),
        )
        .unwrap();
        let replacement = windows_artifact("https://example.test/new.zip");
        let updated =
            add_platform_artifact(&first, "26", Platform::WindowsX86_64, &replacement).unwrap();

        assert!(!updated.contains("old.zip"));
        assert_eq!(
            updated.matches("<superiority:artifact").count(),
            1,
            "the artifact should be replaced, not duplicated"
        );
        assert_eq!(
            Appcast::parse(&updated, Platform::WindowsX86_64)
                .unwrap()
                .releases()[0]
                .artifact,
            replacement
        );
    }

    #[test]
    fn restores_only_extensions_for_releases_the_new_feed_kept() {
        let previous = add_platform_artifact(
            MAC_ONLY,
            "26",
            Platform::WindowsX86_64,
            &windows_artifact("https://example.test/windows.zip"),
        )
        .unwrap();
        let regenerated = MAC_ONLY.replace("# Notes & details", "# Regenerated notes");
        let restored = preserve_platform_artifacts(&regenerated, &previous).unwrap();

        assert!(restored.contains("# Regenerated notes"));
        assert_eq!(
            Appcast::parse(&restored, Platform::WindowsX86_64)
                .unwrap()
                .releases()[0]
                .artifact
                .url
                .as_str(),
            "https://example.test/windows.zip"
        );
    }

    #[test]
    fn publishes_macos_release_and_retains_the_requested_history() {
        let mac = Artifact {
            url: Url::parse("https://example.test/mac-27.zip").unwrap(),
            content_length: 128,
            ed25519_signature: "mac-27-signature".into(),
            sha256: None,
        };
        let published = super::publish_macos_release(
            Some(MAC_ONLY),
            "https://example.test/appcast.xml",
            "0.1.27",
            "0.1.27",
            "27",
            "Fri, 14 Aug 2026 12:00:00 -0400",
            "14.0",
            "# New notes\n\n- One",
            &mac,
            2,
        )
        .unwrap();
        let appcast = Appcast::parse(&published, Platform::MacOs).unwrap();
        assert_eq!(appcast.releases().len(), 2);
        assert_eq!(appcast.releases()[0].build, "27");
        assert_eq!(appcast.releases()[0].artifact, mac);
        assert_eq!(appcast.releases()[1].build, "26");
    }

    #[test]
    fn can_create_the_first_appcast() {
        let mac = Artifact {
            url: Url::parse("https://example.test/mac.zip").unwrap(),
            content_length: 42,
            ed25519_signature: "mac-signature".into(),
            sha256: None,
        };
        let published = super::publish_macos_release(
            None,
            "https://example.test/appcast.xml",
            "0.1.1",
            "0.1.1",
            "1",
            "Fri, 14 Aug 2026 12:00:00 -0400",
            "14.0",
            "# First release",
            &mac,
            3,
        )
        .unwrap();
        assert_eq!(
            Appcast::parse(&published, Platform::MacOs)
                .unwrap()
                .releases()[0]
                .build,
            "1"
        );
    }

    #[test]
    fn can_create_a_windows_only_appcast() {
        let artifact = windows_artifact("http://192.0.2.1:8765/windows.zip");
        let published = publish_platform_release(
            None,
            "http://192.0.2.1:8765/appcast.xml",
            "0.1.30 test update",
            "0.1.30",
            "30",
            "Fri, 14 Aug 2026 12:00:00 -0400",
            "# windows updater test",
            Platform::WindowsX86_64,
            &artifact,
            2,
        )
        .unwrap();
        let appcast = Appcast::parse(&published, Platform::WindowsX86_64).unwrap();
        assert_eq!(appcast.releases().len(), 1);
        assert_eq!(appcast.releases()[0].build, "30");
        assert_eq!(appcast.releases()[0].artifact, artifact);
        assert!(!published.contains("<enclosure"));
    }
}
