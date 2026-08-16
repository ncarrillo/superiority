use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use memmap2::Mmap;
use sha2::{Digest as _, Sha256};
use ureq::{
    Agent,
    tls::{RootCerts, TlsConfig, TlsProvider},
};

use crate::{Artifact, Error, Result};

const MAX_APPCAST_BYTES: u64 = 2 * 1024 * 1024;
const DOWNLOAD_BUFFER_BYTES: usize = 128 * 1024;

fn http_agent() -> Agent {
    Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent()
}

pub(crate) fn fetch_appcast(url: &url::Url, cancelled: &AtomicBool) -> Result<String> {
    check_cancelled(cancelled)?;
    let mut response = http_agent()
        .get(url.as_str())
        .header("Accept", "application/rss+xml, application/xml, text/xml")
        .header("Accept-Encoding", "identity")
        .header("User-Agent", "SuperiorityUpdater/0.1")
        .call()
        .map_err(|error| Error::Network(error.to_string()))?;
    check_cancelled(cancelled)?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_APPCAST_BYTES)
        .read_to_string()
        .map_err(|error| Error::Network(format!("read update feed: {error}")))
}

pub(crate) fn download_artifact(
    artifact: &Artifact,
    destination: &Path,
    cancelled: &Arc<AtomicBool>,
    mut progress: impl FnMut(f64),
) -> Result<()> {
    check_cancelled(cancelled)?;
    let parent = destination.parent().ok_or_else(|| {
        Error::InvalidArtifact("download destination has no parent directory".into())
    })?;
    fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
    let partial = destination.with_extension("download");
    let mut response = http_agent()
        .get(artifact.url.as_str())
        .header("Accept-Encoding", "identity")
        .header("User-Agent", "SuperiorityUpdater/0.1")
        .call()
        .map_err(|error| Error::Network(error.to_string()))?;
    let reported_length = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    if reported_length.is_some_and(|length| length != artifact.content_length) {
        return Err(Error::InvalidArtifact(format!(
            "server reported {reported_length:?} bytes; appcast requires {}",
            artifact.content_length
        )));
    }

    let mut source = response.body_mut().as_reader();
    let mut output = File::create(&partial).map_err(|error| Error::io(&partial, error))?;
    let mut buffer = vec![0_u8; DOWNLOAD_BUFFER_BYTES];
    let mut received = 0_u64;
    progress(0.0);
    loop {
        check_cancelled(cancelled)?;
        let count = source
            .read(&mut buffer)
            .map_err(|error| Error::Network(format!("read update artifact: {error}")))?;
        if count == 0 {
            break;
        }
        output
            .write_all(&buffer[..count])
            .map_err(|error| Error::io(&partial, error))?;
        received = received.saturating_add(count as u64);
        if received > artifact.content_length {
            return Err(Error::InvalidArtifact(format!(
                "download exceeded expected length {}",
                artifact.content_length
            )));
        }
        progress(progress_fraction(received, artifact.content_length.max(1)));
    }
    output
        .sync_all()
        .map_err(|error| Error::io(&partial, error))?;
    if received != artifact.content_length {
        return Err(Error::InvalidArtifact(format!(
            "downloaded {received} bytes; expected {}",
            artifact.content_length
        )));
    }
    fs::rename(&partial, destination).map_err(|error| Error::io(destination, error))?;
    progress(1.0);
    Ok(())
}

pub(crate) fn verify_artifact(
    artifact: &Artifact,
    public_key_base64: &str,
    path: &Path,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| Error::io(path, error))?;
    if !metadata.file_type().is_file() || metadata.len() != artifact.content_length {
        return Err(Error::InvalidArtifact(format!(
            "archive is not a regular {} byte file",
            artifact.content_length
        )));
    }
    let key_bytes = STANDARD
        .decode(public_key_base64)
        .map_err(|error| Error::InvalidSignature(format!("decode public key: {error}")))?;
    let key_bytes: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| Error::InvalidSignature("public key must contain 32 bytes".into()))?;
    let signature_bytes = STANDARD
        .decode(&artifact.ed25519_signature)
        .map_err(|error| Error::InvalidSignature(format!("decode signature: {error}")))?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|error| Error::InvalidSignature(error.to_string()))?;
    let key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|error| Error::InvalidSignature(error.to_string()))?;
    let file = File::open(path).map_err(|error| Error::io(path, error))?;
    // safety: the immutable file descriptor remains open for the lifetime of
    // the read-only mapping, and no updater code mutates the verified path.
    let mapping = unsafe { Mmap::map(&file) }.map_err(|error| Error::io(path, error))?;
    if let Some(expected_hash) = &artifact.sha256 {
        let actual_hash = Sha256::digest(&mapping);
        let actual_hash = hex_string(&actual_hash);
        if !actual_hash.eq_ignore_ascii_case(expected_hash) {
            return Err(Error::InvalidArtifact(format!(
                "SHA-256 mismatch: expected {expected_hash}, received {actual_hash}"
            )));
        }
    }
    key.verify_strict(&mapping, &signature)
        .map_err(|error| Error::InvalidSignature(error.to_string()))
}

fn hex_string(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[allow(clippy::cast_precision_loss)]
fn progress_fraction(completed: u64, total: u64) -> f64 {
    completed as f64 / total as f64
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Write as _, sync::atomic::AtomicBool};

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use ed25519_dalek::{Signer as _, SigningKey};
    use sha2::{Digest as _, Sha256};

    use crate::Artifact;

    use super::{fetch_appcast, hex_string, http_agent, verify_artifact};

    #[test]
    fn http_agent_uses_the_enabled_native_tls_provider() {
        let agent = http_agent();
        let tls = agent.config().tls_config();
        assert_eq!(tls.provider(), ureq::tls::TlsProvider::NativeTls);
        assert!(matches!(
            tls.root_certs(),
            ureq::tls::RootCerts::PlatformVerifier
        ));
    }

    #[test]
    #[ignore = "requires the public update service"]
    fn fetches_live_appcast() {
        let feed = fetch_appcast(
            &url::Url::parse("https://superiority-sc2-updates.pages.dev/appcast.xml").unwrap(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let appcast = crate::Appcast::parse(&feed, crate::Platform::MacOs).unwrap();
        assert!(!appcast.releases().is_empty());
    }

    #[test]
    fn verifies_sparkle_style_ed25519_signature_and_hash() {
        let mut archive = tempfile::NamedTempFile::new().unwrap();
        archive.write_all(b"signed update bytes").unwrap();
        archive.as_file().sync_all().unwrap();
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let signature = key.sign(b"signed update bytes");
        let artifact = Artifact {
            url: url::Url::parse("https://example.test/update.zip").unwrap(),
            content_length: 19,
            ed25519_signature: STANDARD.encode(signature.to_bytes()),
            sha256: Some(hex_string(&Sha256::digest(b"signed update bytes"))),
        };
        verify_artifact(
            &artifact,
            &STANDARD.encode(key.verifying_key().to_bytes()),
            archive.path(),
        )
        .unwrap();
    }

    #[test]
    fn rejects_changed_artifact() {
        let mut archive = tempfile::NamedTempFile::new().unwrap();
        archive.write_all(b"changed update byte").unwrap();
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let signature = key.sign(b"signed update bytes");
        let artifact = Artifact {
            url: url::Url::parse("https://example.test/update.zip").unwrap(),
            content_length: 19,
            ed25519_signature: STANDARD.encode(signature.to_bytes()),
            sha256: None,
        };
        assert!(
            verify_artifact(
                &artifact,
                &STANDARD.encode(key.verifying_key().to_bytes()),
                archive.path(),
            )
            .is_err()
        );
    }
}
