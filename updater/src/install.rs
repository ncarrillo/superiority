use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use rand::Rng as _;
use serde::{Deserialize, Serialize};

use crate::{Artifact, Error, Platform, Release, Result, download, platform};

const INSTALL_PLAN_SCHEMA: u32 = 1;

#[derive(Clone, Debug)]
pub struct PreparedUpdate {
    pub release: Release,
    pub archive_path: PathBuf,
    pub staging_directory: PathBuf,
    pub install_source: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallPlan {
    pub schema: u32,
    pub nonce: String,
    pub app_name: String,
    pub application_id: String,
    pub platform: String,
    pub version: String,
    pub build: String,
    pub archive_path: PathBuf,
    pub staging_directory: PathBuf,
    pub install_source: PathBuf,
    pub install_target: PathBuf,
    pub relaunch_path: PathBuf,
    pub parent_pid: u32,
    pub public_key: String,
    pub artifact: SerializableArtifact,
    pub fallback_progress_delay_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableArtifact {
    pub url: String,
    pub content_length: u64,
    pub ed25519_signature: String,
    pub sha256: Option<String>,
}

impl From<&Artifact> for SerializableArtifact {
    fn from(artifact: &Artifact) -> Self {
        Self {
            url: artifact.url.to_string(),
            content_length: artifact.content_length,
            ed25519_signature: artifact.ed25519_signature.clone(),
            sha256: artifact.sha256.clone(),
        }
    }
}

impl TryFrom<&SerializableArtifact> for Artifact {
    type Error = Error;

    fn try_from(artifact: &SerializableArtifact) -> Result<Self> {
        Ok(Self {
            url: url::Url::parse(&artifact.url).map_err(|error| {
                Error::InvalidArtifact(format!("install plan artifact URL is invalid: {error}"))
            })?,
            content_length: artifact.content_length,
            ed25519_signature: artifact.ed25519_signature.clone(),
            sha256: artifact.sha256.clone(),
        })
    }
}

impl InstallPlan {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        app_name: &str,
        application_id: &str,
        platform: Platform,
        prepared: &PreparedUpdate,
        install_target: &Path,
        relaunch_path: &Path,
        public_key: &str,
    ) -> Self {
        let nonce: [u8; 24] = rand::rng().random();
        Self {
            schema: INSTALL_PLAN_SCHEMA,
            nonce: hex_string(&nonce),
            app_name: app_name.to_owned(),
            application_id: application_id.to_owned(),
            platform: platform.feed_name().to_owned(),
            version: prepared.release.version.clone(),
            build: prepared.release.build.clone(),
            archive_path: prepared.archive_path.clone(),
            staging_directory: prepared.staging_directory.clone(),
            install_source: prepared.install_source.clone(),
            install_target: install_target.to_owned(),
            relaunch_path: relaunch_path.to_owned(),
            parent_pid: std::process::id(),
            public_key: public_key.to_owned(),
            artifact: (&prepared.release.artifact).into(),
            fallback_progress_delay_ms: 700,
        }
    }

    pub fn write_securely(&self, directory: &Path) -> Result<PathBuf> {
        fs::create_dir_all(directory).map_err(|error| Error::io(directory, error))?;
        let destination = directory.join("install-plan.json");
        let temporary = directory.join("install-plan.json.part");
        let bytes = serde_json::to_vec(self)?;
        let mut options = File::options();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| Error::io(&temporary, error))?;
        file.write_all(&bytes)
            .map_err(|error| Error::io(&temporary, error))?;
        file.sync_all()
            .map_err(|error| Error::io(&temporary, error))?;
        fs::rename(&temporary, &destination).map_err(|error| Error::io(&destination, error))?;
        Ok(destination)
    }

    pub fn read(path: &Path) -> Result<Self> {
        let metadata = fs::symlink_metadata(path).map_err(|error| Error::io(path, error))?;
        if !metadata.file_type().is_file() {
            return Err(Error::Installation(
                "install plan is not a regular file".into(),
            ));
        }
        let plan: Self =
            serde_json::from_slice(&fs::read(path).map_err(|error| Error::io(path, error))?)?;
        if plan.schema != INSTALL_PLAN_SCHEMA || plan.nonce.len() != 48 {
            return Err(Error::Installation(
                "install plan schema or nonce is invalid".into(),
            ));
        }
        Ok(plan)
    }

    pub fn execute(&self) -> Result<()> {
        platform::execute_install(self)
    }

    pub fn validate_prepared_update(&self) -> Result<()> {
        let artifact = Artifact::try_from(&self.artifact)?;
        download::verify_artifact(&artifact, &self.public_key, &self.archive_path)?;
        platform::validate_install_source(&self.install_source, &self.application_id)?;
        platform::validate_install_target(&self.install_target, &self.application_id)
    }

    pub fn prepare_for_privileged_install(&self) -> Result<Self> {
        platform::validate_install_target(&self.install_target, &self.application_id)?;
        let artifact = Artifact::try_from(&self.artifact)?;
        let root = platform::privileged_staging_directory(&self.application_id, &self.nonce)?;
        let archive_path = root.join("update.zip");
        fs::copy(&self.archive_path, &archive_path)
            .map_err(|error| Error::io(&archive_path, error))?;
        File::open(&archive_path)
            .and_then(|file| file.sync_all())
            .map_err(|error| Error::io(&archive_path, error))?;
        download::verify_artifact(&artifact, &self.public_key, &archive_path)?;

        let staging_directory = root.join("Extracted");
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let install_source =
            platform::extract(&archive_path, &staging_directory, &cancelled, |_| {})?;
        platform::validate_install_source(&install_source, &self.application_id)?;
        let mut privileged = self.clone();
        privileged.archive_path = archive_path;
        privileged.staging_directory = staging_directory;
        privileged.install_source = install_source;
        Ok(privileged)
    }

    pub fn cleanup_privileged_staging(&self) {
        let Some(root) = self.staging_directory.parent() else {
            return;
        };
        #[cfg(not(target_os = "windows"))]
        let expected_parent = Path::new("/Library/Caches")
            .join(&self.application_id)
            .join("Updater");
        #[cfg(target_os = "windows")]
        let expected_parent = std::env::var_os("PROGRAMDATA")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(&self.application_id)
            .join("Updater");
        if root.parent() == Some(expected_parent.as_path()) && root.file_name().is_some() {
            let _ = fs::remove_dir_all(root);
        }
    }
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
