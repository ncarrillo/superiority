use super::*;

mod connection;
mod model;
mod update;
mod warning;

pub(in crate::app::client) use model::WarningDialog;
use model::compact_error;

pub(in crate::app::client) const CONNECTION_STEPS: usize = 4;
pub(in crate::app::client) const CONNECTION_RAIL: f32 = 456.0;

pub(super) struct ConnectionComponent {
    pub(super) stage: ConnectionStage,
    pub(super) error: Option<String>,
    pub(super) starting: bool,
    pub(super) signed_out: bool,
    pub(super) sign_out_requested: bool,
    pub(super) dialog_visible: bool,
    pub(super) dialog_closing: bool,
    pub(super) close_due: Option<Instant>,
    pub(super) hide_due: Option<Instant>,
    pub(super) fill: f32,
    pub(super) floor: f32,
    pub(super) ceiling: f32,
    pub(super) progress_updated: Instant,
}

pub(super) struct WarningComponent {
    pub(super) warning_dialog: Option<WarningDialog>,
    pub(super) warning_closing: bool,
    pub(super) warning_hide_due: Option<Instant>,
}

pub(super) struct UpdateComponent {
    pub(super) update_service: Option<UpdateService>,
    pub(super) update_events: Option<Receiver<String>>,
    pub(super) update_model: UpdateModel,
    pub(super) update_notes_selection: ui_release_notes::ReleaseNotesSelection,
    pub(super) update_dialog_visible: bool,
    pub(super) update_dialog_closing: bool,
    pub(super) update_hide_due: Option<Instant>,
    pub(super) manual_update_check_deadline: Option<Instant>,
    pub(super) startup_update_check_pending: bool,
    pub(super) startup_update_check_started: Option<Instant>,
    pub(super) startup_connection_pending: bool,
}
