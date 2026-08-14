use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use crate::{Error, InstallPlan, Result};

pub(super) fn system_version() -> Option<String> {
    None
}

pub(super) fn default_paths(_application_id: &str) -> Result<(PathBuf, PathBuf, PathBuf)> {
    Err(Error::UnsupportedPlatform(std::env::consts::OS.into()))
}

pub(super) fn extract(
    _archive: &Path,
    _staging: &Path,
    _cancelled: &Arc<AtomicBool>,
    _progress: impl FnMut(f64),
) -> Result<PathBuf> {
    Err(Error::UnsupportedPlatform(std::env::consts::OS.into()))
}

pub(super) fn validate_install_source(_source: &Path, _application_id: &str) -> Result<()> {
    Err(Error::UnsupportedPlatform(std::env::consts::OS.into()))
}

pub(super) fn validate_install_target(_target: &Path, _application_id: &str) -> Result<()> {
    Err(Error::UnsupportedPlatform(std::env::consts::OS.into()))
}

pub(super) fn privileged_staging_directory(_application_id: &str, _nonce: &str) -> Result<PathBuf> {
    Err(Error::UnsupportedPlatform(std::env::consts::OS.into()))
}

pub(super) fn execute_install(_plan: &InstallPlan) -> Result<()> {
    Err(Error::UnsupportedPlatform(std::env::consts::OS.into()))
}

pub(super) fn installation_needs_authorization(_target: &Path) -> bool {
    false
}

pub(super) fn launch_elevated_worker(
    _plan_path: &Path,
    _result_path: &Path,
    _app_name: &str,
    _app_path: &Path,
) -> Result<()> {
    Err(Error::UnsupportedPlatform(std::env::consts::OS.into()))
}

pub(super) fn wait_for_process(_process_id: u32) -> Result<()> {
    Err(Error::UnsupportedPlatform(std::env::consts::OS.into()))
}

pub(super) fn relaunch(_path: &Path) -> Result<()> {
    Err(Error::UnsupportedPlatform(std::env::consts::OS.into()))
}

pub(super) fn show_progress(_app_name: &str, _app_path: &Path) -> Result<()> {
    Err(Error::UnsupportedPlatform(std::env::consts::OS.into()))
}
