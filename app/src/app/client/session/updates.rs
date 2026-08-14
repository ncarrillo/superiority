use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn poll_update_events(&mut self, cx: &mut Context<Self>) -> bool {
        let mut changed = false;
        if self.updates.startup_update_check_pending
            && self.updates.startup_update_check_started.is_none()
        {
            self.updates.startup_update_check_started = Some(Instant::now());
            if let Some(service) = &self.updates.update_service {
                Self::trace("starting startup update check");
                service.check();
            }
        }
        let events = self
            .updates
            .update_events
            .as_ref()
            .map(|events| events.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut quit_requested = false;
        for event in events {
            Self::trace(format_args!("update event: {event}"));
            changed = true;
            quit_requested |= crate::app::update::update_requests_quit(&event);
            let startup_disposition = startup_check_disposition(&event);
            let previous_notes = self.updates.update_model.notes.clone();
            let should_present = self.updates.update_model.apply_event(&event);
            if self.updates.update_model.notes != previous_notes {
                self.updates.update_notes_selection.clear();
            }
            if startup_disposition != StartupCheckDisposition::Waiting {
                self.updates.manual_update_check_deadline = None;
            }
            if self.updates.startup_update_check_pending {
                match startup_disposition {
                    StartupCheckDisposition::Waiting => {}
                    StartupCheckDisposition::UpdateAvailable => {
                        self.updates.startup_update_check_pending = false;
                        self.updates.startup_update_check_started = None;
                        self.updates.update_dialog_visible = true;
                        self.updates.update_dialog_closing = false;
                        self.updates.update_hide_due = None;
                    }
                    StartupCheckDisposition::Continue => {
                        self.updates.startup_update_check_pending = false;
                        self.updates.startup_update_check_started = None;
                        self.begin_startup_connection(cx);
                    }
                }
            } else if should_present {
                self.updates.update_dialog_visible = true;
                self.updates.update_dialog_closing = false;
                self.updates.update_hide_due = None;
            } else if self.updates.update_dialog_visible
                && !self.updates.update_dialog_closing
                && !matches!(
                    self.updates.update_model.stage,
                    UpdateStage::Current | UpdateStage::Error
                )
            {
                self.updates.update_dialog_closing = true;
                self.updates.update_hide_due = Some(Instant::now() + MODAL_CLOSE_DURATION);
            }
        }
        if quit_requested {
            cx.quit();
            return true;
        }
        if self.updates.startup_update_check_pending
            && self
                .updates
                .startup_update_check_started
                .is_some_and(|started| {
                    Instant::now().saturating_duration_since(started) >= STARTUP_UPDATE_TIMEOUT
                })
        {
            Self::trace("startup update check timed out; continuing startup");
            self.updates.startup_update_check_pending = false;
            self.updates.startup_update_check_started = None;
            if self.updates.update_dialog_visible {
                self.updates
                    .update_model
                    .show_unavailable("The update service did not respond. Please try again.");
                self.updates.update_notes_selection.clear();
            }
            self.begin_startup_connection(cx);
            changed = true;
        }
        if self
            .updates
            .manual_update_check_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            Self::trace("manual update check timed out");
            self.updates.manual_update_check_deadline = None;
            self.updates
                .update_model
                .show_unavailable("The update service did not respond. Please try again.");
            self.updates.update_notes_selection.clear();
            self.updates.update_dialog_visible = true;
            self.updates.update_dialog_closing = false;
            self.updates.update_hide_due = None;
            changed = true;
        }
        changed
    }

    pub(in crate::app::client) fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        Self::trace(format_args!(
            "manual update check requested: warning={} connection={} update={} overlay={}",
            self.warnings.warning_dialog.is_some(),
            self.connection.dialog_visible,
            self.updates.update_dialog_visible,
            self.overlays.active.is_some(),
        ));
        if self.updates.update_dialog_visible {
            return;
        }
        let Some(service) = &self.updates.update_service else {
            self.show_channel_warning(
                "Software update",
                "Automatic updates are available in packaged application builds.",
                None,
            );
            cx.notify();
            return;
        };
        self.updates.update_model.begin_check();
        self.updates.update_notes_selection.clear();
        self.updates.update_dialog_visible = true;
        self.updates.update_dialog_closing = false;
        self.updates.update_hide_due = None;
        if !self.updates.startup_update_check_pending {
            self.updates.manual_update_check_deadline =
                Some(Instant::now() + STARTUP_UPDATE_TIMEOUT);
            service.check();
        }
        cx.notify();
    }

    pub(in crate::app::client) fn perform_update_primary_action(&mut self, cx: &mut Context<Self>) {
        let Some(service) = &self.updates.update_service else {
            return;
        };
        match self.updates.update_model.primary_action() {
            UpdatePrimaryAction::Check => {
                self.updates.update_model.begin_check();
                self.updates.update_notes_selection.clear();
                self.updates.manual_update_check_deadline =
                    Some(Instant::now() + STARTUP_UPDATE_TIMEOUT);
                service.check();
            }
            UpdatePrimaryAction::Install => service.primary_action(),
            UpdatePrimaryAction::None => return,
        }
        cx.notify();
    }

    pub(in crate::app::client) fn close_update_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.updates.update_dialog_visible || self.updates.update_dialog_closing {
            return;
        }
        if let Some(service) = &self.updates.update_service {
            service.dismiss();
        }
        self.updates.manual_update_check_deadline = None;
        self.updates.update_dialog_closing = true;
        self.updates.update_hide_due = Some(Instant::now() + MODAL_CLOSE_DURATION);
        cx.notify();
    }
}
