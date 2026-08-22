use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn queue_roster(&mut self, snapshot: RosterSnapshot) {
        let now = Instant::now();
        let batch_active = self.session.roster.roster_batch_started.is_some();
        let started = *self.session.roster.roster_batch_started.get_or_insert(now);
        self.session
            .roster
            .pending_rosters
            .insert(snapshot.channel_index, snapshot);
        self.session.roster.roster_debounce_window = if batch_active {
            self.session
                .roster
                .roster_debounce_window
                .mul_f32(1.55)
                .min(ROSTER_DEBOUNCE_MAX_WINDOW)
        } else {
            ROSTER_DEBOUNCE_BASE
        };
        let remaining = ROSTER_DEBOUNCE_MAX_LATENCY
            .saturating_sub(now.saturating_duration_since(started))
            .max(Duration::from_millis(1));
        self.session.roster.roster_flush_at =
            Some(now + self.session.roster.roster_debounce_window.min(remaining));
    }

    pub(in crate::app::client) fn flush_rosters_if_due(&mut self) -> bool {
        let now = Instant::now();
        if self
            .session
            .roster
            .roster_flush_at
            .is_none_or(|flush_at| now < flush_at)
        {
            return false;
        }
        if self.session.roster.roster_hovered {
            let started = *self.session.roster.roster_defer_started.get_or_insert(now);
            if now.saturating_duration_since(started) < ROSTER_HOVER_DEFER_MAX {
                self.session.roster.roster_flush_at = Some(now + ROSTER_HOVER_RECHECK);
                return false;
            }
        }
        self.session.roster.roster_defer_started = None;
        self.session.roster.roster_batch_started = None;
        self.session.roster.roster_flush_at = None;
        self.session.roster.roster_debounce_window = ROSTER_DEBOUNCE_BASE;
        let snapshots = std::mem::take(&mut self.session.roster.pending_rosters);
        let mut changed = false;
        for snapshot in snapshots.into_values() {
            changed |= self.apply_roster_snapshot(snapshot);
        }
        changed
    }

    /// a join event announces someone before their avatar and clan tag exist,
    /// so transcript members start out incomplete. copy the roster's version
    /// over them once it lands — permanently, because the roster will not hold
    /// that person forever and a chip must not decay back to a placeholder
    /// when they leave.
    fn adopt_transcript_identities(tab: &mut ChannelState) {
        if !tab.identities_pending {
            return;
        }
        let roster = tab.users.clone();
        let mut pending = false;
        for line in &mut tab.transcript {
            let members: &mut [&mut Vec<UiUser>] = match line {
                ChatLine::Membership { members, .. } => &mut [members],
                ChatLine::Digest { joined, left, .. } => &mut [joined, left],
                _ => continue,
            };
            for group in members {
                for member in group.iter_mut() {
                    pending |= !adopt_identity(member, &roster);
                }
            }
        }
        tab.identities_pending = pending;
    }

    pub(in crate::app::client) fn apply_roster_snapshot(
        &mut self,
        snapshot: RosterSnapshot,
    ) -> bool {
        let Some(position) = self
            .session
            .channels
            .tabs
            .iter()
            .position(|tab| tab.channel_index == Some(snapshot.channel_index))
        else {
            return false;
        };
        let next_users = snapshot
            .users
            .iter()
            .map(|user| UiUser::live(user, &mut self.session.roster.portraits))
            .collect::<Vec<_>>();
        let filter = self.session.channels.tabs[position].roster_filter.clone();
        let previous_users = self.session.channels.tabs[position].users.clone();
        let previous_visible = presented_roster_entries(
            &self.session.channels.tabs,
            &self.session.social.friends,
            &self.session.channels.tabs[position],
            &filter,
        );
        let mut projected_tabs = self.session.channels.tabs.clone();
        projected_tabs[position].users.clone_from(&next_users);
        let next_visible = presented_roster_entries(
            &projected_tabs,
            &self.session.social.friends,
            &projected_tabs[position],
            &filter,
        );
        let previous_handles = previous_visible
            .iter()
            .filter_map(|entry| entry.user().map(|user| user.handle))
            .collect::<Vec<_>>();
        let next_handles = next_visible
            .iter()
            .filter_map(|entry| entry.user().map(|user| user.handle))
            .collect::<Vec<_>>();
        self.session.channels.tabs[position].users = next_users;
        self.session.channels.tabs[position].roster_complete = snapshot.initial_complete;
        Self::adopt_transcript_identities(&mut self.session.channels.tabs[position]);

        if position != self.session.channels.active_tab {
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
                self.session.channels.tabs[position].id,
                previous_visible,
                &next_visible,
            );
        }
        Self::trace(format_args!(
            "roster channel={} users={} visible={}",
            snapshot.channel_index,
            self.session.channels.tabs[position].users.len(),
            next_handles.len()
        ));
        previous_handles != next_handles
            || previous_users != self.session.channels.tabs[position].users
    }
}

/// fill in whatever the join event could not know. returns whether the member
/// is now complete.
pub(in crate::app::client) fn adopt_identity(member: &mut UiUser, roster: &[UiUser]) -> bool {
    if member.portrait.is_some() && member.clan_tag.is_some() {
        return true;
    }
    let Some(current) = roster.iter().find(|user| user.handle == member.handle) else {
        // they are gone from the roster; whatever we already resolved is the
        // best this event will ever have, so stop asking.
        return true;
    };
    if member.portrait.is_none() && current.portrait.is_some() {
        member.portrait.clone_from(&current.portrait);
    }
    if member.clan_tag.is_none() && current.clan_tag.is_some() {
        member.clan_tag.clone_from(&current.clan_tag);
        member.name.clone_from(&current.name);
    }
    member.portrait.is_some() && member.clan_tag.is_some()
}
