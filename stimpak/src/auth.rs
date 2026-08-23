//! sign-in handled inside the library.
//!
//! the app embeds its webview in a window it already owns; a library cannot,
//! because the event loop belongs on the main thread and the main thread
//! belongs to the host. so the window runs as a child process and the token
//! comes back on its stdout.
//!
//! locating that helper is deliberate rather than magical: an explicit path
//! first, then `STIMPAK_AUTH_WINDOW`, then a sibling of the running executable.
//! when none of those find it the event is handed to the host instead, so an
//! embedder that would rather drive its own browser still can.

use std::{
    io::Write as _,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
    thread,
    time::Duration,
};

const HELPER: &str = if cfg!(windows) {
    "stimpak-auth-window.exe"
} else {
    "stimpak-auth-window"
};

pub struct AuthWindow {
    /// resolved once at construction: this is a filesystem probe, and a host
    /// asking repeatedly whether sign-in is available should not pay for it.
    helper: Option<PathBuf>,
    /// the child stays reachable while its watcher waits, so disconnect, sign
    /// out, and disposal can stop it instead of leaving a login window behind.
    child: Arc<Mutex<Option<Child>>>,
}

impl AuthWindow {
    pub fn new(explicit: Option<PathBuf>) -> Self {
        Self {
            helper: Self::candidates(explicit)
                .into_iter()
                .find(|path| path.is_file()),
            child: Arc::new(Mutex::new(None)),
        }
    }

    /// the helper this client will use, or none — in which case the host is
    /// asked to complete the sign-in itself.
    pub fn locate(&self) -> Option<PathBuf> {
        self.helper.clone()
    }

    fn candidates(explicit: Option<PathBuf>) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        if let Some(explicit) = explicit {
            candidates.push(explicit);
        }
        if let Some(from_environment) = std::env::var_os("STIMPAK_AUTH_WINDOW") {
            candidates.push(PathBuf::from(from_environment));
        }
        if let Ok(executable) = std::env::current_exe()
            && let Some(directory) = executable.parent()
        {
            candidates.push(directory.join(HELPER));
        }
        candidates
    }

    /// starts the window and returns its eventual result without parking the
    /// client's polling thread. `Ok(None)` means the person closed it without
    /// finishing.
    /// the url goes over stdin, not argv: argv is readable by any local user
    /// through `ps`, and a battle.net sign-in url is not something to publish
    /// to everyone on the machine.
    pub fn present(
        &self,
        helper: &Path,
        url: &str,
    ) -> Result<Receiver<Result<Option<String>, String>>, String> {
        self.cancel();
        let mut child = Command::new(helper)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("could not start {}: {error}", helper.display()))?;
        let handoff = child
            .stdin
            .take()
            .ok_or_else(|| "sign-in window has no stdin".to_owned())
            .and_then(|mut stdin| {
                stdin
                    .write_all(url.as_bytes())
                    .map_err(|error| format!("could not hand over the sign-in url: {error}"))
            });
        if let Err(error) = handoff {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        let Ok(mut active) = self.child.lock() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("sign-in window state is unavailable".to_owned());
        };
        *active = Some(child);
        drop(active);

        let (completion, result) = mpsc::channel();
        let active = Arc::clone(&self.child);
        let spawn = thread::Builder::new()
            .name("stimpak-auth-window".into())
            .spawn(move || watch(&active, &completion));
        if let Err(error) = spawn {
            self.cancel();
            return Err(format!("could not watch the sign-in window: {error}"));
        }
        Ok(result)
    }

    /// stops an authentication helper, if one is still running.
    pub fn cancel(&self) {
        let Ok(mut active) = self.child.lock() else {
            return;
        };
        let Some(mut child) = active.take() else {
            return;
        };
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for AuthWindow {
    fn drop(&mut self) {
        self.cancel();
    }
}

enum ChildState {
    Running,
    Finished(Child),
    Failed(String),
    Cancelled,
}

fn watch(
    active: &Arc<Mutex<Option<Child>>>,
    completion: &mpsc::Sender<Result<Option<String>, String>>,
) {
    loop {
        let state = match active.lock() {
            Ok(mut active) => match active.as_mut() {
                None => ChildState::Cancelled,
                Some(child) => match child.try_wait() {
                    Ok(Some(_)) => active
                        .take()
                        .map_or(ChildState::Cancelled, ChildState::Finished),
                    Ok(None) => ChildState::Running,
                    Err(error) => {
                        let _ = active.take();
                        ChildState::Failed(format!("sign-in window failed: {error}"))
                    }
                },
            },
            Err(_) => ChildState::Failed("sign-in window state is unavailable".to_owned()),
        };
        match state {
            ChildState::Running => thread::sleep(Duration::from_millis(25)),
            ChildState::Cancelled => return,
            ChildState::Failed(error) => {
                let _ = completion.send(Err(error));
                return;
            }
            ChildState::Finished(child) => {
                let outcome = child
                    .wait_with_output()
                    .map_err(|error| format!("sign-in window failed: {error}"))
                    .and_then(|output| parse_output(&output));
                let _ = completion.send(outcome);
                return;
            }
        }
    }
}

fn parse_output(output: &Output) -> Result<Option<String>, String> {
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return if detail.is_empty() {
            Ok(None)
        } else {
            Err(detail.to_owned())
        };
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Ok((!token.is_empty()).then_some(token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_path_is_preferred_over_everything_else() {
        let candidates =
            AuthWindow::candidates(Some(PathBuf::from("/somewhere/stimpak-auth-window")));
        assert_eq!(
            candidates.first(),
            Some(&PathBuf::from("/somewhere/stimpak-auth-window"))
        );
    }

    #[test]
    fn a_path_that_does_not_exist_is_never_returned() {
        let missing = PathBuf::from("/definitely/not/here");
        let window = AuthWindow::new(Some(missing.clone()));
        assert_ne!(window.locate(), Some(missing));
    }

    #[test]
    fn the_executable_directory_is_always_a_candidate() {
        let candidates = AuthWindow::candidates(None);
        assert!(candidates.iter().any(|path| path.ends_with(HELPER)));
    }
}
