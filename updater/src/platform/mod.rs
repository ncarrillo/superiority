use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use crate::{InstallPlan, Result};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as implementation;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use unsupported as implementation;
#[cfg(target_os = "windows")]
use windows as implementation;

pub(crate) fn system_version() -> Option<String> {
    implementation::system_version()
}

pub(crate) fn default_paths(application_id: &str) -> Result<(PathBuf, PathBuf, PathBuf)> {
    implementation::default_paths(application_id)
}

pub(crate) fn extract(
    archive: &Path,
    staging: &Path,
    cancelled: &Arc<AtomicBool>,
    progress: impl FnMut(f64),
) -> Result<PathBuf> {
    implementation::extract(archive, staging, cancelled, progress)
}

pub(crate) fn validate_install_source(source: &Path, application_id: &str) -> Result<()> {
    implementation::validate_install_source(source, application_id)
}

pub(crate) fn validate_install_target(target: &Path, application_id: &str) -> Result<()> {
    implementation::validate_install_target(target, application_id)
}

pub(crate) fn privileged_staging_directory(application_id: &str, nonce: &str) -> Result<PathBuf> {
    implementation::privileged_staging_directory(application_id, nonce)
}

pub(crate) fn execute_install(plan: &InstallPlan) -> Result<()> {
    implementation::execute_install(plan)
}

#[must_use]
pub fn installation_needs_authorization(target: &Path) -> bool {
    implementation::installation_needs_authorization(target)
}

pub fn launch_elevated_worker(
    plan_path: &Path,
    result_path: &Path,
    app_name: &str,
    app_path: &Path,
) -> Result<()> {
    implementation::launch_elevated_worker(plan_path, result_path, app_name, app_path)
}

pub fn wait_for_process(process_id: u32) -> Result<()> {
    implementation::wait_for_process(process_id)
}

pub fn relaunch(path: &Path) -> Result<()> {
    implementation::relaunch(path)
}

pub fn show_progress(app_name: &str, app_path: &Path) -> Result<()> {
    implementation::show_progress(app_name, app_path)
}
