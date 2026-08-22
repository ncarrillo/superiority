//! The Remastered console's presentation state, policy, and assembly.
//!
//! What a host had to write for itself before this existed: the transcript's
//! reveal and copy, the member list's filtering, ordering, selection,
//! virtualization and join/leave animation, and the header's counts. The
//! desktop and Superiority Live each wrote it against the same renderers in
//! [`super::components`], and the second copy drifted.
//!
//! This owns all of it over the neutral presentation models
//! ([`super::RosterUser`], [`super::TranscriptLine`]). A host adapts its
//! protocol or DTO values into those, hands them in, and supplies only the
//! focus behaviour and the text input that are genuinely its own. It is generic
//! over [`AnimationClock`] so the desktop drives it with `Instant` and the
//! browser with `f64` milliseconds.

use std::ops::Range;

use gpui::{
    Context, MouseButton, Render, ScrollStrategy, Window, div, ease_in_out, prelude::*, px,
    uniform_list,
};

use crate::{
    foundation::{WithScrollbar as _, animation::AnimationClock, text_input::TextInput},
    patterns::{roster as roster_pattern, transcript::TranscriptSelection, workspace::RosterState},
    products::scr::{
        RosterPresence, RosterUser, TranscriptLine,
        components::{chat as scr_chat, roster as scr_roster},
        theme::{ROSTER_ROW_GAP, ROSTER_ROW_HEIGHT},
    },
};

/// One transcript line revealing itself as it lands.
#[derive(Clone, Copy)]
struct Reveal<C> {
    channel_id: u32,
    line_index: usize,
    started: C,
}

/// The channel the console is in.
struct ConsoleChannel {
    id: u32,
    title: String,
    transcript: Vec<TranscriptLine>,
    members: Vec<RosterUser>,
    filter: String,
}

/// The transcript history bound; older lines fall off the top.
const MAX_TRANSCRIPT_LINES: usize = 2_000;
const TRANSCRIPT_DRAIN: usize = 500;

/// The Remastered console's whole presentation, minus the composer and window
/// chrome, which stay with the host.
pub struct Console<C: AnimationClock> {
    channel: Option<ConsoleChannel>,
    transcript: scr_chat::TranscriptState,
    roster: RosterState<u32, RosterUser, C>,
    reveal: Option<Reveal<C>>,
}

impl<C: AnimationClock> Default for Console<C> {
    fn default() -> Self {
        Self {
            channel: None,
            transcript: scr_chat::TranscriptState::default(),
            roster: RosterState::default(),
            reveal: None,
        }
    }
}

impl<C: AnimationClock> Console<C> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn has_channel(&self) -> bool {
        self.channel.is_some()
    }

    #[must_use]
    pub fn channel_id(&self) -> Option<u32> {
        self.channel.as_ref().map(|channel| channel.id)
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.channel.as_ref().map(|channel| channel.title.as_str())
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

    /// A clone of the shared transcript selection, for the host's copy keystroke.
    #[must_use]
    pub fn transcript_selection(&self) -> TranscriptSelection {
        self.transcript.selection.clone()
    }

    pub fn clear_transcript_selection(&self) {
        self.transcript.selection.clear();
    }

    /// The transcript lines as `(row, text)`, for the copy keystroke.
    #[must_use]
    pub fn transcript_rows_text(&self) -> Vec<(usize, String)> {
        self.channel
            .as_ref()
            .map(|channel| {
                channel
                    .transcript
                    .iter()
                    .enumerate()
                    .map(|(index, line)| (index, scr_chat::transcript_text(line)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resets to no channel — a fresh sign-in, a sign-out, or an account switch.
    pub fn clear(&mut self) {
        self.roster.clear_interaction();
        self.roster.selections.clear();
        self.transcript.selection.clear();
        self.reveal = None;
        self.channel = None;
    }

    /// Installs the channel's title and membership, animating the roster if it
    /// is the same channel and resetting the view if it is a new one. Members
    /// arrive already resolved into [`RosterUser`]s by the host's adapter.
    /// `session_time` seeds the "you joined" line when the channel is new.
    pub fn apply_channel(
        &mut self,
        id: u32,
        title: String,
        members: Vec<RosterUser>,
        session_time: String,
        now: C,
    ) {
        if self
            .channel
            .as_ref()
            .is_some_and(|channel| channel.id == id)
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
                .begin_transition(id, previous, &next, now, |member| member.handle);
            let channel = self.channel.as_mut().expect("channel present");
            channel.title = title;
            channel.members = members;
            if self.roster.selection(&id).is_some_and(|selected| {
                !channel
                    .members
                    .iter()
                    .any(|member| member.handle == selected)
            }) {
                self.roster.set_selection(id, None);
            }
            return;
        }
        self.roster.clear_interaction();
        self.roster.set_selection(id, None);
        self.roster
            .retain_selections(|channel_id| *channel_id == id);
        self.transcript.selection.clear();
        self.transcript.scroll.scroll_to_bottom();
        self.reveal = None;
        let online = members.len();
        self.channel = Some(ConsoleChannel {
            id,
            title: title.clone(),
            transcript: vec![TranscriptLine::SessionStart {
                time: session_time,
                channel: title,
                online,
            }],
            members,
            filter: String::new(),
        });
    }

    /// Appends one transcript line, revealing it and following the bottom when
    /// the reader is already there.
    pub fn append_line(&mut self, line: TranscriptLine, now: C) {
        let follows_bottom = self.transcript_follows_bottom();
        let Some(channel) = self.channel.as_mut() else {
            return;
        };
        channel.transcript.push(line);
        if channel.transcript.len() > MAX_TRANSCRIPT_LINES {
            channel.transcript.drain(..TRANSCRIPT_DRAIN);
        }
        self.reveal = Some(Reveal {
            channel_id: channel.id,
            line_index: channel.transcript.len().saturating_sub(1),
            started: now,
        });
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
    pub fn selected_member(&self) -> Option<u32> {
        let id = self.channel.as_ref()?.id;
        self.roster.selection(&id)
    }

    pub fn set_selected_member(&mut self, selected: Option<u32>) {
        let Some(channel) = self.channel.as_ref() else {
            return;
        };
        let id = channel.id;
        let selected = selected.filter(|handle| {
            channel
                .members
                .iter()
                .any(|member| member.handle == *handle)
        });
        self.roster.set_selection(id, selected);
    }

    /// Narrows the roster, animating the change and dropping a hidden selection.
    pub fn set_filter(&mut self, next: String, now: C) {
        let Some(channel) = self.channel.as_ref() else {
            return;
        };
        if channel.filter == next {
            return;
        }
        let id = channel.id;
        let previous = Self::filtered(&channel.members, &channel.filter);
        let next_members = Self::filtered(&channel.members, &next);
        self.channel.as_mut().expect("channel present").filter = next;
        if self
            .roster
            .selection(&id)
            .is_some_and(|selected| !next_members.iter().any(|member| member.handle == selected))
        {
            self.roster.set_selection(id, None);
        }
        self.roster
            .begin_transition(id, previous, &next_members, now, |member| member.handle);
    }

    /// The membership after the filter, present-first then away, alphabetical
    /// within each — the order the list draws.
    #[must_use]
    pub fn visible_members(&self) -> Vec<RosterUser> {
        self.channel
            .as_ref()
            .map(|channel| Self::filtered(&channel.members, &channel.filter))
            .unwrap_or_default()
    }

    fn filtered(members: &[RosterUser], filter: &str) -> Vec<RosterUser> {
        let mut members = roster_pattern::filtered_refs(members, filter, |member, query| {
            roster_pattern::filter_matches(&member.name, query)
        })
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
        members.sort_by(|left, right| {
            roster_pattern::present_before_absent(
                left.presence == RosterPresence::Away,
                &left.name,
                right.presence == RosterPresence::Away,
                &right.name,
            )
        });
        members
    }

    pub fn move_selection(&mut self, delta: isize) -> bool {
        let Some(id) = self.channel_id() else {
            return false;
        };
        let handles = self
            .visible_members()
            .into_iter()
            .map(|member| member.handle)
            .collect::<Vec<_>>();
        let positions = (1..=handles.len()).collect::<Vec<_>>();
        self.roster
            .move_selection(id, &handles, &positions, delta, ScrollStrategy::Center)
            .is_some()
    }

    pub fn select_index(&mut self, index: usize) -> bool {
        let Some(id) = self.channel_id() else {
            return false;
        };
        let handles = self
            .visible_members()
            .into_iter()
            .map(|member| member.handle)
            .collect::<Vec<_>>();
        let positions = (1..=handles.len()).collect::<Vec<_>>();
        self.roster
            .select_index(id, &handles, &positions, index, ScrollStrategy::Center)
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
            || self
                .reveal
                .as_ref()
                .is_some_and(|reveal| now.elapsed(reveal.started) < super::CHAT_ENTRY_REVEAL)
    }

    pub fn finish_animations(&mut self, now: C) {
        self.roster.finish_transition(now);
    }

    /// The transcript, with each line's reveal opacity applied. No host
    /// interaction rides on the transcript — selection is carried by the shared
    /// handle inside the rows — so this needs nothing from the host.
    #[must_use]
    pub fn transcript_viewport(&self, now: C) -> scr_chat::TranscriptViewport {
        let mut transcript = scr_chat::TranscriptViewport::new(
            "scr-chat-transcript",
            self.transcript.selection.clone(),
        )
        .scroll(&self.transcript.scroll);
        let Some(channel) = self.channel.as_ref() else {
            return transcript.empty("JOIN A CHANNEL TO BEGIN CHATTING");
        };
        for (index, line) in channel.transcript.iter().enumerate() {
            let opacity = self.reveal.as_ref().map_or(1.0, |reveal| {
                if reveal.channel_id != channel.id || reveal.line_index != index {
                    return 1.0;
                }
                ease_in_out(
                    (now.elapsed(reveal.started).as_secs_f32()
                        / super::CHAT_ENTRY_REVEAL.as_secs_f32())
                    .clamp(0.0, 1.0),
                )
            });
            transcript = transcript.child(div().opacity(opacity).child(
                scr_chat::selectable_transcript_row(
                    line,
                    u64::from(channel.id),
                    index,
                    &self.transcript.selection,
                ),
            ));
        }
        transcript
    }

    /// The whole member-list panel, wired to the host. The one place a
    /// [`ConsoleHost`] is required, because every row and the header carry a
    /// callback that mutates the host.
    #[must_use]
    pub fn roster_panel<H: ConsoleHost<C>>(
        &self,
        roster_input: TextInput,
        now: C,
        window: &mut Window,
        cx: &mut Context<H>,
    ) -> scr_roster::RosterPanel {
        let focused = self.roster.focused;
        let title = self.title().unwrap_or("No channel").to_owned();
        let total = self.member_count();
        let filter = self.filter().to_owned();
        let filtered = self.visible_members().len();
        let channel_id = self.channel_id().unwrap_or(u32::MAX);
        let selected = self.selected_member();
        let base_scroll = self.roster.scroll.0.borrow().base_handle.clone();
        let scroll = self.roster.scroll.clone();

        let animating = self.channel_id().is_some_and(|id| {
            self.roster
                .animation
                .as_ref()
                .is_some_and(|animation| animation.scope == id && animation.is_running(now))
        });

        let mut list = scr_roster::list_layer("scr-roster-list")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|host, _, window, cx| host.scr_focus_roster(window, cx)),
            )
            .on_click(cx.listener(|host, _, _, cx| {
                if let Some(console) = host.scr_console_mut() {
                    console.set_selected_member(None);
                }
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(|host, _, _, cx| {
                if host
                    .scr_console_mut()
                    .is_some_and(Console::take_roster_animation)
                {
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
                        scr_roster::RosterRow::new(
                            format!("scr-roster-member-removed-{}", member.handle),
                            format!("scr-roster-member-removed-{}", member.handle),
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
                    scr_roster::segment_header(filtered),
                    ROSTER_ROW_HEIGHT,
                    ROSTER_ROW_GAP,
                ))
                .children(rows)
        } else {
            let item_count = usize::from(self.has_channel()) + filtered;
            list.child(
                uniform_list(
                    "scr-roster-users",
                    item_count,
                    cx.processor(move |host: &mut H, range: Range<usize>, _, cx| {
                        let (members, selected) = host.scr_console().map_or_else(
                            || (Vec::new(), None),
                            |console| (console.visible_members(), console.selected_member()),
                        );
                        range
                            .filter_map(|index| {
                                let row = if index == 0 {
                                    scr_roster::segment_header(members.len()).into_any_element()
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
            crate::products::modal::ModalVariant::Remastered.scrollbar(),
            window,
            cx,
        );

        let model = scr_roster::RosterHeaderModel::new(title, total, filtered, &filter, focused);
        let mut header = scr_roster::RosterHeader::new(
            format!("scr-roster-header-{channel_id}"),
            model,
            focused,
        )
        .on_focus(cx.listener(|host, _, window, cx| host.scr_focus_roster(window, cx)));
        if !filter.is_empty() {
            header = header.on_clear(cx.listener(|host, _, _, cx| {
                host.scr_clear_roster_filter(cx);
                cx.stop_propagation();
                cx.notify();
            }));
        }

        scr_roster::RosterPanel::new(header, list)
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
                host.scr_roster_pointer(*hovered, window, cx);
            }))
    }
}

/// One selectable member row wired to the host.
fn roster_row<C: AnimationClock, H: ConsoleHost<C>>(
    member: &RosterUser,
    selected: bool,
    cx: &mut Context<H>,
) -> scr_roster::RosterRow {
    let handle = member.handle;
    let group = format!("scr-roster-member-{handle}");
    scr_roster::RosterRow::new(
        format!("scr-roster-member-{handle}"),
        group,
        member.clone(),
        selected,
    )
    .on_click(cx.listener(move |host, _, window, cx| {
        host.scr_focus_roster(window, cx);
        if let Some(console) = host.scr_console_mut() {
            console.set_selected_member(Some(handle));
        }
        cx.stop_propagation();
        cx.notify();
    }))
}

/// What the console needs from whichever host is drawing it: a way to reach the
/// console for a callback, and the focus transitions that are the host's own
/// (the composer it sits beside, or the absence of one, is the host's).
pub trait ConsoleHost<C: AnimationClock>: Render + Sized {
    fn scr_console(&self) -> Option<&Console<C>>;
    fn scr_console_mut(&mut self) -> Option<&mut Console<C>>;
    /// Give the roster focus. A host with a composer also parks it.
    fn scr_focus_roster(&mut self, window: &mut Window, cx: &mut Context<Self>);
    /// Pointer entered or left the roster; the host decides whether to take
    /// focus, since only it knows if a dialog is up.
    fn scr_roster_pointer(&mut self, hovered: bool, window: &mut Window, cx: &mut Context<Self>);
    /// Clear the roster filter (and the input backing it, if the host has one).
    fn scr_clear_roster_filter(&mut self, cx: &mut Context<Self>);
}
