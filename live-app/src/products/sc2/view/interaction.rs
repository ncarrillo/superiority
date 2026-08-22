use super::*;

impl LiveView {
    pub(super) fn roster_filter(&self, channel: Option<&str>) -> &str {
        channel
            .and_then(|channel| self.roster_filters.get(channel))
            .map_or("", String::as_str)
    }

    pub(super) fn set_roster_filter(&mut self, filter: String) {
        let Some(channel) = self.selected.clone() else {
            return;
        };
        let previous_filter = self.roster_filter(Some(&channel)).to_owned();
        if previous_filter == filter {
            return;
        }
        let previous = self
            .channel_data
            .get(&channel)
            .map_or_else(Vec::new, |data| {
                presented_roster_members(
                    &self.channels,
                    &self.channel_data,
                    &channel,
                    &data.roster,
                    &previous_filter,
                )
            });
        if filter.is_empty() {
            self.roster_filters.remove(&channel);
        } else {
            self.roster_filters.insert(channel.clone(), filter);
        }
        let active_filter = self.roster_filter(Some(&channel));
        if let Some(selected) = self.workspace.roster.selection(&channel)
            && self.channel_data.get(&channel).is_none_or(|data| {
                !data.roster.iter().any(|member| {
                    member.handle == selected && roster_member_matches(member, active_filter)
                })
            })
        {
            self.workspace.roster.set_selection(channel.clone(), None);
        }
        if let Some(data) = self.channel_data.get(&channel) {
            let next = presented_roster_members(
                &self.channels,
                &self.channel_data,
                &channel,
                &data.roster,
                self.roster_filter(Some(&channel)),
            );
            self.workspace.roster.begin_transition(
                channel,
                previous,
                &next,
                js_sys::Date::now(),
                |member| member.handle,
            );
        }
    }

    pub(super) fn move_roster_selection(&mut self, delta: isize) {
        let Some(channel) = self.selected.clone() else {
            return;
        };
        let members = self
            .channel_data
            .get(&channel)
            .map(|data| {
                presented_roster_members(
                    &self.channels,
                    &self.channel_data,
                    &channel,
                    &data.roster,
                    self.roster_filter(Some(&channel)),
                )
                .into_iter()
                .map(|member| member.handle)
                .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if members.is_empty() {
            return;
        }
        let positions = (0..members.len()).collect::<Vec<_>>();
        let _ = self.workspace.roster.move_selection(
            channel,
            &members,
            &positions,
            delta,
            ScrollStrategy::Nearest,
        );
    }

    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if (event.keystroke.modifiers.platform || event.keystroke.modifiers.control)
            && event.keystroke.key.eq_ignore_ascii_case("c")
            && self.workspace.transcript.selection.has_selection()
        {
            let rows = self
                .selected
                .as_deref()
                .and_then(|channel| self.channel_data.get(channel))
                .into_iter()
                .flat_map(|data| data.items.iter().enumerate())
                .map(|(index, item)| {
                    (
                        index,
                        chat::transcript_text(&stream_line(item, &self.ui_assets)),
                    )
                })
                .collect::<Vec<_>>();
            if let Some(text) = self.workspace.transcript.selection.selected_text(&rows) {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                cx.stop_propagation();
            }
            return;
        }
        if !self.workspace.roster.focused {
            return;
        }
        match event.keystroke.key.as_str() {
            "escape" => {
                if self.roster_filter(self.selected.as_deref()).is_empty() {
                    self.workspace.roster.focused = false;
                } else {
                    self.set_roster_filter(String::new());
                }
            }
            "backspace" | "delete" => {
                let mut filter = self.roster_filter(self.selected.as_deref()).to_owned();
                filter.pop();
                self.set_roster_filter(filter);
            }
            "up" => self.move_roster_selection(-1),
            "down" => self.move_roster_selection(1),
            _ => {
                if !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.control
                    && !event.keystroke.modifiers.alt
                    && let Some(value) = &event.keystroke.key_char
                    && value.chars().all(|character| !character.is_control())
                {
                    let mut filter = self.roster_filter(self.selected.as_deref()).to_owned();
                    if filter.chars().count() < 64 {
                        filter.extend(value.chars().filter(|character| !character.is_control()));
                        filter = filter.chars().take(64).collect();
                        self.set_roster_filter(filter);
                    }
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
}
