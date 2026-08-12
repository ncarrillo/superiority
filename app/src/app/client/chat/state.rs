use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn chat_follows_bottom(&self) -> bool {
        let offset = -f32::from(self.chat.transcript.scroll.offset().y);
        let maximum = f32::from(self.chat.transcript.scroll.max_offset().y);
        maximum <= CHAT_BOTTOM_TOLERANCE || (maximum - offset).abs() <= CHAT_BOTTOM_TOLERANCE
    }

    pub(in crate::app::client) fn append_chat_line(&mut self, index: usize, line: ChatLine) {
        if index >= self.channels.tabs.len() {
            return;
        }
        let active = index == self.channels.active_tab;
        let follows_bottom = active && self.chat_follows_bottom();
        self.channels.tabs[index].transcript.push(line);
        if self.channels.tabs[index].transcript.len() > 2_000 {
            self.channels.tabs[index].transcript.drain(..500);
        }
        if active {
            self.channels.chat_entry_reveal = Some(ChatEntryReveal {
                tab_id: self.channels.tabs[index].id,
                line_index: self.channels.tabs[index].transcript.len().saturating_sub(1),
                started: Instant::now(),
            });
            if follows_bottom {
                self.chat.transcript.scroll.scroll_to_bottom();
            }
        }
    }
}
