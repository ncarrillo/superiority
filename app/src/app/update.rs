use std::sync::mpsc::Receiver;

use serde_json::Value;
use superiority_ui::components::release_notes::ReleaseNotesDocument;

#[cfg(feature = "rust-updater")]
const UPDATE_FEED_URL: &str = "https://superiority-sc2-updates.pages.dev/appcast.xml";
#[cfg(feature = "rust-updater")]
const UPDATE_PUBLIC_KEY: &str = "IVqqIejocXACpzUqr/W4FpT8qkuJidILS7UqPZ7x7xE=";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UpdatePrimaryAction {
    Check,
    Install,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupCheckDisposition {
    Waiting,
    UpdateAvailable,
    Continue,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum UpdateStage {
    Idle,
    Checking,
    Available,
    Downloading(f32),
    Extracting(f32),
    Ready,
    Installing,
    Current,
    Error,
}

#[derive(Clone, Debug)]
pub(crate) struct UpdateModel {
    pub stage: UpdateStage,
    pub headline: String,
    pub summary: String,
    pub notes: ReleaseNotesDocument,
}

impl Default for UpdateModel {
    fn default() -> Self {
        Self {
            stage: UpdateStage::Idle,
            headline: "Checking for updates…".to_owned(),
            summary: format!("Installed {}", env!("SUPERIORITY_EFFECTIVE_VERSION")),
            notes: ReleaseNotesDocument::plain(
                "Release notes will appear here when an update is found.",
            ),
        }
    }
}

impl UpdateModel {
    pub(crate) fn begin_check(&mut self) {
        self.stage = UpdateStage::Checking;
        self.headline = "Checking for updates…".to_owned();
        self.summary = format!("Installed {}", env!("SUPERIORITY_EFFECTIVE_VERSION"));
        self.notes = ReleaseNotesDocument::plain("Contacting the update service…");
    }

    pub(crate) fn show_unavailable(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.stage = UpdateStage::Error;
        self.headline = "Update check failed".to_owned();
        self.summary = "Superiority could not reach or validate the update service.".to_owned();
        self.notes = ReleaseNotesDocument::plain(&message);
    }

    pub(crate) fn apply_event(&mut self, json: &str) -> bool {
        let Ok(event) = serde_json::from_str::<Value>(json) else {
            self.show_unavailable("The update service returned an unreadable response.");
            return true;
        };
        let kind = event.get("kind").and_then(Value::as_str).unwrap_or("error");
        match kind {
            "checking" => self.begin_check(),
            "available" => {
                let version = event
                    .get("version")
                    .and_then(Value::as_str)
                    .unwrap_or("new");
                let title = event
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("Superiority update");
                let size = event.get("size").and_then(Value::as_u64).unwrap_or(0);
                self.headline = format!("Superiority {version} is available");
                self.summary = format!("{title}  •  {}", format_size(size));
                self.notes = ReleaseNotesDocument::parse(
                    event
                        .get("notes")
                        .and_then(Value::as_str)
                        .unwrap_or("Release notes are not available."),
                    event
                        .get("notes_format")
                        .and_then(Value::as_str)
                        .unwrap_or("plain-text"),
                );
                self.stage = UpdateStage::Available;
            }
            "notes" => {
                if let Some(notes) = event.get("notes").and_then(Value::as_str) {
                    self.notes = ReleaseNotesDocument::parse(
                        notes,
                        event
                            .get("notes_format")
                            .and_then(Value::as_str)
                            .unwrap_or("plain-text"),
                    );
                }
            }
            "downloading" => self.stage = UpdateStage::Downloading(progress(&event)),
            "extracting" => self.stage = UpdateStage::Extracting(progress(&event)),
            "ready" => self.stage = UpdateStage::Ready,
            "installing" => self.stage = UpdateStage::Installing,
            "installed" => self.stage = UpdateStage::Current,
            "not_found" => {
                self.headline = "Superiority is up to date".to_owned();
                self.summary = format!(
                    "Version {} is installed.",
                    env!("SUPERIORITY_EFFECTIVE_VERSION")
                );
                self.notes = ReleaseNotesDocument::plain(message(&event));
                self.stage = UpdateStage::Current;
            }
            "error" => self.show_unavailable(message(&event)),
            "dismissed" => return false,
            "focus" => return true,
            "quit_requested" => return false,
            _ => self.show_unavailable("The update service returned an unknown event."),
        }
        true
    }

    pub(crate) fn primary_action(&self) -> UpdatePrimaryAction {
        match self.stage {
            UpdateStage::Idle | UpdateStage::Current | UpdateStage::Error => {
                UpdatePrimaryAction::Check
            }
            UpdateStage::Available | UpdateStage::Ready => UpdatePrimaryAction::Install,
            UpdateStage::Checking
            | UpdateStage::Downloading(_)
            | UpdateStage::Extracting(_)
            | UpdateStage::Installing => UpdatePrimaryAction::None,
        }
    }

    pub(crate) fn status(&self) -> (&'static str, f32, &'static str, &'static str, bool) {
        match self.stage {
            UpdateStage::Idle | UpdateStage::Checking => (
                "Contacting the update service…",
                0.08,
                "CHECKING",
                "CANCEL",
                false,
            ),
            UpdateStage::Available => ("Ready to download", 0.0, "UPDATE NOW", "LATER", true),
            UpdateStage::Downloading(progress) => (
                "Downloading update…",
                progress,
                "DOWNLOADING",
                "CANCEL",
                false,
            ),
            UpdateStage::Extracting(progress) => (
                "Verifying and extracting…",
                progress,
                "VERIFYING",
                "LATER",
                false,
            ),
            UpdateStage::Ready => (
                "Ready to install and relaunch",
                1.0,
                "RELAUNCH",
                "LATER",
                true,
            ),
            UpdateStage::Installing => ("Installing…", 1.0, "INSTALLING", "LATER", false),
            UpdateStage::Current => ("No update is available", 1.0, "CHECK AGAIN", "CLOSE", true),
            UpdateStage::Error => (
                "Review the details above and try again.",
                1.0,
                "TRY AGAIN",
                "CLOSE",
                true,
            ),
        }
    }
}

pub(crate) fn startup_check_disposition(json: &str) -> StartupCheckDisposition {
    let kind = serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|event| event.get("kind").and_then(Value::as_str).map(str::to_owned));
    match kind.as_deref() {
        Some("checking" | "notes" | "focus") => StartupCheckDisposition::Waiting,
        Some("available") => StartupCheckDisposition::UpdateAvailable,
        _ => StartupCheckDisposition::Continue,
    }
}

pub(crate) fn update_requests_quit(json: &str) -> bool {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|event| event.get("kind").and_then(Value::as_str).map(str::to_owned))
        .as_deref()
        == Some("quit_requested")
}

fn progress(event: &Value) -> f32 {
    event
        .get("progress")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0) as f32
}

fn message(event: &Value) -> &str {
    event
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("The update could not be completed.")
}

fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "download size unavailable".to_owned();
    }
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
}

pub(crate) struct UpdateService {
    #[cfg(feature = "rust-updater")]
    client: superiority_updater::Client,
}

impl UpdateService {
    #[cfg(feature = "rust-updater")]
    pub(crate) fn start() -> Option<(Self, Receiver<String>)> {
        let feed_url = std::env::var("SUPERIORITY_UPDATE_FEED_URL")
            .unwrap_or_else(|_| UPDATE_FEED_URL.to_owned());
        let config = superiority_updater::Config::for_current_process(
            &feed_url,
            UPDATE_PUBLIC_KEY,
            env!("SUPERIORITY_EFFECTIVE_VERSION"),
            "Superiority",
            "com.superiority.sc2-chat",
        )
        .ok()?;
        let (client, updater_events) = superiority_updater::Client::start(config);
        let (events, receiver) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("superiority-updater-events".into())
            .spawn(move || {
                for event in updater_events {
                    let Ok(event) = event.to_json() else {
                        continue;
                    };
                    if events.send(event).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some((Self { client }, receiver))
    }

    #[cfg(not(feature = "rust-updater"))]
    pub(crate) fn start() -> Option<(Self, Receiver<String>)> {
        None
    }

    pub(crate) fn check(&self) {
        #[cfg(feature = "rust-updater")]
        self.client.check();
    }

    pub(crate) fn primary_action(&self) {
        #[cfg(feature = "rust-updater")]
        self.client.primary_action();
    }

    pub(crate) fn dismiss(&self) {
        #[cfg(feature = "rust-updater")]
        self.client.dismiss();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StartupCheckDisposition, UpdateModel, UpdatePrimaryAction, UpdateStage,
        startup_check_disposition, update_requests_quit,
    };

    #[test]
    fn startup_check_only_waits_for_non_terminal_events() {
        assert_eq!(
            startup_check_disposition(r#"{"kind":"checking"}"#),
            StartupCheckDisposition::Waiting
        );
        assert_eq!(
            startup_check_disposition(r#"{"kind":"available"}"#),
            StartupCheckDisposition::UpdateAvailable
        );
        assert_eq!(
            startup_check_disposition(r#"{"kind":"not_found"}"#),
            StartupCheckDisposition::Continue
        );
    }

    #[test]
    fn updater_agent_can_request_a_clean_application_exit() {
        assert!(update_requests_quit(r#"{"kind":"quit_requested"}"#));
        assert!(!update_requests_quit(r#"{"kind":"installing"}"#));
    }

    #[test]
    fn update_model_tracks_installation_events() {
        let mut model = UpdateModel::default();
        assert!(model.apply_event(
            r##"{"kind":"available","version":"2.0","title":"Update","notes":"# Fixed\n\n- Tabs","notes_format":"markdown","size":1048576}"##
        ));
        assert_eq!(model.stage, UpdateStage::Available);
        assert_eq!(model.primary_action(), UpdatePrimaryAction::Install);
        assert_eq!(model.notes.plain_text(), "Fixed\n• Tabs");
    }

    #[test]
    fn terminal_results_survive_a_housekeeping_dismissal() {
        let mut model = UpdateModel::default();
        assert!(model.apply_event(r#"{"kind":"not_found","message":"current"}"#));
        assert!(!model.apply_event(r#"{"kind":"dismissed"}"#));
        assert_eq!(model.stage, UpdateStage::Current);

        assert!(model.apply_event(r#"{"kind":"error","message":"offline"}"#));
        assert_eq!(model.stage, UpdateStage::Error);
    }

    #[test]
    fn markdown_release_notes_preserve_readable_copy() {
        let mut model = UpdateModel::default();
        model.apply_event(
            r##"{"kind":"available","notes":"# Update\n\n1. First\n2. Second","notes_format":"markdown"}"##,
        );
        assert_eq!(model.notes.plain_text(), "Update\n1. First\n2. Second");
    }
}
