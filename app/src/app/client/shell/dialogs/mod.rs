use super::*;

mod connection;
mod model;
mod skin;
mod update;
mod warning;

pub(in crate::app::client) use model::WarningDialog;
use model::compact_error;
pub(in crate::app::client) use skin::DialogSkin;

pub(in crate::app::client) const CONNECTION_STEPS: usize = 4;
pub(in crate::app::client) const CONNECTION_RAIL: f32 = 456.0;

pub(in crate::app::client) struct ConnectionComponent {
    pub(in crate::app::client) stage: ConnectionStage,
    pub(in crate::app::client) error: Option<String>,
    pub(in crate::app::client) starting: bool,
    pub(in crate::app::client) signed_out: bool,
    pub(in crate::app::client) sign_out_requested: bool,
    pub(in crate::app::client) dialog_visible: bool,
    pub(in crate::app::client) dialog_closing: bool,
    pub(in crate::app::client) close_due: Option<Instant>,
    pub(in crate::app::client) hide_due: Option<Instant>,
    pub(in crate::app::client) fill: f32,
    pub(in crate::app::client) floor: f32,
    pub(in crate::app::client) ceiling: f32,
    pub(in crate::app::client) progress_updated: Instant,
}

pub(in crate::app::client) struct WarningComponent {
    pub(in crate::app::client) warning_dialog: Option<WarningDialog>,
    pub(in crate::app::client) warning_closing: bool,
    pub(in crate::app::client) warning_hide_due: Option<Instant>,
}

pub(in crate::app::client) struct UpdateComponent {
    pub(in crate::app::client) update_service: Option<UpdateService>,
    pub(in crate::app::client) update_events: Option<Receiver<String>>,
    pub(in crate::app::client) update_model: UpdateModel,
    pub(in crate::app::client) update_notes_selection: ui_release_notes::ReleaseNotesSelection,
    pub(in crate::app::client) update_notes_scroll: ScrollHandle,
    pub(in crate::app::client) update_dialog_visible: bool,
    pub(in crate::app::client) update_dialog_closing: bool,
    pub(in crate::app::client) update_hide_due: Option<Instant>,
    pub(in crate::app::client) manual_update_check_deadline: Option<Instant>,
    pub(in crate::app::client) startup_update_check_pending: bool,
    pub(in crate::app::client) startup_update_check_started: Option<Instant>,
    pub(in crate::app::client) startup_connection_pending: bool,
    /// `SUPERIORITY_PREVIEW_UPDATE`: the model holds a fixture and there is no
    /// service behind it, so Check for Updates… reopens the dialog instead of
    /// checking — the way to see it dressed for each realm.
    pub(in crate::app::client) preview_fixture: bool,
}
