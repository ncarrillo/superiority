use super::*;

mod model;
mod state;
mod view;

pub(in crate::app::client) use model::{
    ChatEntryReveal, ChatLine, DIGEST_WINDOW, HIGH_TRAFFIC_ONLINE, MEMBERSHIP_COLLAPSE_WINDOW,
    freshest_member, shared_transcript_line,
};

pub(in crate::app::client) const CHAT_BOTTOM_TOLERANCE: f32 = 6.0;
pub(in crate::app::client) const CHAT_ENTRY_REVEAL_DURATION: Duration = Duration::from_millis(350);
/// channel events arrive on their own schedule rather than the reader's, so
/// they fade in faster than a message does — and never make a sound.
pub(in crate::app::client) const CHAT_EVENT_REVEAL_DURATION: Duration = Duration::from_millis(180);
/// how long a digest takes to open or close.
pub(in crate::app::client) const EVENT_TOGGLE_DURATION: Duration = Duration::from_millis(180);

/// the one digest showing its names. holding this here rather than on the
/// transcript line is what makes "one at a time" true by construction, and
/// gives that digest scroll handles nobody else can move.
pub(in crate::app::client) struct ExpandedDigest {
    pub(in crate::app::client) tab_id: u64,
    pub(in crate::app::client) line_index: usize,
    pub(in crate::app::client) started: Instant,
    pub(in crate::app::client) closing: bool,
    pub(in crate::app::client) joined: ScrollHandle,
    pub(in crate::app::client) left: ScrollHandle,
}

impl ExpandedDigest {
    fn covers(&self, tab_id: u64, line_index: usize) -> bool {
        self.tab_id == tab_id && self.line_index == line_index
    }

    /// 0 through 1: how far open the columns are right now.
    fn progress(&self, now: Instant) -> f32 {
        let elapsed = ease_in_out(
            (now.saturating_duration_since(self.started).as_secs_f32()
                / EVENT_TOGGLE_DURATION.as_secs_f32())
            .clamp(0.0, 1.0),
        );
        if self.closing { 1.0 - elapsed } else { elapsed }
    }

    pub(in crate::app::client) fn is_running(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) < EVENT_TOGGLE_DURATION
    }

    pub(in crate::app::client) fn is_finished_closing(&self, now: Instant) -> bool {
        self.closing && !self.is_running(now)
    }
}

pub(super) struct ChatComponent {
    pub(super) transcript: ui_workspace::TranscriptState,
    pub(super) expanded_digest: Option<ExpandedDigest>,
}

/// the parts of a transcript render that are the same for every channel on
/// screen: the reader's preferences, who people are to them, and the art.
pub(in crate::app::client) struct TranscriptChrome<'a> {
    pub(in crate::app::client) show_membership: bool,
    pub(in crate::app::client) show_timestamps: bool,
    pub(in crate::app::client) affinity: &'a RosterAffinity,
    pub(in crate::app::client) assets: &'a UiAssets,
}

/// everything a transcript row needs to know about the channel it is being
/// drawn for. passed as one piece because events resolve people, tone, and
/// portraits against live state rather than against what they were born with.
struct LineContext<'a> {
    scope: u64,
    channel_title: &'a str,
    online: usize,
    roster: &'a [UiUser],
    affinity: &'a RosterAffinity,
    show_membership: bool,
    show_timestamps: bool,
    assets: &'a UiAssets,
    expanded: Option<&'a ExpandedDigest>,
    now: Instant,
}

impl LineContext<'_> {
    fn shared(&self, line: &ChatLine) -> TranscriptLine {
        shared_transcript_line(
            line,
            self.online,
            self.roster,
            Some(self.affinity),
            self.assets,
        )
    }
}

impl ChatComponent {
    fn transcript_line(
        &self,
        context: &LineContext<'_>,
        index: usize,
        line: &ChatLine,
        cx: &mut Context<SuperiorityView>,
    ) -> Option<AnyElement> {
        if matches!(line, ChatLine::Membership { .. } | ChatLine::Digest { .. })
            && !context.show_membership
        {
            return None;
        }
        // channel events are chrome rather than conversation — they render as
        // rules and chips instead of going through the selectable text path.
        match line {
            ChatLine::SessionStart { time, channel } => {
                return Some(
                    ui_chat::session_start_row(
                        time,
                        channel,
                        context.online,
                        context.show_timestamps,
                    )
                    .into_any_element(),
                );
            }
            ChatLine::Digest { joined, .. } => {
                return Self::digest_rows(context, index, line, joined, cx);
            }
            ChatLine::Membership { .. } => {
                return Self::membership_row(context, index, line, cx);
            }
            _ => {}
        }
        let mut row = ui_chat::selectable_transcript_row(
            &context.shared(line),
            context.show_timestamps,
            context.scope,
            index,
            &self.transcript.selection,
            context.assets,
        );
        if let ChatLine::Message { sender, .. } = line {
            let tooltip_user = shared_roster_user(sender, context.assets);
            let tooltip_channel = context.channel_title.to_owned();
            let tooltip_assets = context.assets.clone();
            row = row.tooltip(move |_, cx| {
                cx.new(|_| {
                    ui_roster::RosterTooltip::new(
                        tooltip_user.clone(),
                        tooltip_channel.clone(),
                        tooltip_assets.clone(),
                    )
                })
                .into()
            });
        }
        Some(row.into_any_element())
    }

    fn membership_row(
        context: &LineContext<'_>,
        index: usize,
        line: &ChatLine,
        cx: &mut Context<SuperiorityView>,
    ) -> Option<AnyElement> {
        let TranscriptLine::Membership(event) = context.shared(line) else {
            return None;
        };
        let tab = context.scope;
        Some(
            ui_chat::MembershipRow::new(
                ("transcript-event", index),
                event,
                context.channel_title.to_owned(),
                context.assets.clone(),
                context.show_timestamps,
            )
            .on_member({
                let select = cx.listener(move |this, member: &usize, _, cx| {
                    this.select_event_member(tab, index, *member);
                    cx.notify();
                });
                move |member, window, cx| select(&member, window, cx)
            })
            .on_toggle({
                let toggle = cx.listener(move |this, (): &(), _, cx| {
                    this.toggle_event_expansion(tab, index);
                    cx.notify();
                });
                move |window, cx| toggle(&(), window, cx)
            })
            .into_any_element(),
        )
    }

    /// a digest minute, with the arrivals worth caring about lifted out of it.
    /// the break-out is decided here rather than when the events landed, so a
    /// clan tag or friendship that resolves late still counts.
    fn digest_rows(
        context: &LineContext<'_>,
        index: usize,
        line: &ChatLine,
        joined: &[UiUser],
        cx: &mut Context<SuperiorityView>,
    ) -> Option<AnyElement> {
        let TranscriptLine::Digest(mut event) = context.shared(line) else {
            return None;
        };
        let tab = context.scope;
        let breakout = joined
            .iter()
            .enumerate()
            .filter(|(_, member)| {
                context
                    .affinity
                    .notable(freshest_member(member, context.roster))
            })
            .map(|(position, _)| position)
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for position in breakout.iter().copied() {
            rows.push(
                ui_chat::MembershipRow::new(
                    ("transcript-breakout", index * 64 + position),
                    MembershipEvent {
                        time: event.time.clone(),
                        kind: MembershipKind::Joined,
                        members: vec![event.joined[position].clone()],
                        expanded: false,
                    },
                    context.channel_title.to_owned(),
                    context.assets.clone(),
                    context.show_timestamps,
                )
                .on_member({
                    let select = cx.listener(move |this, _: &usize, _, cx| {
                        this.select_digest_member(tab, index, position);
                        cx.notify();
                    });
                    move |member, window, cx| select(&member, window, cx)
                })
                .into_any_element(),
            );
        }
        for position in breakout.iter().rev() {
            event.joined.remove(*position);
        }
        if !event.joined.is_empty() || !event.left.is_empty() {
            // only the one open digest is handed the scroll handles and a
            // non-zero expansion; every other one renders collapsed.
            let expanded = context
                .expanded
                .filter(|expanded| expanded.covers(tab, index));
            let expansion = expanded.map_or(0.0, |expanded| expanded.progress(context.now));
            let fallback = ScrollHandle::default();
            let (joined_scroll, left_scroll) = expanded
                .map_or((fallback.clone(), fallback), |expanded| {
                    (expanded.joined.clone(), expanded.left.clone())
                });
            rows.push(
                ui_chat::DigestRow::new(
                    ("transcript-digest", index),
                    event,
                    context.channel_title.to_owned(),
                    context.assets.clone(),
                    context.show_timestamps,
                )
                .expansion(expansion)
                .columns(&joined_scroll, &left_scroll)
                .on_member({
                    let select = cx.listener(move |this, member: &usize, _, cx| {
                        this.select_digest_member(tab, index, *member);
                        cx.notify();
                    });
                    move |member, window, cx| select(&member, window, cx)
                })
                .on_toggle({
                    let toggle = cx.listener(move |this, (): &(), _, cx| {
                        this.toggle_digest_expansion(tab, index);
                        cx.notify();
                    });
                    move |window, cx| toggle(&(), window, cx)
                })
                .into_any_element(),
            );
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(5.0))
                .children(rows)
                .into_any_element(),
        )
    }

    pub(super) fn transcript_view(
        &self,
        channel: Option<&ChannelState>,
        scrollable: bool,
        reveal: Option<&ChatEntryReveal>,
        chrome: &TranscriptChrome<'_>,
        cx: &mut Context<SuperiorityView>,
    ) -> ui_chat::TranscriptViewport {
        let now = Instant::now();
        let mut transcript = ui_chat::TranscriptViewport::new(
            if scrollable {
                "chat-transcript-scroll"
            } else {
                "chat-transcript-transition"
            },
            self.transcript.selection.clone(),
        );
        if scrollable {
            transcript = transcript.scroll(&self.transcript.scroll);
        }
        if let Some(channel) = channel {
            let context = LineContext {
                scope: channel.id,
                channel_title: &channel.title,
                online: channel.users.len(),
                roster: &channel.users,
                affinity: chrome.affinity,
                show_membership: chrome.show_membership,
                show_timestamps: chrome.show_timestamps,
                assets: chrome.assets,
                expanded: self.expanded_digest.as_ref(),
                now,
            };
            for (index, line) in channel.transcript.iter().enumerate() {
                let event = matches!(
                    line,
                    ChatLine::Membership { .. }
                        | ChatLine::Digest { .. }
                        | ChatLine::SessionStart { .. }
                );
                if let Some(line) = self.transcript_line(&context, index, line, cx) {
                    // no crossfade on the row itself: a digest opening or
                    // closing grows its own columns, and fading the whole line
                    // would flash the counts that never went anywhere.
                    let opacity = reveal.map_or(1.0, |reveal| {
                        if reveal.tab_id != channel.id || reveal.line_index != index {
                            return 1.0;
                        }
                        let duration = if event {
                            CHAT_EVENT_REVEAL_DURATION
                        } else {
                            CHAT_ENTRY_REVEAL_DURATION
                        };
                        ease_in_out(
                            (now.saturating_duration_since(reveal.started).as_secs_f32()
                                / duration.as_secs_f32())
                            .clamp(0.0, 1.0),
                        )
                    });
                    transcript = transcript.child(div().opacity(opacity).child(line));
                }
            }
        } else {
            transcript = transcript.empty("Join a channel with + to begin chatting.");
        }
        transcript
    }
}
