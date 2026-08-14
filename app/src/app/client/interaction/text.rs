use super::super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn sync_text_inputs(&mut self) {
        let join_query = self.join.join_input.content();
        if join_query != self.join.join_query {
            self.join.join_query = join_query;
            self.join.join_selected = 0;
            self.join.schedule_group_search();
        }

        let mut roster_filter = self.roster.roster_input.content();
        if roster_filter.chars().count() > 64 {
            roster_filter = roster_filter.chars().take(64).collect();
            self.roster.roster_input.set_content(roster_filter.clone());
        }
        if roster_filter != self.active_roster_filter() {
            self.set_roster_filter(roster_filter);
        }
    }

    pub(in crate::app::client) fn text_input_focused(&self, window: &Window) -> bool {
        self.composer.composer.is_focused(window)
            || self.join.join_input.is_focused(window)
            || self.roster.roster_input.is_focused(window)
            || self.social.conversation_input.is_focused(window)
    }
}
