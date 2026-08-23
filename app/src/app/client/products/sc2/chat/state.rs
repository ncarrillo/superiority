use super::*;

impl SuperiorityView {
    /// append a join or leave, folding it into the previous event when the same
    /// kind landed within the collapse window. merging re-stamps the existing
    /// row so it re-reveals rather than appearing silently.
    pub(in crate::app::client) fn append_membership(
        &mut self,
        index: usize,
        kind: MembershipKind,
        member: UiUser,
    ) {
        if index >= self.session.channels.tabs.len() {
            return;
        }
        // a join knows neither avatar nor clan tag yet; flag the channel so the
        // next roster update fills them in.
        if member.portrait.is_none() || member.clan_tag.is_none() {
            self.session.channels.tabs[index].identities_pending = true;
        }
        if self.session.channels.tabs[index].users.len() > HIGH_TRAFFIC_ONLINE {
            self.append_digest(index, kind, member);
            return;
        }
        let Some(tab) = self.session.channels.tabs.get_mut(index) else {
            return;
        };
        let now = Instant::now();
        let mergeable = matches!(
            tab.transcript.last(),
            Some(ChatLine::Membership {
                kind: previous,
                at,
                expanded: false,
                ..
            }) if *previous == kind
                && now.saturating_duration_since(*at) <= MEMBERSHIP_COLLAPSE_WINDOW
        );
        if mergeable {
            let time = Self::current_timestamp();
            let Some(ChatLine::Membership {
                time: line_time,
                at,
                members,
                ..
            }) = tab.transcript.last_mut()
            else {
                return;
            };
            *line_time = time;
            *at = now;
            members.push(member);
            self.keep_following_bottom(index);
            return;
        }
        self.append_chat_line(
            index,
            ChatLine::Membership {
                time: Self::current_timestamp(),
                at: now,
                kind,
                members: vec![member],
                expanded: false,
            },
        );
    }

    /// fold one arrival or departure into the current minute's digest, opening
    /// a new one once the window has passed. the live minute updates in place
    /// rather than appending, so a busy channel grows by one line a minute.
    fn append_digest(&mut self, index: usize, kind: MembershipKind, member: UiUser) {
        let now = Instant::now();
        let Some(tab) = self.session.channels.tabs.get_mut(index) else {
            return;
        };
        let open = matches!(
            tab.transcript.last(),
            Some(ChatLine::Digest { opened, .. })
                if now.saturating_duration_since(*opened) < DIGEST_WINDOW
        );
        if open {
            let Some(ChatLine::Digest {
                at, joined, left, ..
            }) = tab.transcript.last_mut()
            else {
                return;
            };
            *at = now;
            match kind {
                MembershipKind::Joined => joined.push(member),
                MembershipKind::Left => left.push(member),
            }
            self.keep_following_bottom(index);
            return;
        }
        let (joined, left) = match kind {
            MembershipKind::Joined => (vec![member], Vec::new()),
            MembershipKind::Left => (Vec::new(), vec![member]),
        };
        self.append_chat_line(
            index,
            ChatLine::Digest {
                time: Self::current_timestamp(),
                at: now,
                opened: now,
                joined,
                left,
            },
        );
    }

    pub(in crate::app::client) fn select_digest_member(
        &mut self,
        tab_id: u64,
        line_index: usize,
        member: usize,
    ) {
        let Some(ChatLine::Digest { joined, .. }) = self
            .session
            .channels
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| tab.transcript.get(line_index))
        else {
            return;
        };
        let Some(handle) = joined.get(member).map(|member| member.handle) else {
            return;
        };
        self.set_selected_user(Some(handle));
    }

    /// open this digest, or start closing it if it is the one already open.
    /// opening replaces whatever was open — that is what holds the transcript
    /// to a single expansion — and hands the new one its own scroll handles
    /// rather than a position inherited from the last.
    pub(in crate::app::client) fn toggle_digest_expansion(
        &mut self,
        tab_id: u64,
        line_index: usize,
    ) {
        let now = Instant::now();
        if let Some(expanded) = &mut self.session.chat.expanded_digest
            && expanded.tab_id == tab_id
            && expanded.line_index == line_index
        {
            if !expanded.closing {
                expanded.closing = true;
                expanded.started = now;
            }
            return;
        }
        self.session.chat.expanded_digest = Some(ExpandedDigest {
            tab_id,
            line_index,
            started: now,
            closing: false,
            joined: ScrollHandle::default(),
            left: ScrollHandle::default(),
        });
    }

    /// clicking a name chip takes you to that person in the roster — the app's
    /// profile surface for someone who is not you.
    pub(in crate::app::client) fn select_event_member(
        &mut self,
        tab_id: u64,
        line_index: usize,
        member: usize,
    ) {
        let Some(ChatLine::Membership { members, .. }) = self
            .session
            .channels
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| tab.transcript.get(line_index))
        else {
            return;
        };
        let Some(handle) = members.get(member).map(|member| member.handle) else {
            return;
        };
        self.set_selected_user(Some(handle));
    }

    pub(in crate::app::client) fn toggle_event_expansion(
        &mut self,
        tab_id: u64,
        line_index: usize,
    ) {
        let Some(ChatLine::Membership { expanded, .. }) = self
            .session
            .channels
            .tabs
            .iter_mut()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| tab.transcript.get_mut(line_index))
        else {
            return;
        };
        *expanded = !*expanded;
    }

    /// a line that updated in place must not replay its entrance — the counts
    /// change, the row does not re-appear. keep following the bottom, though,
    /// in case the row grew. a tab that is not open keeps following too: its
    /// handle holds the request until the tab is next drawn.
    fn keep_following_bottom(&mut self, index: usize) {
        if let Some(channel) = self.session.channels.tabs.get(index)
            && follows_bottom(&channel.scroll)
        {
            channel.scroll.scroll_to_bottom();
        }
    }

    /// puts a line in the open channel that answers something the reader just
    /// did, rather than something the service said.
    pub(in crate::app::client) fn append_error_line(&mut self, text: String) {
        if self.session.channels.active_tab >= self.session.channels.tabs.len() {
            return;
        }
        self.append_chat_line(
            self.session.channels.active_tab,
            ChatLine::Error {
                time: Self::current_timestamp(),
                text,
            },
        );
    }

    pub(in crate::app::client) fn append_chat_line(&mut self, index: usize, line: ChatLine) {
        if index >= self.session.channels.tabs.len() {
            return;
        }
        let active = index == self.session.channels.active_tab;
        let follows_bottom = follows_bottom(&self.session.channels.tabs[index].scroll);
        self.session.channels.tabs[index].transcript.push(line);
        if self.session.channels.tabs[index].transcript.len() > 2_000 {
            self.session.channels.tabs[index].transcript.drain(..500);
        }
        if active {
            self.session.channels.chat_entry_reveal = Some(ChatEntryReveal {
                tab_id: self.session.channels.tabs[index].id,
                line_index: self.session.channels.tabs[index]
                    .transcript
                    .len()
                    .saturating_sub(1),
                started: Instant::now(),
            });
        }
        if follows_bottom {
            self.session.channels.tabs[index].scroll.scroll_to_bottom();
        }
    }
}

/// whether a transcript is at (or within a few pixels of) its last line. read
/// from the handle's last layout, so it also answers for a tab that is not on
/// screen — a fresh tab has no layout yet and counts as following.
fn follows_bottom(scroll: &ScrollHandle) -> bool {
    let offset = -f32::from(scroll.offset().y);
    let maximum = f32::from(scroll.max_offset().y);
    maximum <= CHAT_BOTTOM_TOLERANCE || (maximum - offset).abs() <= CHAT_BOTTOM_TOLERANCE
}
