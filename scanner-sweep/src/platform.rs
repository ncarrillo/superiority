use std::{
    ffi::{CStr, c_int, c_void},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStrExt;

use crate::{Error, Result};

#[cfg(target_os = "macos")]
#[link(name = "proc")]
unsafe extern "C" {
    fn proc_listallpids(buffer: *mut c_void, buffer_size: c_int) -> c_int;
    fn proc_name(pid: c_int, buffer: *mut c_void, buffer_size: u32) -> c_int;
    fn proc_pidpath(pid: c_int, buffer: *mut c_void, buffer_size: u32) -> c_int;
}

#[cfg(target_os = "macos")]
pub fn find_sc2() -> Result<Option<i32>> {
    let count = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
    if count < 0 {
        return Err(Error::Platform(
            "could not enumerate macOS processes".to_owned(),
        ));
    }
    let capacity = usize::try_from(count)
        .unwrap_or_default()
        .saturating_add(64);
    let mut processes = vec![0_i32; capacity];
    let bytes = processes
        .len()
        .checked_mul(std::mem::size_of::<i32>())
        .and_then(|size| i32::try_from(size).ok())
        .ok_or_else(|| Error::Platform("process buffer is too large".to_owned()))?;
    let filled = unsafe { proc_listallpids(processes.as_mut_ptr().cast(), bytes) };
    if filled < 0 {
        return Err(Error::Platform(
            "could not read the macOS process list".to_owned(),
        ));
    }
    processes.truncate(usize::try_from(filled).unwrap_or_default());
    for pid in processes.into_iter().filter(|pid| *pid > 0) {
        let mut name = [0_i8; 1024];
        let length = unsafe {
            proc_name(
                pid,
                name.as_mut_ptr().cast(),
                u32::try_from(name.len()).expect("the process-name buffer fits u32"),
            )
        };
        if length > 0
            && unsafe { CStr::from_ptr(name.as_ptr()) }.to_bytes() == b"SC2"
            && is_sc2_executable(pid)
        {
            return Ok(Some(pid));
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
fn is_sc2_executable(pid: i32) -> bool {
    process_executable_path(pid)
        .is_some_and(|path| is_sc2_executable_path(path.as_os_str().as_bytes()))
}

#[cfg(target_os = "macos")]
fn process_executable_path(pid: i32) -> Option<std::path::PathBuf> {
    let mut path = [0_i8; 4096];
    let length = unsafe {
        proc_pidpath(
            pid,
            path.as_mut_ptr().cast(),
            u32::try_from(path.len()).expect("the process-path buffer fits u32"),
        )
    };
    (length > 0).then(|| {
        std::path::Path::new(std::ffi::OsStr::from_bytes(
            unsafe { CStr::from_ptr(path.as_ptr()) }.to_bytes(),
        ))
        .to_path_buf()
    })
}

#[cfg(target_os = "macos")]
fn sc2_executable_path(pid: i32) -> Result<std::path::PathBuf> {
    let path = process_executable_path(pid)
        .ok_or_else(|| Error::Platform(format!("could not resolve SC2 PID {pid}")))?;
    if !is_sc2_executable_path(path.as_os_str().as_bytes()) {
        return Err(Error::Platform(format!(
            "PID {pid} is not the SC2 application executable"
        )));
    }
    Ok(path)
}

#[cfg(target_os = "macos")]
fn is_sc2_executable_path(path: &[u8]) -> bool {
    path.ends_with(b"/SC2.app/Contents/MacOS/SC2")
}

#[cfg(target_os = "macos")]
pub fn wait_for_sc2(stop: &Arc<AtomicBool>) -> Result<i32> {
    while !stop.load(Ordering::Relaxed) {
        if let Some(pid) = find_sc2()? {
            return Ok(pid);
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    Err(Error::Platform("SC2 wait was cancelled".to_owned()))
}

#[cfg(target_os = "macos")]
pub fn installed_sc2_executable() -> Result<std::path::PathBuf> {
    let versions = std::path::Path::new("/Applications/StarCraft II/Versions");
    let mut executables = std::fs::read_dir(versions)?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path().join("SC2.app/Contents/MacOS/SC2"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    executables.sort();
    executables.pop().ok_or_else(|| {
        Error::Platform(format!(
            "could not find an installed SC2 executable below {}",
            versions.display()
        ))
    })
}

#[cfg(target_os = "macos")]
pub fn request_sc2_quit(pid: i32) -> Result<()> {
    use objc2_app_kit::NSRunningApplication;

    let application = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        .ok_or_else(|| Error::Platform(format!("SC2 PID {pid} disappeared")))?;
    if application.terminate() {
        Ok(())
    } else {
        Err(Error::Platform(
            "macOS did not accept SC2's normal quit request".to_owned(),
        ))
    }
}

#[cfg(target_os = "macos")]
pub fn launch_sc2() -> Result<()> {
    use objc2_app_kit::{NSWorkspace, NSWorkspaceOpenConfiguration};
    use objc2_foundation::{NSArray, NSString, NSURL};

    let application_path = "/Applications/Battle.net.app";
    if !std::path::Path::new(application_path).is_dir() {
        return Err(Error::Platform(format!(
            "Battle.net is not installed at {application_path}"
        )));
    }
    let path = NSString::from_str(application_path);
    let url = NSURL::fileURLWithPath_isDirectory(&path, true);
    let argument = NSString::from_str("--exec=launch S2");
    let arguments = NSArray::from_slice(&[&*argument]);
    let configuration = NSWorkspaceOpenConfiguration::configuration();
    configuration.setArguments(&arguments);
    configuration.setActivates(true);
    configuration.setCreatesNewApplicationInstance(true);
    NSWorkspace::sharedWorkspace().openApplicationAtURL_configuration_completionHandler(
        &url,
        &configuration,
        None,
    );
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn terminate_sc2(pid: i32, force: bool) -> Result<()> {
    if !is_sc2_executable(pid) {
        return Err(Error::Platform(format!(
            "refusing to terminate PID {pid} because it is not SC2"
        )));
    }
    let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
    if unsafe { libc::kill(pid, signal) } != 0 {
        return Err(Error::Platform(format!(
            "could not terminate SC2 PID {pid}: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::is_sc2_executable_path;

    #[test]
    fn recognizes_only_the_sc2_app_executable() {
        assert!(is_sc2_executable_path(
            b"/Applications/StarCraft II/Versions/Base97563/SC2.app/Contents/MacOS/SC2"
        ));
        assert!(!is_sc2_executable_path(b"/tmp/SC2"));
        assert!(!is_sc2_executable_path(
            b"/Applications/Other.app/Contents/MacOS/SC2"
        ));
    }
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub fn capture_game_utilities(pid: i32, output: &std::path::Path) -> Result<Vec<u8>> {
    lldb::capture(pid, &sc2_executable_path(pid)?, output)
}

#[cfg(all(target_os = "macos", not(target_arch = "x86_64")))]
pub fn capture_game_utilities(_pid: i32, _output: &std::path::Path) -> Result<Vec<u8>> {
    Err(Error::Platform(
        "live capture requires the x86_64 scanner-sweep binary because SC2 runs under Rosetta; build with --target x86_64-apple-darwin"
            .to_owned(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn find_sc2() -> Result<Option<i32>> {
    Err(Error::Platform(
        "live scanner-sweep capture is currently macOS-only".to_owned(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn wait_for_sc2(_stop: &Arc<AtomicBool>) -> Result<i32> {
    Err(Error::Platform(
        "live scanner-sweep capture is currently macOS-only".to_owned(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn request_sc2_quit(_pid: i32) -> Result<()> {
    Err(Error::Platform(
        "closing SC2 is currently macOS-only".to_owned(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn launch_sc2() -> Result<()> {
    Err(Error::Platform(
        "launching SC2 is currently macOS-only".to_owned(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn capture_game_utilities(_pid: i32, _output: &std::path::Path) -> Result<Vec<u8>> {
    Err(Error::Platform(
        "live scanner-sweep capture is currently macOS-only".to_owned(),
    ))
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
mod lldb;
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
mod macho;
