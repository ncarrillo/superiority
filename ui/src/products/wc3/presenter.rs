//! the Reforged hall's presentation state, policy, and assembly.
//!
//! The mirror of [`super::super::scr::presenter::Console`], scaled to a realm:
//! one hall at a time, its transcript and its member list, the filtering and
//! ordering and selection the desktop and Superiority Live would otherwise each
//! write against the same [`super::components`] renderers.
//!
//! It owns all of it over the neutral presentation models ([`super::RosterUser`],
//! [`super::TranscriptLine`]). A host adapts its protocol or DTO values into
//! those, hands them in, and supplies only the focus behaviour and the text
//! input that are its own. It is generic over [`AnimationClock`] so the desktop
//! drives it with `Instant` and the browser with `f64` milliseconds.
//!
//! Unlike the console there is no per-line reveal: Reforged's transcript simply
//! scrolls to the bottom as lines land, exactly as the desktop hall does.

use std::ops::Range;

use gpui::{
    Context, MouseButton, Render, ScrollStrategy, Window, div, prelude::*, px, uniform_list,
};

use crate::{
    foundation::{WithScrollbar as _, animation::AnimationClock, text_input::TextInput},
    patterns::{roster as roster_pattern, transcript::TranscriptSelection, workspace::RosterState},
    products::wc3::{
        RosterPresence, RosterUser, TranscriptLine,
        components::{chat as wc3_chat, roster as wc3_roster},
        theme::{ROSTER_ROW_GAP, ROSTER_ROW_HEIGHT},
    },
};

/// the hall the window is in.
struct HallChannel {
    name: String,
    transcript: Vec<TranscriptLine>,
    members: Vec<RosterUser>,
    filter: String,
}

/// the transcript history bound; older lines fall off the top.
const MAX_TRANSCRIPT_LINES: usize = 2_000;
const TRANSCRIPT_DRAIN: usize = 500;

/// the Reforged hall's whole presentation, minus the composer, titlebar, and
/// account plaque, which stay with the host.
pub struct Hall<C: AnimationClock> {
    channel: Option<HallChannel>,
    transcript: wc3_chat::TranscriptState,
    roster: RosterState<String, RosterUser, C, u64>,
}

impl<C: AnimationClock> Default for Hall<C> {
    fn default() -> Self {
        Self {
            channel: None,
            transcript: wc3_chat::TranscriptState::default(),
            roster: RosterState::default(),
        }
    }
}

impl<C: AnimationClock> Hall<C> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn has_channel(&self) -> bool {
        self.channel.is_some()
    }

    #[must_use]
    pub fn channel_name(&self) -> Option<&str> {
        self.channel.as_ref().map(|channel| channel.name.as_str())
    }

    /// the lowercased hall name the roster state is scoped by.
    fn scope(name: &str) -> String {
        name.to_lowercase()
    }

    fn current_scope(&self) -> Option<String> {
        self.channel
            .as_ref()
            .map(|channel| Self::scope(&channel.name))
    }

    #[must_use]
    pub fn member_count(&self) -> usize {
        self.channel
            .as_ref()
            .map_or(0, |channel| channel.members.len())
    }

    #[must_use]
    pub fn filter(&self) -> &str {
        self.channel
            .as_ref()
            .map_or("", |channel| channel.filter.as_str())
    }

    #[must_use]
    pub fn roster_focused(&self) -> bool {
        self.roster.focused
    }

    pub fn set_roster_focused(&mut self, focused: bool) {
        self.roster.focused = focused;
    }

    /// a clone of the shared transcript selection, for the host's copy keystroke.
    #[must_use]
    pub fn transcript_selection(&self) -> TranscriptSelection {
        self.transcript.selection.clone()
    }

    pub fn clear_transcript_selection(&self) {
        self.transcript.selection.clear();
    }

    /// the transcript lines as `(row, text)`, for the copy keystroke.
    #[must_use]
    pub fn transcript_rows_text(&self) -> Vec<(usize, String)> {
        self.channel
            .as_ref()
            .map(|channel| {
                channel
                    .transcript
                    .iter()
                    .enumerate()
                    .map(|(index, line)| (index, wc3_chat::transcript_text(line)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// resets to no hall — a fresh sign-in, a sign-out, or an account switch.
    pub fn clear(&mut self) {
        self.roster.clear_interaction();
        self.roster.selections.clear();
        self.transcript.selection.clear();
        self.channel = None;
    }

    /// installs the hall's name and membership, animating the roster if it is
    /// the same hall and resetting the view if it is a new one. Members arrive
    /// already resolved into [`RosterUser`]s by the host's adapter.
    /// `session_online` seeds the "you entered" line's population and
    /// `session_time` its timestamp when the hall is new.
    pub fn apply_channel(
        &mut self,
        name: String,
        members: Vec<RosterUser>,
        session_online: usize,
        session_time: String,
        now: C,
    ) {
        let scope = Self::scope(&name);
        if self
            .channel
            .as_ref()
            .is_some_and(|channel| Self::scope(&channel.name) == scope)
        {
            let (filter, previous) = self
                .channel
                .as_ref()
                .map(|channel| {
                    (
                        channel.filter.clone(),
                        Self::filtered(&channel.members, &channel.filter),
                    )
                })
                .expect("channel present");
            let next = Self::filtered(&members, &filter);
            self.roster
                .begin_transition(scope.clone(), previous, &next, now, |member| member.handle);
            let channel = self.channel.as_mut().expect("channel present");
            channel.name = name;
            channel.members = members;
            if self.roster.selection(&scope).is_some_and(|selected| {
                !channel
                    .members
                    .iter()
                    .any(|member| member.handle == selected)
            }) {
                self.roster.set_selection(scope, None);
            }
            return;
        }
        self.roster.clear_interaction();
        self.roster.retain_selections(|current| *current == scope);
        self.roster.set_selection(scope, None);
        self.transcript.selection.clear();
        self.transcript.scroll.scroll_to_bottom();
        self.channel = Some(HallChannel {
            name: name.clone(),
            transcript: vec![TranscriptLine::SessionStart {
                time: session_time,
                channel: name,
                online: session_online,
            }],
            members,
            filter: String::new(),
        });
    }

    /// appends one transcript line, following the bottom when the reader is
    /// already there. Reforged has no per-line reveal.
    pub fn append_line(&mut self, line: TranscriptLine) {
        let follows_bottom = self.transcript_follows_bottom();
        let Some(channel) = self.channel.as_mut() else {
            return;
        };
        channel.transcript.push(line);
        if channel.transcript.len() > MAX_TRANSCRIPT_LINES {
            channel.transcript.drain(..TRANSCRIPT_DRAIN);
        }
        if follows_bottom {
            self.transcript.scroll.scroll_to_bottom();
        }
    }

    #[must_use]
    pub fn transcript_follows_bottom(&self) -> bool {
        let offset = -f32::from(self.transcript.scroll.offset().y);
        let maximum = f32::from(self.transcript.scroll.max_offset().y);
        maximum <= 6.0 || (maximum - offset).abs() <= 6.0
    }

    #[must_use]
    pub fn selected_member(&self) -> Option<u64> {
        let scope = self.current_scope()?;
        self.roster.selection(&scope)
    }

    pub fn set_selected_member(&mut self, selected: Option<u64>) {
        let Some(channel) = self.channel.as_ref() else {
            return;
        };
        let scope = Self::scope(&channel.name);
        let selected = selected.filter(|handle| {
            channel
                .members
                .iter()
                .any(|member| member.handle == *handle)
        });
        self.roster.set_selection(scope, selected);
    }

    /// narrows the roster, animating the change and dropping a hidden selection.
    pub fn set_filter(&mut self, next: String, now: C) {
        let Some(channel) = self.channel.as_ref() else {
            return;
        };
        if channel.filter == next {
            return;
        }
        let scope = Self::scope(&channel.name);
        let previous = Self::filtered(&channel.members, &channel.filter);
        let next_members = Self::filtered(&channel.members, &next);
        self.channel.as_mut().expect("channel present").filter = next;
        if self
            .roster
            .selection(&scope)
            .is_some_and(|selected| !next_members.iter().any(|member| member.handle == selected))
        {
            self.roster.set_selection(scope.clone(), None);
        }
        self.roster
            .begin_transition(scope, previous, &next_members, now, |member| member.handle);
    }

    /// the membership after the filter, present-first then absent, alphabetical
    /// within each — the order the list draws.
    #[must_use]
    pub fn visible_members(&self) -> Vec<RosterUser> {
        self.channel
            .as_ref()
            .map(|channel| Self::filtered(&channel.members, &channel.filter))
            .unwrap_or_default()
    }

    /// Reforged counts Away and Offline as absent; both sink below the present.
    const fn absent(presence: RosterPresence) -> bool {
        matches!(presence, RosterPresence::Away | RosterPresence::Offline)
    }

    fn filtered(members: &[RosterUser], filter: &str) -> Vec<RosterUser> {
        let mut members = roster_pattern::filtered_refs(members, filter, |member, query| {
            roster_pattern::filter_matches(&member.name, query)
                || member
                    .clan_abbreviation
                    .as_ref()
                    .is_some_and(|clan| roster_pattern::filter_matches(clan, query))
        })
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
        members.sort_by(|left, right| {
            roster_pattern::present_before_absent(
                Self::absent(left.presence),
                &left.name,
                Self::absent(right.presence),
                &right.name,
            )
        });
        members
    }

    pub fn move_selection(&mut self, delta: isize) -> bool {
        let Some(scope) = self.current_scope() else {
            return false;
        };
        let handles = self
            .visible_members()
            .into_iter()
            .map(|member| member.handle)
            .collect::<Vec<_>>();
        let positions = (1..=handles.len()).collect::<Vec<_>>();
        self.roster
            .move_selection(scope, &handles, &positions, delta, ScrollStrategy::Center)
            .is_some()
    }

    pub fn select_index(&mut self, index: usize) -> bool {
        let Some(scope) = self.current_scope() else {
            return false;
        };
        let handles = self
            .visible_members()
            .into_iter()
            .map(|member| member.handle)
            .collect::<Vec<_>>();
        let positions = (1..=handles.len()).collect::<Vec<_>>();
        self.roster
            .select_index(scope, &handles, &positions, index, ScrollStrategy::Center)
            .is_some()
    }

    #[must_use]
    pub fn page_rows(&self) -> isize {
        let height = f32::from(
            self.roster
                .scroll
                .0
                .borrow()
                .base_handle
                .bounds()
                .size
                .height,
        );
        (height / (ROSTER_ROW_HEIGHT + ROSTER_ROW_GAP))
            .floor()
            .max(1.0) as isize
    }

    pub fn take_roster_animation(&mut self) -> bool {
        self.roster.animation.take().is_some()
    }

    #[must_use]
    pub fn is_animating(&self, now: C) -> bool {
        self.roster
            .animation
            .as_ref()
            .is_some_and(|animation| animation.is_running(now))
    }

    pub fn finish_animations(&mut self, now: C) {
        self.roster.finish_transition(now);
    }

    /// the transcript. No host interaction rides on it — selection is carried by
    /// the shared handle inside the rows — so this needs nothing from the host.
    #[must_use]
    pub fn transcript_viewport(&self, _now: C) -> wc3_chat::TranscriptViewport {
        let mut transcript = wc3_chat::TranscriptViewport::new(
            "wc3-chat-transcript",
            self.transcript.selection.clone(),
        )
        .scroll(&self.transcript.scroll);
        let Some(channel) = self.channel.as_ref() else {
            return transcript.empty("NO HALL ENTERED · /JOIN <REALM> OPENS A DOOR");
        };
        let scope = Self::scope(&channel.name);
        for (index, line) in channel.transcript.iter().enumerate() {
            transcript = transcript.child(wc3_chat::selectable_transcript_row(
                line,
                &scope,
                index,
                &self.transcript.selection,
            ));
        }
        transcript
    }

    /// the whole member-list panel, wired to the host. The one place a
    /// [`HallHost`] is required, because every row and the header carry a
    /// callback that mutates the host.
    #[must_use]
    pub fn roster_panel<H: HallHost<C>>(
        &self,
        roster_input: TextInput,
        now: C,
        window: &mut Window,
        cx: &mut Context<H>,
    ) -> wc3_roster::RosterPanel {
        let focused = self.roster.focused;
        let total = self.member_count();
        let filter = self.filter().to_owned();
        let filtered = self.visible_members().len();
        let scope = self.current_scope().unwrap_or_else(|| "no-hall".to_owned());
        let selected = self.selected_member();
        let base_scroll = self.roster.scroll.0.borrow().base_handle.clone();
        let scroll = self.roster.scroll.clone();

        let animating = self.current_scope().is_some_and(|scope| {
            self.roster
                .animation
                .as_ref()
                .is_some_and(|animation| animation.scope == scope && animation.is_running(now))
        });

        let mut list = wc3_roster::list_layer("wc3-roster-list")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|host, _, window, cx| host.wc3_focus_roster(window, cx)),
            )
            .on_click(cx.listener(|host, _, _, cx| {
                if let Some(hall) = host.wc3_hall_mut() {
                    hall.set_selected_member(None);
                }
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(|host, _, _, cx| {
                if host.wc3_hall_mut().is_some_and(Hall::take_roster_animation) {
                    cx.notify();
                }
            }));

        list = if animating {
            let members = self.visible_members();
            let rows = roster_pattern::animated_rows(
                members,
                self.roster.animation.as_ref(),
                now,
                |member| member.handle,
                ROSTER_ROW_HEIGHT,
                ROSTER_ROW_GAP,
                |member, motion| {
                    let is_selected = selected == Some(member.handle);
                    if motion == roster_pattern::RowMotion::Removed {
                        wc3_roster::RosterRow::new(
                            format!("wc3-roster-member-removed-{}", member.handle),
                            format!("wc3-roster-member-removed-{}", member.handle),
                            member.clone(),
                            is_selected,
                        )
                        .into_any_element()
                    } else {
                        roster_row(member, is_selected, cx).into_any_element()
                    }
                },
            );
            list.overflow_y_scroll()
                .track_scroll(&base_scroll)
                .child(roster_pattern::virtual_row_slot(
                    wc3_roster::segment_header(filtered),
                    ROSTER_ROW_HEIGHT,
                    ROSTER_ROW_GAP,
                ))
                .children(rows)
        } else {
            let item_count = usize::from(self.has_channel()) + filtered;
            list.child(
                uniform_list(
                    "wc3-roster-users",
                    item_count,
                    cx.processor(move |host: &mut H, range: Range<usize>, _, cx| {
                        let (members, selected) = host.wc3_hall().map_or_else(
                            || (Vec::new(), None),
                            |hall| (hall.visible_members(), hall.selected_member()),
                        );
                        range
                            .filter_map(|index| {
                                let row = if index == 0 {
                                    wc3_roster::segment_header(members.len()).into_any_element()
                                } else {
                                    let member = members.get(index - 1)?;
                                    roster_row(member, selected == Some(member.handle), cx)
                                        .into_any_element()
                                };
                                Some(
                                    roster_pattern::virtual_row_slot(
                                        row,
                                        ROSTER_ROW_HEIGHT,
                                        ROSTER_ROW_GAP,
                                    )
                                    .into_any_element(),
                                )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .size_full()
                .track_scroll(&scroll),
            )
        };
        let list = list.vertical_scrollbar_in(
            &base_scroll,
            crate::products::modal::ModalVariant::Reforged.scrollbar(),
            window,
            cx,
        );

        let model =
            wc3_roster::RosterHeaderModel::new("Players", total, filtered, &filter, focused);
        let mut header =
            wc3_roster::RosterHeader::new(format!("wc3-roster-header-{scope}"), model, focused)
                .on_focus(cx.listener(|host, _, window, cx| host.wc3_focus_roster(window, cx)));
        if !filter.is_empty() {
            header = header.on_clear(cx.listener(|host, _, _, cx| {
                host.wc3_clear_roster_filter(cx);
                cx.stop_propagation();
                cx.notify();
            }));
        }

        wc3_roster::RosterPanel::new(header, list)
            .overlay(
                div()
                    .absolute()
                    .right(px(4.0))
                    .top(px(4.0))
                    .size(px(1.0))
                    .opacity(0.001)
                    .child(roster_input.element()),
            )
            .focused(focused)
            .on_hover(cx.listener(|host, hovered, window, cx| {
                host.wc3_roster_pointer(*hovered, window, cx);
            }))
    }
}

/// one selectable member row wired to the host.
fn roster_row<C: AnimationClock, H: HallHost<C>>(
    member: &RosterUser,
    selected: bool,
    cx: &mut Context<H>,
) -> wc3_roster::RosterRow {
    let handle = member.handle;
    let group = format!("wc3-roster-member-{handle}");
    wc3_roster::RosterRow::new(
        format!("wc3-roster-member-{handle}"),
        group,
        member.clone(),
        selected,
    )
    .on_click(cx.listener(move |host, _, window, cx| {
        host.wc3_focus_roster(window, cx);
        if let Some(hall) = host.wc3_hall_mut() {
            hall.set_selected_member(Some(handle));
        }
        cx.stop_propagation();
        cx.notify();
    }))
}

/// what the hall needs from whichever host is drawing it: a way to reach the
/// hall for a callback, and the focus transitions that are the host's own (the
/// composer it sits beside, or the absence of one, is the host's).
pub trait HallHost<C: AnimationClock>: Render + Sized {
    fn wc3_hall(&self) -> Option<&Hall<C>>;
    fn wc3_hall_mut(&mut self) -> Option<&mut Hall<C>>;
    /// give the roster focus. A host with a composer also parks it.
    fn wc3_focus_roster(&mut self, window: &mut Window, cx: &mut Context<Self>);
    /// pointer entered or left the roster; the host decides whether to take
    /// focus, since only it knows if a dialog is up.
    fn wc3_roster_pointer(&mut self, hovered: bool, window: &mut Window, cx: &mut Context<Self>);
    /// clear the roster filter (and the input backing it, if the host has one).
    fn wc3_clear_roster_filter(&mut self, cx: &mut Context<Self>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{ImageSource, Resource};

    fn member(handle: u64, name: &str, presence: RosterPresence, clan: Option<&str>) -> RosterUser {
        RosterUser {
            handle,
            name: name.to_owned(),
            presence,
            portrait: ImageSource::Resource(Resource::Embedded(
                "products/wc3/portraits/p003.png".into(),
            )),
            clan_abbreviation: clan.map(str::to_owned),
        }
    }

    #[test]
    fn roster_order_keeps_present_people_before_absent_people() {
        let members = vec![
            member(1, "Aaron", RosterPresence::Away, None),
            member(2, "Zulu", RosterPresence::Online, None),
            member(3, "Beta", RosterPresence::Offline, None),
            member(4, "Alpha", RosterPresence::Busy, None),
        ];
        let ordered = Hall::<f64>::filtered(&members, "");
        assert_eq!(
            ordered.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            vec!["Alpha", "Zulu", "Aaron", "Beta"]
        );
    }

    #[test]
    fn filter_matches_name_or_clan_abbreviation() {
        let members = vec![
            member(1, "Arthas", RosterPresence::Online, None),
            member(2, "Jaina", RosterPresence::Online, Some("KT")),
            member(3, "Thrall", RosterPresence::Online, Some("HORDE")),
        ];
        let by_clan = Hall::<f64>::filtered(&members, "kt");
        assert_eq!(
            by_clan.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            vec!["Jaina"]
        );
        let by_name = Hall::<f64>::filtered(&members, "art");
        assert_eq!(
            by_name.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            vec!["Arthas"]
        );
    }
}
