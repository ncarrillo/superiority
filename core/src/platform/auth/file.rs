use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::{
    fs::Permissions,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
};

use zeroize::Zeroize;

use crate::{Error, Result, bgs::SecretBytes, product::Product};

use super::CredentialStore;

const MAX_CREDENTIAL_BYTES: usize = 16 * 1024;
/// What the cache was called when there was only ever one product to cache for.
/// Still read, once, so signing in does not start over — see `for_product`.
const LEGACY_CREDENTIAL_FILENAME: &str = "web-credentials.bin";

/// A web credential is issued per program: `GenerateWebCredentials` takes the
/// product it is for, and one signed in as `StarCraft II` is not one signed in
/// as Remastered. Two sessions sharing a file would overwrite each other's.
fn credential_filename(product: Product) -> String {
    format!("web-credentials-{}.bin", product.code())
}

#[derive(Clone, Debug)]
pub struct FileCredentialStore {
    path: Option<PathBuf>,
    /// Where a `StarCraft II` credential written before the cache was keyed by
    /// product still sits. Read only if the product's own file is absent, and
    /// promoted to it on the next store.
    legacy: Option<PathBuf>,
}

impl Default for FileCredentialStore {
    fn default() -> Self {
        Self::for_product(Product::StarCraft2)
    }
}

impl CredentialStore for FileCredentialStore {
    fn load(&self) -> Result<Option<SecretBytes>> {
        let path = self.path()?;
        // fall back to the un-keyed file exactly once, so an existing sign-in
        // survives the rename rather than sending everyone back to the browser
        let path = match self.legacy.as_deref() {
            Some(legacy) if !path.exists() && legacy.exists() => legacy,
            _ => path,
        };
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(cache_error("inspect", &error)),
        };
        if !metadata.file_type().is_file() {
            return Err(Error::Authentication(
                "credential cache is not a regular file".into(),
            ));
        }
        if metadata.len() == 0 || metadata.len() > MAX_CREDENTIAL_BYTES as u64 {
            return Err(Error::Authentication(
                "credential cache has an invalid length".into(),
            ));
        }

        let mut value = fs::read(path).map_err(|error| cache_error("read", &error))?;
        if value.is_empty() || value.len() > MAX_CREDENTIAL_BYTES {
            value.zeroize();
            return Err(Error::Authentication(
                "credential cache has an invalid length".into(),
            ));
        }
        SecretBytes::new(value).map(Some)
    }

    fn store(&self, credential: &SecretBytes) -> Result<()> {
        if credential.is_empty() || credential.len() > MAX_CREDENTIAL_BYTES {
            return Err(Error::Authentication(
                "credential cache has an invalid length".into(),
            ));
        }

        let path = self.path()?;
        let parent = path.parent().ok_or_else(|| {
            Error::Authentication("credential cache has no parent directory".into())
        })?;
        // a bare relative filename has an empty parent, which is the working
        // directory: it already exists, and creating or chmod-ing "" fails.
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            create_private_directory(parent)?;
            parent
        };

        // named after the file it will become, so two products writing at once
        // cannot collide on the same temporary
        let temporary = parent.join(format!(
            ".{}.{}-{}.tmp",
            path.file_name()
                .unwrap_or(std::ffi::OsStr::new("web-credentials"))
                .to_string_lossy(),
            std::process::id(),
            rand::random::<u64>()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&temporary)
            .map_err(|error| cache_error("create", &error))?;
        if let Err(error) = file
            .write_all(credential.expose())
            .and_then(|()| file.sync_all())
        {
            let _ = fs::remove_file(&temporary);
            return Err(cache_error("write", &error));
        }
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::remove_file(&temporary);
            return Err(cache_error("replace", &error));
        }
        secure_file(path)
    }

    fn delete(&self) -> Result<()> {
        let path = self.path()?;
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(cache_error("delete", &error)),
        }
    }
}

impl FileCredentialStore {
    fn path(&self) -> Result<&Path> {
        self.path.as_deref().ok_or_else(|| {
            Error::Authentication("could not locate the current user's home directory".into())
        })
    }

    /// a store at an explicit path, for embedders that must not share the
    /// app's cache.
    #[must_use]
    pub fn at(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            legacy: None,
        }
    }

    /// the app's cache for one product. each product signs in separately, so
    /// each gets its own file.
    #[must_use]
    pub fn for_product(product: Product) -> Self {
        Self {
            path: credential_directory().map(|dir| dir.join(credential_filename(product))),
            legacy: (product == Product::StarCraft2)
                .then(credential_directory)
                .flatten()
                .map(|dir| dir.join(LEGACY_CREDENTIAL_FILENAME)),
        }
    }
}

#[cfg(windows)]
fn credential_directory() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|app_data| PathBuf::from(app_data).join("Superiority"))
}

#[cfg(not(windows))]
fn credential_directory() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support/Superiority"))
}

fn create_private_directory(path: &Path) -> Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder
        .create(path)
        .map_err(|error| cache_error("create directory", &error))?;
    secure_directory(path)
}

#[cfg(unix)]
fn secure_file(path: &Path) -> Result<()> {
    fs::set_permissions(path, Permissions::from_mode(0o600))
        .map_err(|error| cache_error("secure", &error))
}

#[cfg(windows)]
#[expect(clippy::unnecessary_wraps, reason = "the other platform's twin fails")]
fn secure_file(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn secure_directory(path: &Path) -> Result<()> {
    fs::set_permissions(path, Permissions::from_mode(0o700))
        .map_err(|error| cache_error("secure directory", &error))
}

#[cfg(windows)]
#[expect(clippy::unnecessary_wraps, reason = "the other platform's twin fails")]
fn secure_directory(_path: &Path) -> Result<()> {
    Ok(())
}

fn cache_error(operation: &str, error: &std::io::Error) -> Error {
    Error::Authentication(format!("credential cache {operation} failed: {error}"))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn stores_beside_the_working_directory_when_the_path_is_a_bare_filename() {
        let directory = std::env::temp_dir().join(format!(
            "superiority-relative-test-{}-{:x}",
            std::process::id(),
            rand::random::<u64>()
        ));
        fs::create_dir_all(&directory).expect("test directory is created");
        let restore = std::env::current_dir().expect("a working directory exists");
        std::env::set_current_dir(&directory).expect("test directory is usable");

        let store = FileCredentialStore::at(PathBuf::from("bare-credentials.bin"));
        let credential = SecretBytes::new(vec![9, 9, 9]).expect("fixture is valid");
        let stored = store.store(&credential);
        let loaded = store.load();

        std::env::set_current_dir(restore).expect("working directory is restored");
        stored.expect("a bare filename stores into the working directory");
        assert_eq!(loaded.expect("credential is loaded").unwrap(), credential);
        fs::remove_dir_all(&directory).expect("test directory is removed");
    }

    #[test]
    fn each_product_caches_its_own_credential() {
        // GenerateWebCredentials is issued per program, so a credential signed
        // in as StarCraft II is not one signed in as Remastered; sharing one
        // file meant two sessions overwriting each other
        assert_ne!(
            credential_filename(Product::StarCraft2),
            credential_filename(Product::Remastered)
        );
        assert!(credential_filename(Product::StarCraft2).contains("S2"));
    }

    #[test]
    fn a_credential_written_before_the_split_is_still_found() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must follow the unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "superiority-credential-legacy-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temp directory is created");
        let credential = SecretBytes::new(vec![9, 8, 7]).expect("fixture is valid");

        // what the cache was called when there was only one product
        let legacy = directory.join(LEGACY_CREDENTIAL_FILENAME);
        FileCredentialStore::at(legacy.clone())
            .store(&credential)
            .expect("legacy credential is written");

        let store = FileCredentialStore {
            path: Some(directory.join(credential_filename(Product::StarCraft2))),
            legacy: Some(legacy),
        };
        // found without a fresh sign-in
        assert_eq!(
            store.load().expect("legacy credential is read").unwrap(),
            credential
        );

        // and a product that never had a legacy file does not borrow one
        let remastered = FileCredentialStore {
            path: Some(directory.join(credential_filename(Product::Remastered))),
            legacy: None,
        };
        assert!(remastered.load().expect("absent cache is valid").is_none());

        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn stores_loads_and_deletes_private_credentials() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must follow the unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "superiority-credential-test-{}-{nonce}",
            std::process::id()
        ));
        let path = directory.join(credential_filename(Product::StarCraft2));
        let store = FileCredentialStore::at(path.clone());
        let credential = SecretBytes::new(vec![1, 2, 3, 4]).expect("fixture is valid");

        assert!(store.load().expect("missing cache is valid").is_none());
        store.store(&credential).expect("credential is stored");
        assert_eq!(
            store.load().expect("credential is loaded").unwrap(),
            credential
        );
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&path)
                .expect("credential metadata exists")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        store.delete().expect("credential is deleted");
        assert!(store.load().expect("deleted cache is valid").is_none());
        fs::remove_dir(directory).expect("test directory is empty");
    }
}
