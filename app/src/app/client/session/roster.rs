use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn queue_roster(&mut self, snapshot: RosterSnapshot) {
        let now = Instant::now();
        let batch_active = self.roster.roster_batch_started.is_some();
        let started = *self.roster.roster_batch_started.get_or_insert(now);
        self.roster
            .pending_rosters
            .insert(snapshot.channel_index, snapshot);
        self.roster.roster_debounce_window = if batch_active {
            self.roster
                .roster_debounce_window
                .mul_f32(1.55)
                .min(ROSTER_DEBOUNCE_MAX_WINDOW)
        } else {
            ROSTER_DEBOUNCE_BASE
        };
        let remaining = ROSTER_DEBOUNCE_MAX_LATENCY
            .saturating_sub(now.saturating_duration_since(started))
            .max(Duration::from_millis(1));
        self.roster.roster_flush_at = Some(now + self.roster.roster_debounce_window.min(remaining));
    }

    pub(in crate::app::client) fn flush_rosters_if_due(&mut self) -> bool {
        let now = Instant::now();
        if self
            .roster
            .roster_flush_at
            .is_none_or(|flush_at| now < flush_at)
        {
            return false;
        }
        if self.roster.roster_hovered {
            let started = *self.roster.roster_defer_started.get_or_insert(now);
            if now.saturating_duration_since(started) < ROSTER_HOVER_DEFER_MAX {
                self.roster.roster_flush_at = Some(now + ROSTER_HOVER_RECHECK);
                return false;
            }
        }
        self.roster.roster_defer_started = None;
        self.roster.roster_batch_started = None;
        self.roster.roster_flush_at = None;
        self.roster.roster_debounce_window = ROSTER_DEBOUNCE_BASE;
        let snapshots = std::mem::take(&mut self.roster.pending_rosters);
        let mut changed = false;
        for snapshot in snapshots.into_values() {
            changed |= self.apply_roster_snapshot(snapshot);
        }
        changed
    }

    pub(in crate::app::client) fn roster_follows_bottom(&self) -> bool {
        let scroll = self.roster_base_scroll();
        let offset = -f32::from(scroll.offset().y);
        let maximum = f32::from(scroll.max_offset().y);
        maximum <= ROSTER_BOTTOM_TOLERANCE || (maximum - offset).abs() <= ROSTER_BOTTOM_TOLERANCE
    }

    pub(in crate::app::client) fn apply_roster_snapshot(
        &mut self,
        snapshot: RosterSnapshot,
    ) -> bool {
        let Some(position) = self
            .channels
            .tabs
            .iter()
            .position(|tab| tab.channel_index == Some(snapshot.channel_index))
        else {
            return false;
        };
        let follows_bottom = position == self.channels.active_tab && self.roster_follows_bottom();
        let next_users = snapshot
            .users
            .iter()
            .map(|user| UiUser::live(user, &mut self.roster.portraits))
            .collect::<Vec<_>>();
        let filter = self.roster.roster.filter.clone();
        let previous_users = self.channels.tabs[position].users.clone();
        let previous_visible = filtered_roster_users(&previous_users, &filter)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let next_visible = filtered_roster_users(&next_users, &filter)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let previous_handles = previous_visible
            .iter()
            .map(|user| user.handle)
            .collect::<Vec<_>>();
        let next_handles = next_visible
            .iter()
            .map(|user| user.handle)
            .collect::<Vec<_>>();
        self.channels.tabs[position].users = next_users;
        self.channels.tabs[position].roster_complete = snapshot.initial_complete;

        if position != self.channels.active_tab {
            return previous_handles != next_handles;
        }
        if self
            .selected_user()
            .is_some_and(|handle| !next_handles.contains(&handle))
        {
            self.set_selected_user(None);
        }
        if previous_handles != next_handles {
            self.begin_roster_animation(
                self.channels.tabs[position].id,
                previous_visible,
                &next_visible,
            );
        }
        if follows_bottom && next_handles.len() >= previous_handles.len() {
            if !next_handles.is_empty() {
                self.roster
                    .roster
                    .scroll
                    .scroll_to_item(next_handles.len() - 1, ScrollStrategy::Bottom);
            }
        }
        Self::trace(format_args!(
            "roster channel={} users={} visible={} follows_bottom={follows_bottom}",
            snapshot.channel_index,
            self.channels.tabs[position].users.len(),
            next_handles.len()
        ));
        previous_handles != next_handles || previous_users != self.channels.tabs[position].users
    }
}
