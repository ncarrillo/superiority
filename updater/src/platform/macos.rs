use std::{
    fs::{self, File},
    io::Read as _,
    os::unix::{
        ffi::OsStrExt as _,
        fs::{MetadataExt as _, PermissionsExt as _},
        io::AsRawFd as _,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use crate::{Error, InstallPlan, Result};

unsafe extern "C" {
    fn superiority_updater_launch_privileged_worker(
        worker_path: *const libc::c_char,
        plan_path: *const libc::c_char,
        result_path: *const libc::c_char,
        job_label: *const libc::c_char,
        authorization_prompt: *const libc::c_char,
        application_path: *const libc::c_char,
    ) -> libc::c_int;
    fn superiority_updater_show_progress(
        application_name: *const libc::c_char,
        application_path: *const libc::c_char,
    );
}

pub(super) fn system_version() -> Option<String> {
    let output = Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub(super) fn default_paths(application_id: &str) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let executable =
        std::env::current_exe().map_err(|error| Error::io("current executable", error))?;
    let target = executable
        .ancestors()
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| {
            Error::Installation("current executable is not inside an app bundle".into())
        })?
        .to_owned();
    let relaunch = target.clone();
    let cache = dirs::cache_dir()
        .ok_or_else(|| Error::Installation("macOS cache directory is unavailable".into()))?
        .join(application_id)
        .join("Updater");
    Ok((target, relaunch, cache))
}

pub(super) fn extract(
    archive: &Path,
    staging: &Path,
    cancelled: &Arc<AtomicBool>,
    mut progress: impl FnMut(f64),
) -> Result<PathBuf> {
    validate_zip_paths(archive)?;
    fs::create_dir_all(staging).map_err(|error| Error::io(staging, error))?;
    let total = zip_uncompressed_size(archive)?.max(1);
    let mut child = Command::new("/usr/bin/ditto")
        .args(["-x", "-k"])
        .arg(archive)
        .arg(staging)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Error::io("/usr/bin/ditto", error))?;
    progress(0.0);
    loop {
        if cancelled.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Cancelled);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| Error::io("ditto", error))?
        {
            if !status.success() {
                let mut message = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = stderr.read_to_string(&mut message);
                }
                return Err(Error::InvalidArtifact(format!(
                    "ditto could not extract update: {}",
                    message.trim()
                )));
            }
            break;
        }
        let extracted = extracted_size(staging);
        progress(progress_fraction(extracted, total).clamp(0.0, 0.98));
        thread::sleep(Duration::from_millis(50));
    }
    progress(1.0);
    find_application(staging)
}

pub(super) fn validate_install_source(source: &Path, application_id: &str) -> Result<()> {
    let status = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(source)
        .status()
        .map_err(|error| Error::io("/usr/bin/codesign", error))?;
    if !status.success() {
        return Err(Error::InvalidArtifact(
            "the extracted application has an invalid code signature".into(),
        ));
    }
    let info = source.join("Contents/Info.plist");
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleIdentifier"])
        .arg(&info)
        .output()
        .map_err(|error| Error::io("/usr/libexec/PlistBuddy", error))?;
    let actual = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || actual != application_id {
        return Err(Error::InvalidArtifact(format!(
            "application identifier {actual:?} does not match {application_id:?}"
        )));
    }
    Ok(())
}

pub(super) fn validate_install_target(target: &Path, application_id: &str) -> Result<()> {
    validate_install_source(target, application_id)
}

pub(super) fn privileged_staging_directory(application_id: &str, nonce: &str) -> Result<PathBuf> {
    if application_id != "com.superiority.sc2-chat"
        || nonce.len() != 48
        || !nonce.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(Error::Installation(
            "privileged staging identity is invalid".into(),
        ));
    }
    let root = Path::new("/Library/Caches")
        .join(application_id)
        .join("Updater");
    fs::create_dir_all(&root).map_err(|error| Error::io(&root, error))?;
    if fs::symlink_metadata(&root)
        .map_err(|error| Error::io(&root, error))?
        .file_type()
        .is_symlink()
    {
        return Err(Error::Installation(
            "privileged staging root is a symbolic link".into(),
        ));
    }
    let directory = root.join(nonce);
    fs::create_dir(&directory).map_err(|error| Error::io(&directory, error))?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|error| Error::io(&directory, error))?;
    Ok(directory)
}

pub(super) fn execute_install(plan: &InstallPlan) -> Result<()> {
    validate_plan_paths(plan)?;
    if needs_authorization(&plan.install_target) {
        return Err(Error::Installation(
            "the application is not writable; privileged installation has not started".into(),
        ));
    }
    replace_bundle(&plan.install_source, &plan.install_target)
}

pub(super) fn installation_needs_authorization(target: &Path) -> bool {
    needs_authorization(target)
}

pub(super) fn launch_elevated_worker(
    plan_path: &Path,
    result_path: &Path,
    app_name: &str,
    app_path: &Path,
) -> Result<()> {
    let worker = std::env::current_exe().map_err(|error| Error::io("updater agent", error))?;
    let worker = path_c_string(&worker)?;
    let plan = path_c_string(plan_path)?;
    let result = path_c_string(result_path)?;
    let label = std::ffi::CString::new("com.superiority.sc2-chat.rust-updater")
        .map_err(|_| Error::Installation("launchd label contains a NUL byte".into()))?;
    let prompt = std::ffi::CString::new(format!("{app_name} wants permission to update."))
        .map_err(|_| Error::Installation("authorization prompt contains a NUL byte".into()))?;
    let app_path = path_c_string(app_path)?;
    // safety: all pointers reference live, nul-terminated strings for the
    // duration of this synchronous objective-c bridge call.
    let status = unsafe {
        superiority_updater_launch_privileged_worker(
            worker.as_ptr(),
            plan.as_ptr(),
            result.as_ptr(),
            label.as_ptr(),
            prompt.as_ptr(),
            app_path.as_ptr(),
        )
    };
    match status {
        0 => Ok(()),
        1 => Err(Error::AuthorizationCancelled),
        _ => Err(Error::Installation(
            "macOS could not launch the privileged installer".into(),
        )),
    }
}

pub(super) fn wait_for_process(process_id: u32) -> Result<()> {
    let process_id = i32::try_from(process_id)
        .map_err(|_| Error::Installation("process identifier is invalid".into()))?;
    loop {
        // safety: kill with signal zero does not modify the target process and
        // accepts any process identifier representable by pid_t.
        let result = unsafe { libc::kill(process_id, 0) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            return Err(Error::Installation(format!(
                "wait for application process: {error}"
            )));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub(super) fn relaunch(path: &Path) -> Result<()> {
    Command::new("/usr/bin/open")
        .arg("-n")
        .arg(path)
        .spawn()
        .map_err(|error| Error::io("/usr/bin/open", error))?;
    Ok(())
}

pub(super) fn show_progress(app_name: &str, app_path: &Path) -> Result<()> {
    let name = std::ffi::CString::new(app_name)
        .map_err(|_| Error::Installation("application name contains a NUL byte".into()))?;
    let path = path_c_string(app_path)?;
    // safety: both pointers remain valid for the blocking appkit run loop.
    unsafe { superiority_updater_show_progress(name.as_ptr(), path.as_ptr()) };
    Ok(())
}

fn validate_zip_paths(archive: &Path) -> Result<()> {
    let file = File::open(archive).map_err(|error| Error::io(archive, error))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|error| Error::InvalidArtifact(format!("open ZIP: {error}")))?;
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|error| Error::InvalidArtifact(format!("read ZIP entry: {error}")))?;
        if entry.enclosed_name().is_none() {
            return Err(Error::InvalidArtifact(format!(
                "archive contains unsafe path {:?}",
                entry.name()
            )));
        }
    }
    Ok(())
}

fn zip_uncompressed_size(archive: &Path) -> Result<u64> {
    let file = File::open(archive).map_err(|error| Error::io(archive, error))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|error| Error::InvalidArtifact(format!("open ZIP: {error}")))?;
    let mut total = 0_u64;
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|error| Error::InvalidArtifact(format!("read ZIP entry: {error}")))?;
        total = total.saturating_add(entry.size());
    }
    Ok(total)
}

fn extracted_size(directory: &Path) -> u64 {
    walkdir::WalkDir::new(directory)
        .follow_links(false)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(std::fs::Metadata::is_file)
        .map(|metadata| metadata.len())
        .sum()
}

fn find_application(staging: &Path) -> Result<PathBuf> {
    walkdir::WalkDir::new(staging)
        .min_depth(1)
        .max_depth(4)
        .follow_links(false)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .map(walkdir::DirEntry::into_path)
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| Error::ApplicationNotFound(staging.to_owned()))
}

fn validate_plan_paths(plan: &InstallPlan) -> Result<()> {
    let source = fs::canonicalize(&plan.install_source)
        .map_err(|error| Error::io(&plan.install_source, error))?;
    let staging = fs::canonicalize(&plan.staging_directory)
        .map_err(|error| Error::io(&plan.staging_directory, error))?;
    if !source.starts_with(&staging) || source.extension().is_none_or(|value| value != "app") {
        return Err(Error::Installation(
            "install source is outside the staging directory".into(),
        ));
    }
    if plan
        .install_target
        .extension()
        .is_none_or(|value| value != "app")
    {
        return Err(Error::Installation(
            "install target is not an app bundle".into(),
        ));
    }
    Ok(())
}

fn needs_authorization(target: &Path) -> bool {
    let parent = target.parent().unwrap_or(target);
    if !is_writable(target) || !is_writable(parent) {
        return true;
    }
    let Ok(temporary) = tempfile::Builder::new().tempfile_in(parent) else {
        return true;
    };
    let Ok(metadata) = fs::metadata(target) else {
        return true;
    };
    // safety: the descriptor belongs to a live temporary file, and uid/gid
    // are copied verbatim from the existing target's metadata.
    unsafe {
        libc::fchown(
            temporary.as_file().as_raw_fd(),
            metadata.uid(),
            metadata.gid(),
        ) != 0
    }
}

fn is_writable(path: &Path) -> bool {
    let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // safety: path is a valid, nul-terminated filesystem path and access does
    // not retain the pointer.
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}

fn path_c_string(path: &Path) -> Result<std::ffi::CString> {
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| Error::Installation("filesystem path contains a NUL byte".into()))
}

fn replace_bundle(source: &Path, target: &Path) -> Result<()> {
    let source_c = std::ffi::CString::new(source.as_os_str().as_bytes())
        .map_err(|_| Error::Installation("source path contains a NUL byte".into()))?;
    let target_c = std::ffi::CString::new(target.as_os_str().as_bytes())
        .map_err(|_| Error::Installation("target path contains a NUL byte".into()))?;
    // safety: both c strings are valid, nul-terminated filesystem paths. the
    // flags request an atomic swap and neither pointer outlives this call.
    let swapped =
        unsafe { libc::renamex_np(source_c.as_ptr(), target_c.as_ptr(), libc::RENAME_SWAP) } == 0;
    if swapped {
        return Ok(());
    }

    let backup = target.with_file_name(format!(
        ".{}.superiority-backup-{}",
        target.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    fs::rename(target, &backup).map_err(|error| Error::io(target, error))?;
    if let Err(error) = fs::rename(source, target) {
        let _ = fs::rename(&backup, target);
        return Err(Error::io(target, error));
    }
    fs::remove_dir_all(&backup).map_err(|error| Error::io(&backup, error))?;
    Ok(())
}

#[allow(clippy::cast_precision_loss)]
fn progress_fraction(completed: u64, total: u64) -> f64 {
    completed as f64 / total as f64
}
