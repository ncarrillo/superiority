use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn active_roster_filter(&self) -> &str {
        self.channels
            .active()
            .map_or("", |channel| channel.roster_filter.as_str())
    }

    pub(in crate::app::client) fn sync_roster_filter_input(&mut self) {
        let filter = self.active_roster_filter().to_owned();
        if self.roster.roster_input.content() != filter {
            self.roster.roster_input.set_content(filter);
        }
    }

    pub(in crate::app::client) fn selected_user(&self) -> Option<u32> {
        let channel = self.channels.active()?;
        self.roster.roster.selection(&channel.id)
    }

    pub(in crate::app::client) fn set_selected_user(&mut self, selected: Option<u32>) {
        let Some(channel_id) = self.channels.active().map(|channel| channel.id) else {
            return;
        };
        self.roster.roster.set_selection(channel_id, selected);
    }

    pub(in crate::app::client) fn visible_roster_users(&self) -> Vec<UiUser> {
        self.channels.active().map_or_else(Vec::new, |channel| {
            presented_roster_users(
                &self.channels.tabs,
                &self.social.friends,
                channel,
                &channel.roster_filter,
            )
        })
    }

    pub(in crate::app::client) fn visible_roster_entries(&self) -> Vec<RosterEntry> {
        self.channels.active().map_or_else(Vec::new, |channel| {
            presented_roster_entries(
                &self.channels.tabs,
                &self.social.friends,
                channel,
                &channel.roster_filter,
            )
        })
    }

    /// keyboard navigation walks people, not headers — but the list it scrolls
    /// counts both, so every selectable handle carries the row it lives on.
    fn roster_cursor(&self) -> (Vec<u32>, Vec<usize>) {
        let mut handles = Vec::new();
        let mut positions = Vec::new();
        for (index, entry) in self.visible_roster_entries().iter().enumerate() {
            if let Some(user) = entry.user() {
                handles.push(user.handle);
                positions.push(index);
            }
        }
        (handles, positions)
    }

    pub(in crate::app::client) fn roster_base_scroll(&self) -> ScrollHandle {
        self.roster.roster.scroll.0.borrow().base_handle.clone()
    }

    pub(in crate::app::client) fn set_roster_filter(&mut self, next: String) {
        if self.active_roster_filter() == next {
            return;
        }
        let previous = self.visible_roster_entries();
        let Some(channel) = self.channels.tabs.get_mut(self.channels.active_tab) else {
            return;
        };
        channel.roster_filter = next;
        let next = self.visible_roster_entries();
        let next_handles = next
            .iter()
            .filter_map(|entry| entry.user().map(|user| user.handle))
            .collect::<Vec<_>>();
        if self
            .selected_user()
            .is_some_and(|handle| !next_handles.contains(&handle))
        {
            self.set_selected_user(None);
        }
        if let Some(channel_id) = self.channels.active().map(|channel| channel.id) {
            self.begin_roster_animation(channel_id, previous, &next);
        }
    }

    pub(in crate::app::client) fn select_roster_index(&mut self, index: usize) {
        let (handles, positions) = self.roster_cursor();
        let Some(channel_id) = self.channels.active().map(|channel| channel.id) else {
            return;
        };
        let _ = self.roster.roster.select_index(
            channel_id,
            &handles,
            &positions,
            index,
            ScrollStrategy::Center,
        );
    }

    pub(in crate::app::client) fn move_roster_selection(&mut self, delta: isize) {
        let (handles, positions) = self.roster_cursor();
        let Some(channel_id) = self.channels.active().map(|channel| channel.id) else {
            return;
        };
        let _ = self.roster.roster.move_selection(
            channel_id,
            &handles,
            &positions,
            delta,
            ScrollStrategy::Center,
        );
    }

    pub(in crate::app::client) fn begin_roster_animation(
        &mut self,
        channel_id: u64,
        previous: Vec<RosterEntry>,
        next: &[RosterEntry],
    ) {
        let previous_handles = previous
            .iter()
            .map(RosterEntry::handle)
            .collect::<BTreeSet<_>>();
        let next_handles = next
            .iter()
            .map(RosterEntry::handle)
            .collect::<BTreeSet<_>>();
        if previous_handles == next_handles {
            self.roster.roster.animation = None;
            return;
        }
        self.roster.roster.begin_transition(
            channel_id,
            previous,
            next,
            Instant::now(),
            RosterEntry::handle,
        );
    }
}
