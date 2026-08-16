#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer as _, SigningKey};
use sha2::{Digest as _, Sha256};
use superiority_updater::{
    Artifact, Platform, add_platform_artifact, preserve_platform_artifacts, publish_macos_release,
    publish_platform_release,
};
use url::Url;

fn main() {
    if let Err(error) = run() {
        eprintln!("superiority-appcast: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = arguments()?;
    if let Some(path) = arguments.get("sign-file") {
        let key = signing_key(&arguments)?;
        let signature = key.sign(&fs::read(path)?);
        println!("{}", STANDARD.encode(signature.to_bytes()));
        return Ok(());
    }
    let input = required_path(&arguments, "input")?;
    let output = required_path(&arguments, "output")?;
    if let Some(previous) = arguments.get("preserve-from") {
        let generated = fs::read_to_string(&input)?;
        let previous = fs::read_to_string(previous)?;
        let updated = preserve_platform_artifacts(&generated, &previous)?;
        write_atomically(&output, updated.as_bytes())?;
        return Ok(());
    }
    let artifact_path = required_path(&arguments, "file")?;
    let build = required(&arguments, "build")?;
    let signature = required(&arguments, "signature")?;
    let url = Url::parse(required(&arguments, "url")?)?;
    let metadata = fs::metadata(&artifact_path)?;
    if !metadata.is_file() {
        return Err(format!(
            "artifact is not a regular file: {}",
            artifact_path.display()
        )
        .into());
    }
    let artifact = Artifact {
        url,
        content_length: metadata.len(),
        ed25519_signature: signature.to_owned(),
        sha256: (arguments.get("mode").map(String::as_str) != Some("publish-macos"))
            .then(|| sha256(&artifact_path))
            .transpose()?,
    };
    let xml = fs::read_to_string(&input)?;
    let mode = arguments.get("mode").map(String::as_str);
    let updated = if mode == Some("publish-macos") {
        let notes = fs::read_to_string(required_path(&arguments, "notes-file")?)?;
        let maximum_releases = arguments
            .get("maximum-releases")
            .map_or(Ok(3), |value| value.parse::<usize>())?;
        publish_macos_release(
            (!xml.trim().is_empty()).then_some(xml.as_str()),
            required(&arguments, "feed-url")?,
            required(&arguments, "title")?,
            required(&arguments, "version")?,
            build,
            required(&arguments, "published-at")?,
            required(&arguments, "minimum-system-version")?,
            &notes,
            &artifact,
            maximum_releases,
        )?
    } else if mode == Some("publish-platform") {
        let platform = Platform::from_feed_name(required(&arguments, "platform")?)?;
        let notes = fs::read_to_string(required_path(&arguments, "notes-file")?)?;
        let maximum_releases = arguments
            .get("maximum-releases")
            .map_or(Ok(3), |value| value.parse::<usize>())?;
        publish_platform_release(
            (!xml.trim().is_empty()).then_some(xml.as_str()),
            required(&arguments, "feed-url")?,
            required(&arguments, "title")?,
            required(&arguments, "version")?,
            build,
            required(&arguments, "published-at")?,
            &notes,
            platform,
            &artifact,
            maximum_releases,
        )?
    } else {
        let platform = Platform::from_feed_name(required(&arguments, "platform")?)?;
        add_platform_artifact(&xml, build, platform, &artifact)?
    };
    write_atomically(&output, updated.as_bytes())?;
    Ok(())
}

fn arguments() -> Result<BTreeMap<String, String>, String> {
    let mut values = std::env::args().skip(1);
    let mut arguments = BTreeMap::new();
    while let Some(flag) = values.next() {
        let Some(name) = flag.strip_prefix("--") else {
            return Err(format!("unexpected positional argument {flag:?}"));
        };
        let value = values
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        if arguments.insert(name.to_owned(), value).is_some() {
            return Err(format!("{flag} was provided more than once"));
        }
    }
    Ok(arguments)
}

fn required<'a>(arguments: &'a BTreeMap<String, String>, name: &str) -> Result<&'a str, String> {
    arguments
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("missing --{name}"))
}

fn required_path(arguments: &BTreeMap<String, String>, name: &str) -> Result<PathBuf, String> {
    required(arguments, name).map(PathBuf::from)
}

fn sha256(path: &Path) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 128 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn signing_key(
    arguments: &BTreeMap<String, String>,
) -> Result<SigningKey, Box<dyn std::error::Error>> {
    let encoded = if let Some(path) = arguments.get("key-file") {
        if path == "-" {
            let mut encoded = String::new();
            std::io::stdin().read_to_string(&mut encoded)?;
            encoded
        } else {
            fs::read_to_string(path)?
        }
    } else if let Ok(encoded) = std::env::var("SUPERIORITY_UPDATE_PRIVATE_KEY") {
        encoded
    } else if let Some(account) = arguments.get("keychain-account") {
        read_keychain_key(account)?
    } else {
        return Err(
            "signing requires --key-file, SUPERIORITY_UPDATE_PRIVATE_KEY, or --keychain-account"
                .into(),
        );
    };
    decode_signing_key(
        encoded.trim(),
        arguments.get("public-key").map(String::as_str),
    )
}

fn decode_signing_key(
    encoded: &str,
    expected_public_key: Option<&str>,
) -> Result<SigningKey, Box<dyn std::error::Error>> {
    let decoded = STANDARD.decode(encoded)?;
    if !matches!(decoded.len(), 32 | 64 | 96) {
        return Err("the decoded update private key must contain 32, 64, or 96 bytes".into());
    }
    let expected = expected_public_key
        .map(|value| STANDARD.decode(value))
        .transpose()?;
    let offsets: &[usize] = if decoded.len() == 32 {
        &[0]
    } else if expected.is_some() {
        &[0, 32, 64]
    } else {
        return Err("legacy update keys require --public-key to identify their seed".into());
    };
    for offset in offsets {
        let Some(candidate) = decoded.get(*offset..offset.saturating_add(32)) else {
            continue;
        };
        let seed: [u8; 32] = candidate.try_into()?;
        let key = SigningKey::from_bytes(&seed);
        if expected
            .as_deref()
            .is_none_or(|expected| key.verifying_key().to_bytes().as_slice() == expected)
        {
            return Ok(key);
        }
    }
    Err("the update private key does not match --public-key".into())
}

fn read_keychain_key(account: &str) -> Result<String, Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/usr/bin/security")
            .args([
                "find-generic-password",
                "-s",
                "https://sparkle-project.org",
                "-a",
                account,
                "-w",
            ])
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .output()?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr);
            return Err(format!("read update key from Keychain: {}", message.trim()).into());
        }
        Ok(String::from_utf8(output.stdout)?)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = account;
        Err("--keychain-account is only supported on macOS".into())
    }
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    use super::decode_signing_key;

    #[test]
    fn signing_key_must_match_the_published_key() {
        let encoded = STANDARD.encode([7_u8; 32]);
        let key = decode_signing_key(&encoded, None).unwrap();
        let public = STANDARD.encode(key.verifying_key().to_bytes());
        assert!(decode_signing_key(&encoded, Some(&public)).is_ok());
        assert!(decode_signing_key(&encoded, Some(&STANDARD.encode([8_u8; 32]))).is_err());
    }
}
