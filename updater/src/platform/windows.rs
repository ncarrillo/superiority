use std::{
    ffi::c_void,
    fs::{self, File},
    io::{self, Write as _},
    os::windows::ffi::OsStrExt as _,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use serde::{Deserialize, Serialize};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM, LRESULT, WAIT_OBJECT_0, WPARAM},
        Graphics::Gdi::{COLOR_WINDOW, HBRUSH},
        Security::WinTrust::{
            WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
            WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY,
            WTD_UI_NONE, WTD_UICONTEXT_EXECUTE, WinVerifyTrust,
        },
        Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
        },
        UI::{
            Controls::{
                ICC_PROGRESS_CLASS, INITCOMMONCONTROLSEX, InitCommonControlsEx, PBM_SETMARQUEE,
                PBS_MARQUEE, PROGRESS_CLASSW,
            },
            Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::{
                CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW,
                GetMessageW, GetSystemMetrics, IDC_ARROW, LoadCursorW, MSG, RegisterClassExW,
                SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW, SendMessageW, SetForegroundWindow,
                ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY,
                WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_DISABLED, WS_POPUP, WS_VISIBLE,
            },
        },
    },
    core::PCWSTR,
};

use crate::{Error, InstallPlan, Result};

const CURRENT_MANIFEST: &str = "current.json";
const LAUNCHER_NAME: &str = "Superiority.exe";
const APPLICATION_NAME: &str = "superiority-app.exe";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CurrentVersion {
    schema: u32,
    version: String,
    executable: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_executable: Option<PathBuf>,
}

pub(super) fn system_version() -> Option<String> {
    None
}

pub(super) fn default_paths(application_id: &str) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let executable =
        std::env::current_exe().map_err(|error| Error::io("current executable", error))?;
    let root = installation_root(&executable)?;
    let cache = dirs::cache_dir()
        .ok_or_else(|| Error::Installation("Windows cache directory is unavailable".into()))?
        .join(application_id)
        .join("Updater");
    Ok((root.clone(), root.join(LAUNCHER_NAME), cache))
}

pub(super) fn extract(
    archive: &Path,
    staging: &Path,
    cancelled: &Arc<AtomicBool>,
    mut progress: impl FnMut(f64),
) -> Result<PathBuf> {
    fs::create_dir_all(staging).map_err(|error| Error::io(staging, error))?;
    let file = File::open(archive).map_err(|error| Error::io(archive, error))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|error| Error::InvalidArtifact(format!("open ZIP: {error}")))?;
    let total = (0..zip.len())
        .filter_map(|index| zip.by_index(index).ok().map(|entry| entry.size()))
        .sum::<u64>()
        .max(1);
    let mut written = 0_u64;
    progress(0.0);
    for index in 0..zip.len() {
        if cancelled.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        let mut entry = zip
            .by_index(index)
            .map_err(|error| Error::InvalidArtifact(format!("read ZIP entry: {error}")))?;
        let relative = entry.enclosed_name().ok_or_else(|| {
            Error::InvalidArtifact(format!("archive contains unsafe path {:?}", entry.name()))
        })?;
        if entry.is_symlink() {
            return Err(Error::InvalidArtifact(
                "Windows update archive contains a symbolic link".into(),
            ));
        }
        let destination = staging.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|error| Error::io(&destination, error))?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
        }
        let mut output =
            File::create(&destination).map_err(|error| Error::io(&destination, error))?;
        let copied =
            io::copy(&mut entry, &mut output).map_err(|error| Error::io(&destination, error))?;
        written = written.saturating_add(copied);
        progress(progress_fraction(written, total).clamp(0.0, 1.0));
    }
    progress(1.0);
    find_payload(staging)
}

pub(super) fn validate_install_source(source: &Path, _application_id: &str) -> Result<()> {
    let executable = source.join(APPLICATION_NAME);
    let launcher = source.join(LAUNCHER_NAME);
    let agent = source.join("superiority-updater-agent.exe");
    if !executable.is_file() || !launcher.is_file() || !agent.is_file() {
        return Err(Error::InvalidArtifact(
            "Windows update is missing its application, launcher, or updater agent".into(),
        ));
    }
    verify_authenticode(&executable)?;
    verify_authenticode(&launcher)?;
    verify_authenticode(&agent)
}

pub(super) fn validate_install_target(target: &Path, _application_id: &str) -> Result<()> {
    let launcher = target.join(LAUNCHER_NAME);
    let agent = target.join("superiority-updater-agent.exe");
    if !launcher.is_file() || !agent.is_file() || !target.join(CURRENT_MANIFEST).is_file() {
        return Err(Error::Installation(
            "the Windows installation is missing its launcher, updater agent, or current-version manifest".into(),
        ));
    }
    verify_authenticode(&launcher)?;
    verify_authenticode(&agent)
}

pub(super) fn privileged_staging_directory(application_id: &str, nonce: &str) -> Result<PathBuf> {
    let root = std::env::var_os("PROGRAMDATA")
        .map(PathBuf::from)
        .ok_or_else(|| Error::Installation("ProgramData is unavailable".into()))?
        .join(application_id)
        .join("Updater")
        .join(nonce);
    fs::create_dir_all(&root).map_err(|error| Error::io(&root, error))?;
    Ok(root)
}

pub(super) fn execute_install(plan: &InstallPlan) -> Result<()> {
    validate_install_source(&plan.install_source, &plan.application_id)?;
    validate_install_target(&plan.install_target, &plan.application_id)?;
    if !safe_version_component(&plan.build) {
        return Err(Error::Installation(
            "update build identifier is unsafe".into(),
        ));
    }
    let versions = plan.install_target.join("versions");
    fs::create_dir_all(&versions).map_err(|error| Error::io(&versions, error))?;
    let destination = versions.join(&plan.build);
    if destination.exists() {
        let metadata =
            fs::symlink_metadata(&destination).map_err(|error| Error::io(&destination, error))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(Error::Installation(
                "existing version destination is unsafe".into(),
            ));
        }
        fs::remove_dir_all(&destination).map_err(|error| Error::io(&destination, error))?;
    }
    move_or_copy_directory(&plan.install_source, &destination)?;

    // the running agent is a session copy, and the launcher exits as soon as
    // it starts the versioned app, so both stable binaries can be advanced
    // before the new version becomes current.
    replace_file_durably(
        &destination.join(LAUNCHER_NAME),
        &plan.install_target.join(LAUNCHER_NAME),
    )?;
    replace_file_durably(
        &destination.join("superiority-updater-agent.exe"),
        &plan.install_target.join("superiority-updater-agent.exe"),
    )?;

    let current_path = plan.install_target.join(CURRENT_MANIFEST);
    let previous = read_current_version(&current_path).ok();
    let manifest = CurrentVersion {
        schema: 1,
        version: plan.version.clone(),
        executable: Path::new("versions")
            .join(&plan.build)
            .join(APPLICATION_NAME),
        previous_executable: previous.map(|current| current.executable),
    };
    write_current_version(&current_path, &manifest)?;
    remove_obsolete_versions(
        &versions,
        &manifest.executable,
        manifest.previous_executable.as_deref(),
    );
    Ok(())
}

pub(super) fn installation_needs_authorization(target: &Path) -> bool {
    tempfile::Builder::new().tempfile_in(target).is_err()
}

pub(super) fn launch_elevated_worker(
    plan_path: &Path,
    result_path: &Path,
    _app_name: &str,
    _app_path: &Path,
) -> Result<()> {
    let worker = std::env::current_exe().map_err(|error| Error::io("updater agent", error))?;
    let parameters = format!(
        "--execute-plan {} --result {}",
        quote_windows_argument(plan_path.as_os_str()),
        quote_windows_argument(result_path.as_os_str())
    );
    let worker_wide = wide(&worker);
    let parameters_wide = wide(std::ffi::OsStr::new(&parameters));
    let mut execute = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(std::mem::size_of::<SHELLEXECUTEINFOW>())
            .expect("SHELLEXECUTEINFOW size fits in u32"),
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: windows::core::w!("runas"),
        lpFile: PCWSTR(worker_wide.as_ptr()),
        lpParameters: PCWSTR(parameters_wide.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // safety: all string buffers remain live for the call and cbsize is valid.
    unsafe { ShellExecuteExW(&raw mut execute) }.map_err(|error| {
        if error.code().0 == i32::from_win32(1223) {
            Error::AuthorizationCancelled
        } else {
            Error::Installation(format!("launch elevated Windows installer: {error}"))
        }
    })?;
    if !execute.hProcess.is_invalid() {
        // safety: hprocess is owned because see_mask_nocloseprocess was requested.
        let _ = unsafe { CloseHandle(execute.hProcess) };
    }
    Ok(())
}

pub(super) fn wait_for_process(process_id: u32) -> Result<()> {
    // safety: openprocess returns an owned handle; process_synchronize only permits waiting.
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, process_id) }
        .map_err(|error| Error::Installation(format!("open application process: {error}")))?;
    // safety: process is a valid handle returned by openprocess.
    let result = unsafe { WaitForSingleObject(process, u32::MAX) };
    // safety: process is no longer used after this close.
    let _ = unsafe { CloseHandle(process) };
    if result == WAIT_OBJECT_0 {
        Ok(())
    } else {
        Err(Error::Installation(format!(
            "wait for application process returned {result:?}"
        )))
    }
}

pub(super) fn relaunch(path: &Path) -> Result<()> {
    std::process::Command::new(path)
        .spawn()
        .map_err(|error| Error::io(path, error))?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
pub(super) fn show_progress(app_name: &str, _app_path: &Path) -> Result<()> {
    const WIDTH: i32 = 420;
    const HEIGHT: i32 = 150;
    let controls = INITCOMMONCONTROLSEX {
        dwSize: u32::try_from(std::mem::size_of::<INITCOMMONCONTROLSEX>())
            .expect("INITCOMMONCONTROLSEX size fits in u32"),
        dwICC: ICC_PROGRESS_CLASS,
    };
    // safety: controls has the required size and class flags.
    if !unsafe { InitCommonControlsEx(&raw const controls) }.as_bool() {
        return Err(Error::Installation(
            "initialize Windows progress control failed".into(),
        ));
    }
    // safety: querying the current module does not retain caller-owned data.
    let instance = unsafe { GetModuleHandleW(None) }
        .map_err(|error| Error::Installation(format!("get updater module: {error}")))?
        .into();
    let class = WNDCLASSEXW {
        cbSize: u32::try_from(std::mem::size_of::<WNDCLASSEXW>())
            .expect("WNDCLASSEXW size fits in u32"),
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(progress_window_proc),
        hInstance: instance,
        // safety: loading the system arrow cursor requires no owned resource.
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
        hbrBackground: HBRUSH(std::ptr::without_provenance_mut(
            usize::try_from(COLOR_WINDOW.0 + 1).unwrap_or_default(),
        )),
        lpszClassName: windows::core::w!("SuperiorityUpdaterProgress"),
        ..Default::default()
    };
    // safety: class fields remain valid for the call; duplicate registration
    // of this fixed class within the process is harmless.
    unsafe { RegisterClassExW(&raw const class) };
    let title = wide(std::ffi::OsStr::new(&format!("Updating {app_name}")));
    // safety: getsystemmetrics has no pointer preconditions.
    let (screen_width, screen_height) =
        unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
    // safety: the class was registered above and all optional parent/menu
    // handles are intentionally absent for this top-level status window.
    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            windows::core::w!("SuperiorityUpdaterProgress"),
            PCWSTR(title.as_ptr()),
            WS_POPUP | WS_CAPTION | WS_VISIBLE,
            (screen_width - WIDTH).max(0) / 2,
            (screen_height - HEIGHT).max(0) / 2,
            WIDTH,
            HEIGHT,
            None,
            None,
            Some(instance),
            None,
        )
    }
    .map_err(|error| Error::Installation(format!("create progress window: {error}")))?;
    let status = wide(std::ffi::OsStr::new("Installing update…"));
    // safety: child controls use the live top-level window and module handles.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            windows::core::w!("STATIC"),
            PCWSTR(status.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            28,
            24,
            364,
            22,
            Some(window),
            None,
            Some(instance),
            None,
        )
        .map_err(|error| Error::Installation(format!("create progress label: {error}")))?;
    }
    // safety: this creates a standard common-controls marquee progress child.
    let progress = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PROGRESS_CLASSW,
            windows::core::w!(""),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(PBS_MARQUEE),
            28,
            54,
            364,
            14,
            Some(window),
            None,
            Some(instance),
            None,
        )
    }
    .map_err(|error| Error::Installation(format!("create progress bar: {error}")))?;
    // safety: progress is a valid window and the message parameters have the expected types.
    unsafe {
        SendMessageW(progress, PBM_SETMARQUEE, Some(WPARAM(1)), Some(LPARAM(30)));
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            windows::core::w!("BUTTON"),
            windows::core::w!("Cancel Update"),
            WS_CHILD | WS_VISIBLE | WS_DISABLED,
            274,
            82,
            118,
            30,
            Some(window),
            None,
            Some(instance),
            None,
        )
        .map_err(|error| Error::Installation(format!("create progress button: {error}")))?;
        let _ = ShowWindow(window, SW_SHOW);
        let _ = SetForegroundWindow(window);
    }
    let mut message = MSG::default();
    // safety: message is writable storage and message-loop calls remain paired.
    while unsafe { GetMessageW(&raw mut message, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&raw const message);
            DispatchMessageW(&raw const message);
        }
    }
    Ok(())
}

unsafe extern "system" fn progress_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_DESTROY {
        return LRESULT(0);
    }
    // safety: unhandled messages are forwarded unchanged to the system
    // default window procedure.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

fn installation_root(executable: &Path) -> Result<PathBuf> {
    let parent = executable
        .parent()
        .ok_or_else(|| Error::Installation("current executable has no parent directory".into()))?;
    if parent
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name.eq_ignore_ascii_case("versions"))
    {
        return parent
            .parent()
            .and_then(Path::parent)
            .map(Path::to_owned)
            .ok_or_else(|| Error::Installation("versioned installation root is invalid".into()));
    }
    Ok(parent.to_owned())
}

fn find_payload(staging: &Path) -> Result<PathBuf> {
    for entry in walkdir::WalkDir::new(staging)
        .min_depth(1)
        .max_depth(4)
        .follow_links(false)
    {
        let entry = entry.map_err(|error| Error::InvalidArtifact(error.to_string()))?;
        if entry.file_type().is_file()
            && entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(APPLICATION_NAME)
        {
            return entry
                .path()
                .parent()
                .map(Path::to_owned)
                .ok_or_else(|| Error::ApplicationNotFound(staging.to_owned()));
        }
    }
    Err(Error::ApplicationNotFound(staging.to_owned()))
}

fn verify_authenticode(path: &Path) -> Result<()> {
    let path_wide = wide(path.as_os_str());
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: u32::try_from(std::mem::size_of::<WINTRUST_FILE_INFO>())
            .expect("WINTRUST_FILE_INFO size fits in u32"),
        pcwszFilePath: PCWSTR(path_wide.as_ptr()),
        hFile: HANDLE::default(),
        pgKnownSubject: std::ptr::null_mut(),
    };
    let mut data = WINTRUST_DATA {
        cbStruct: u32::try_from(std::mem::size_of::<WINTRUST_DATA>())
            .expect("WINTRUST_DATA size fits in u32"),
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &raw mut file,
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwUIContext: WTD_UICONTEXT_EXECUTE,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // safety: data and file follow the wintrust api and
    // their referenced buffers remain live until state is closed below.
    let status = unsafe {
        WinVerifyTrust(
            HWND::default(),
            &raw mut action,
            std::ptr::from_mut(&mut data).cast::<c_void>(),
        )
    };
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    // safety: this closes the state opened by the preceding verification call.
    let _ = unsafe {
        WinVerifyTrust(
            HWND::default(),
            &raw mut action,
            std::ptr::from_mut(&mut data).cast::<c_void>(),
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(Error::InvalidArtifact(format!(
            "{} does not have a valid Authenticode signature (0x{status:08x})",
            path.display()
        )))
    }
}

fn move_or_copy_directory(source: &Path, destination: &Path) -> Result<()> {
    if fs::rename(source, destination).is_ok() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|error| Error::Installation(error.to_string()))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| Error::Installation(error.to_string()))?;
        let target = destination.join(relative);
        if entry.file_type().is_symlink() {
            return Err(Error::InvalidArtifact(
                "Windows payload contains a symbolic link".into(),
            ));
        }
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(|error| Error::io(&target, error))?;
        } else {
            fs::copy(entry.path(), &target).map_err(|error| Error::io(&target, error))?;
        }
    }
    fs::remove_dir_all(source).map_err(|error| Error::io(source, error))?;
    Ok(())
}

fn read_current_version(path: &Path) -> Result<CurrentVersion> {
    let current: CurrentVersion =
        serde_json::from_slice(&fs::read(path).map_err(|error| Error::io(path, error))?)?;
    validate_relative_executable(&current.executable)?;
    Ok(current)
}

fn write_current_version(path: &Path, current: &CurrentVersion) -> Result<()> {
    validate_relative_executable(&current.executable)?;
    let temporary = path.with_extension("json.new");
    let mut file = File::create(&temporary).map_err(|error| Error::io(&temporary, error))?;
    file.write_all(serde_json::to_string_pretty(current)?.as_bytes())
        .map_err(|error| Error::io(&temporary, error))?;
    file.sync_all()
        .map_err(|error| Error::io(&temporary, error))?;
    if path.exists() {
        // safety: both paths are nul-terminated buffers and flags request an
        // atomic, durable replacement on the same volume.
        unsafe {
            MoveFileExW(
                PCWSTR(wide(&temporary).as_ptr()),
                PCWSTR(wide(path).as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }
        .map_err(|error| Error::Installation(format!("activate update: {error}")))?;
    } else {
        fs::rename(&temporary, path).map_err(|error| Error::io(path, error))?;
    }
    Ok(())
}

fn replace_file_durably(source: &Path, destination: &Path) -> Result<()> {
    // safety: both paths are nul-terminated buffers and flags request an
    // atomic, durable replacement on the same volume.
    unsafe {
        MoveFileExW(
            PCWSTR(wide(source).as_ptr()),
            PCWSTR(wide(destination).as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| {
        Error::Installation(format!(
            "replace stable update component {}: {error}",
            destination.display()
        ))
    })
}

fn remove_obsolete_versions(versions: &Path, current: &Path, previous: Option<&Path>) {
    let keep = |executable: &Path| {
        executable
            .components()
            .nth(1)
            .and_then(|component| match component {
                Component::Normal(value) => Some(value.to_owned()),
                _ => None,
            })
    };
    let current = keep(current);
    let previous = previous.and_then(keep);
    let Ok(entries) = fs::read_dir(versions) else {
        return;
    };
    for entry in entries.flatten() {
        if Some(entry.file_name()) != current && Some(entry.file_name()) != previous {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn validate_relative_executable(path: &Path) -> Result<()> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || path.extension().is_none_or(|extension| extension != "exe")
    {
        return Err(Error::Installation(
            "current-version executable path is unsafe".into(),
        ));
    }
    Ok(())
}

fn safe_version_component(version: &str) -> bool {
    !version.is_empty()
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn wide(path: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    path.as_ref().encode_wide().chain(Some(0)).collect()
}

fn quote_windows_argument(argument: &std::ffi::OsStr) -> String {
    let value = argument.to_string_lossy();
    let mut quoted = String::from("\"");
    let mut backslashes = 0_usize;
    for character in value.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                backslashes = 0;
                quoted.push(character);
            }
        }
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

#[allow(clippy::cast_precision_loss)]
fn progress_fraction(completed: u64, total: u64) -> f64 {
    completed as f64 / total as f64
}

trait HResultFromWin32 {
    fn from_win32(error: u32) -> Self;
}

impl HResultFromWin32 for i32 {
    fn from_win32(error: u32) -> Self {
        if error == 0 {
            0
        } else {
            ((error & 0x0000_ffff) | 0x8007_0000).cast_signed()
        }
    }
}
