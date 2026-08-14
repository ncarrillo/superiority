use super::*;

mod model;
mod state;
mod view;

pub(in crate::app::client) use model::{
    UiUser, filtered_roster_count, presence_kind, presented_roster_range, presented_roster_users,
    shared_roster_user,
};

pub(in crate::app::client) const ROSTER_DEBOUNCE_BASE: Duration = Duration::from_millis(45);
pub(in crate::app::client) const ROSTER_DEBOUNCE_MAX_WINDOW: Duration = Duration::from_millis(240);
pub(in crate::app::client) const ROSTER_DEBOUNCE_MAX_LATENCY: Duration = Duration::from_millis(380);
pub(in crate::app::client) const ROSTER_HOVER_DEFER_MAX: Duration = Duration::from_secs(10);
pub(in crate::app::client) const ROSTER_HOVER_RECHECK: Duration = Duration::from_millis(400);
pub(super) struct RosterComponent {
    pub(super) roster: ui_workspace::RosterState<u64, UiUser, Instant>,
    pub(super) roster_input: ui_text_input::TextInput,
    pub(super) roster_hovered: bool,
    pub(super) roster_defer_started: Option<Instant>,
    pub(super) pending_rosters: HashMap<u8, RosterSnapshot>,
    pub(super) roster_batch_started: Option<Instant>,
    pub(super) roster_flush_at: Option<Instant>,
    pub(super) roster_debounce_window: Duration,
    pub(super) portraits: PortraitRegistry,
}

impl RosterComponent {
    pub(super) fn row(
        &self,
        user: &UiUser,
        selected: bool,
        channel_title: String,
        assets: &UiAssets,
        cx: &mut Context<SuperiorityView>,
    ) -> ui_roster::RosterRow {
        let handle = user.handle;
        let hover_group = format!("roster-user-{handle}");
        ui_roster::RosterRow::new(
            format!("roster-user-{handle}"),
            hover_group,
            shared_roster_user(user, assets),
            channel_title,
            selected,
            assets.clone(),
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            this.roster.roster_input.focus(window, cx);
            this.composer.composer_focused = false;
            this.join.join_focused = false;
            this.roster.roster.focused = true;
            this.set_selected_user(Some(handle));
            cx.stop_propagation();
            cx.notify();
        }))
    }

    fn row_snapshot(&self, user: &UiUser, selected: bool, assets: &UiAssets) -> Div {
        ui_roster::static_row(&shared_roster_user(user, assets), assets, selected)
    }

    pub(super) fn row_slots(
        &self,
        channels: &ChannelComponent,
        channel: Option<&ChannelState>,
        selected_user: Option<u32>,
        interactive: bool,
        assets: &UiAssets,
        cx: &mut Context<SuperiorityView>,
    ) -> Vec<AnyElement> {
        let now = Instant::now();
        let users = channel
            .map(|channel| presented_roster_users(&channels.tabs, channel, &channel.roster_filter))
            .unwrap_or_default();
        let animation = interactive
            .then_some((channel, self.roster.animation.as_ref()))
            .and_then(|(channel, animation)| match (channel, animation) {
                (Some(channel), Some(animation)) if animation.scope == channel.id => {
                    Some(animation)
                }
                _ => None,
            });
        ui_roster::animated_rows(
            users,
            animation,
            now,
            |user| user.handle,
            ROSTER_ROW_GAP,
            |user, motion| {
                let selected = selected_user == Some(user.handle);
                let row: AnyElement = if interactive && motion != ui_roster::RowMotion::Removed {
                    self.row(
                        user,
                        selected,
                        channel.map_or_else(String::new, |channel| channel.title.clone()),
                        assets,
                        cx,
                    )
                    .into_any_element()
                } else {
                    self.row_snapshot(user, selected, assets).into_any_element()
                };
                row
            },
        )
    }
}
