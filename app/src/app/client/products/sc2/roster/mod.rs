use super::*;

mod model;
mod state;
mod view;

pub(in crate::app::client) use model::{
    RosterAffinity, RosterEntry, UiUser, filtered_roster_count, presence_kind,
    presented_roster_entries, presented_roster_entry_count, presented_roster_range,
    presented_roster_users, shared_roster_user,
};

pub(in crate::app::client) const ROSTER_DEBOUNCE_BASE: Duration = Duration::from_millis(45);
pub(in crate::app::client) const ROSTER_DEBOUNCE_MAX_WINDOW: Duration = Duration::from_millis(240);
pub(in crate::app::client) const ROSTER_DEBOUNCE_MAX_LATENCY: Duration = Duration::from_millis(380);
pub(in crate::app::client) const ROSTER_HOVER_DEFER_MAX: Duration = Duration::from_secs(10);
pub(in crate::app::client) const ROSTER_HOVER_RECHECK: Duration = Duration::from_millis(400);
pub(in crate::app::client) struct RosterComponent {
    pub(in crate::app::client) roster: ui_workspace::RosterState<u64, RosterEntry, Instant>,
    pub(in crate::app::client) roster_input: ui_text_input::TextInput,
    pub(in crate::app::client) roster_hovered: bool,
    pub(in crate::app::client) roster_defer_started: Option<Instant>,
    pub(in crate::app::client) pending_rosters: HashMap<u8, RosterSnapshot>,
    pub(in crate::app::client) roster_batch_started: Option<Instant>,
    pub(in crate::app::client) roster_flush_at: Option<Instant>,
    pub(in crate::app::client) roster_debounce_window: Duration,
    /// this product's avatars. Per-session on purpose: the registry is keyed by
    /// `ImageTableEntry`, which is `StarCraft II`'s own wire type, and holds
    /// seventeen of `StarCraft II`'s atlases. Another product has a different
    /// avatar model — Remastered resolves its avatars through ToonProfile and
    /// Url — so this is never shared chrome and never cloned from one product
    /// into another. An unresolved product adapter gets its own placeholder.
    pub(in crate::app::client) portraits: PortraitRegistry,
}

impl RosterComponent {
    pub(in crate::app::client) fn row(
        &self,
        user: &UiUser,
        selected: bool,
        channel_title: String,
        assets: &Sc2Assets,
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
            this.session.roster.roster_input.focus(window, cx);
            this.session.composer.composer_focused = false;
            this.session.roster.roster.focused = true;
            this.set_selected_user(Some(handle));
            cx.stop_propagation();
            cx.notify();
        }))
    }

    fn row_snapshot(&self, user: &UiUser, selected: bool, assets: &Sc2Assets) -> Div {
        ui_roster::static_row(&shared_roster_user(user, assets), assets, selected)
    }

    pub(in crate::app::client) fn entry(
        &self,
        entry: &RosterEntry,
        selected_user: Option<u32>,
        channel_title: &str,
        interactive: bool,
        assets: &Sc2Assets,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        match entry {
            RosterEntry::Segment { segment, count } => {
                ui_roster::segment_header(*segment, *count).into_any_element()
            }
            RosterEntry::User(user) => {
                let selected = selected_user == Some(user.handle);
                if interactive {
                    self.row(user, selected, channel_title.to_owned(), assets, cx)
                        .into_any_element()
                } else {
                    self.row_snapshot(user, selected, assets).into_any_element()
                }
            }
        }
    }

    pub(in crate::app::client) fn row_slots(
        &self,
        channels: &ChannelComponent,
        channel: Option<&ChannelState>,
        friends: &[UiFriend],
        selected_user: Option<u32>,
        interactive: bool,
        assets: &Sc2Assets,
        cx: &mut Context<SuperiorityView>,
    ) -> Vec<AnyElement> {
        let now = Instant::now();
        let entries = channel
            .map(|channel| {
                presented_roster_entries(&channels.tabs, friends, channel, &channel.roster_filter)
            })
            .unwrap_or_default();
        let animation = interactive
            .then_some((channel, self.roster.animation.as_ref()))
            .and_then(|(channel, animation)| match (channel, animation) {
                (Some(channel), Some(animation)) if animation.scope == channel.id => {
                    Some(animation)
                }
                _ => None,
            });
        let title = channel.map_or_else(String::new, |channel| channel.title.clone());
        ui_roster::animated_rows(
            entries,
            animation,
            now,
            RosterEntry::handle,
            ROSTER_ROW_GAP,
            |entry, motion| {
                let live = interactive && motion != ui_roster::RowMotion::Removed;
                self.entry(entry, selected_user, &title, live, assets, cx)
            },
        )
    }
}
