use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn open_connection_dialog(&mut self) {
        self.overlays.active = None;
        self.overlays.closing = false;
        self.connection.dialog_visible = true;
        self.connection.dialog_closing = false;
        self.connection.close_due = None;
        self.connection.hide_due = None;
    }

    pub(in crate::app::client) fn set_connection_progress_stage(&mut self, stage: ConnectionStage) {
        let index = match stage {
            ConnectionStage::WebAuthentication => Some(0),
            ConnectionStage::GameUtilities => Some(1),
            ConnectionStage::NativeAuthentication => Some(2),
            ConnectionStage::ChatBootstrap => Some(3),
            ConnectionStage::Connected => Some(CONNECTION_STEPS),
            ConnectionStage::Disconnected => None,
        };
        let (floor, ceiling) = match index {
            None => (0.0, 0.0),
            Some(index) if index >= CONNECTION_STEPS => (1.0, 1.0),
            Some(index) => {
                let steps = CONNECTION_STEPS as f32;
                (index as f32 / steps, (index + 1) as f32 / steps)
            }
        };
        self.connection.floor = floor;
        self.connection.ceiling = ceiling;
        if index.is_none() || floor >= 1.0 {
            self.connection.fill = floor;
        }
        self.connection.progress_updated = Instant::now();
    }

    pub(in crate::app::client) fn show_disconnect_warning(&mut self, detail: String) {
        self.overlays.active = None;
        self.overlays.closing = false;
        self.connection.dialog_visible = false;
        self.connection.dialog_closing = false;
        self.connection.close_due = None;
        self.connection.hide_due = None;
        self.warnings.warning_dialog = Some(WarningDialog::Disconnected { detail });
        self.warnings.warning_closing = false;
        self.warnings.warning_hide_due = None;
    }

    pub(in crate::app::client) fn show_channel_warning(
        &mut self,
        title: impl Into<String>,
        detail: impl Into<String>,
        close_tab: Option<usize>,
    ) {
        self.warnings.warning_dialog = Some(WarningDialog::Channel {
            title: title.into().to_uppercase(),
            detail: detail.into(),
            close_tab,
        });
        self.warnings.warning_closing = false;
        self.warnings.warning_hide_due = None;
    }

    pub(in crate::app::client) fn open_settings(&mut self, cx: &mut Context<Self>) {
        if self.warnings.warning_dialog.is_some()
            || self.connection.dialog_visible
            || self.updates.update_dialog_visible
            || matches!(self.overlays.active, Some(Overlay::Friends | Overlay::Join))
        {
            return;
        }
        self.overlays.active = Some(Overlay::Settings);
        self.overlays.closing = false;
        self.settings.settings_tooltip = None;
        cx.notify();
    }

    pub(in crate::app::client) fn begin_warning_close(&mut self, cx: &mut Context<Self>) {
        if self.warnings.warning_dialog.is_none() || self.warnings.warning_closing {
            return;
        }
        self.warnings.warning_closing = true;
        self.warnings.warning_hide_due = Some(Instant::now() + MODAL_CLOSE_DURATION);
        cx.notify();
    }
}
