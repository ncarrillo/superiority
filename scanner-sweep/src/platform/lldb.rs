use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

use super::{Error, Result};

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(120);
const DETACH_TIMEOUT: Duration = Duration::from_secs(5);
const CAPTURE_HOOK: &str = include_str!("../../lldb/capture_game_utilities.py");

pub(super) fn capture(pid: i32, executable: &Path, output: &Path) -> Result<Vec<u8>> {
    let analysis_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(".analysis");
    let analysis_image = analysis_directory.join("SC2");
    let hook = analysis_directory.join("capture_game_utilities.py");
    super::macho::prepare_analysis_image(executable, &analysis_image)?;
    install_capture_hook(&hook)?;
    let output = absolute_path(output)?;
    let debugger_log = output.with_extension("lldb.log");

    remove_if_present(&output)?;
    remove_if_present(&debugger_log)?;

    let log = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&debugger_log)?;
    let error_log = log.try_clone()?;

    let mut command = Command::new("/usr/bin/arch");
    command
        .args(["-x86_64", "/usr/bin/lldb"])
        .args(["-o", "settings set target.load-cwd-lldbinit false"])
        .args([
            "-o",
            &format!("target create {}", lldb_path(&analysis_image)),
        ])
        .args(["-o", &format!("process attach --pid {pid}")])
        .args(["-o", &format!("command script import {}", lldb_path(&hook))])
        .args(["-o", "continue"])
        .args(["-o", "process detach"])
        .args(["-o", "quit"])
        .env("SC2_GAME_UTILITIES_LOG", &output)
        .env("SC2_GAME_UTILITIES_STOP_AFTER_CAPTURE", "1")
        .env("SC2_GAME_UTILITIES_STOP_DELAY_SECONDS", "0")
        .env("SC2_GAME_UTILITIES_CAPTURE_REQUEST", "0")
        .env("SC2_GAME_UTILITIES_HARDWARE_RESPONSE_BREAKPOINT", "1")
        .env("SC2_GAME_UTILITIES_HARDWARE_RETURN_BREAKPOINT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(error_log));
    let mut child = command
        .spawn()
        .map_err(|error| Error::Platform(format!("could not launch Intel LLDB: {error}")))?;

    let (status, timed_out) = wait_for_debugger(&mut child, pid)?;
    let response = read_response(&output);
    let debugger_output = std::fs::read_to_string(&debugger_log).unwrap_or_default();

    if let Ok(response) = response {
        remove_if_present(&output)?;
        remove_if_present(&debugger_log)?;
        return Ok(response);
    }

    let _ = remove_if_present(&output);
    let _ = remove_if_present(&debugger_log);
    let reason = if timed_out {
        "timed out waiting for the GameUtilities response".to_owned()
    } else {
        format!("Intel LLDB exited with {status}")
    };
    let details = log_tail(&debugger_output, 40);
    Err(Error::Platform(if details.is_empty() {
        reason
    } else {
        format!("{reason}\n\nLLDB output:\n{details}")
    }))
}

fn wait_for_debugger(child: &mut std::process::Child, sc2_pid: i32) -> Result<(ExitStatus, bool)> {
    let deadline = Instant::now() + CAPTURE_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait()? {
            resume_sc2(sc2_pid);
            return Ok((status, false));
        }
        if Instant::now() >= deadline {
            stop_sc2(sc2_pid);
            let detach_deadline = Instant::now() + DETACH_TIMEOUT;
            while Instant::now() < detach_deadline {
                if let Some(status) = child.try_wait()? {
                    resume_sc2(sc2_pid);
                    return Ok((status, true));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            child.kill()?;
            let status = child.wait()?;
            resume_sc2(sc2_pid);
            return Ok((status, true));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn stop_sc2(pid: i32) {
    unsafe {
        libc::kill(pid, libc::SIGSTOP);
    }
}

fn resume_sc2(pid: i32) {
    unsafe {
        libc::kill(pid, libc::SIGCONT);
    }
}

fn read_response(path: &Path) -> Result<Vec<u8>> {
    let contents = std::fs::read_to_string(path).map_err(|error| {
        Error::Platform(format!(
            "LLDB did not produce a GameUtilities capture at {}: {error}",
            path.display()
        ))
    })?;
    for line in contents.lines() {
        let value: serde_json::Value = serde_json::from_str(line).map_err(|error| {
            Error::Platform(format!("LLDB wrote invalid capture JSON: {error}"))
        })?;
        if value.get("type").and_then(serde_json::Value::as_str)
            == Some("game_utilities_client_response")
        {
            let encoded = value
                .get("hex")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    Error::Platform("the GameUtilities response has no hex payload".to_owned())
                })?;
            return hex::decode(encoded).map_err(|error| {
                Error::Platform(format!(
                    "the GameUtilities response is not valid hex: {error}"
                ))
            });
        }
    }
    Err(Error::Platform(
        "LLDB stopped without capturing the GameUtilities response".to_owned(),
    ))
}

fn install_capture_hook(path: &Path) -> Result<()> {
    if std::fs::read_to_string(path).is_ok_and(|contents| contents == CAPTURE_HOOK) {
        return Ok(());
    }
    let temporary = path.with_extension("tmp");
    remove_if_present(&temporary)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(CAPTURE_HOOK.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temporary, path)?;
    Ok(())
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn lldb_path(path: &Path) -> String {
    let escaped = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn remove_if_present(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn log_tail(log: &str, lines: usize) -> String {
    let mut tail = log.lines().rev().take(lines).collect::<Vec<_>>();
    tail.reverse();
    tail.join("\n")
}
