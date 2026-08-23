use super::*;

/// the invitation row: the party's green, at the height of a message with a
/// face on it.
const INVITATION_ROW_HEIGHT: f32 = 40.0;
const INVITATION_PORTRAIT: f32 = 26.0;
const INVITATION_FILL: u32 = 0x030c_08eb;
const INVITATION_BORDER: u32 = 0x47d1_8473;
const INVITATION_ACCENT: u32 = 0x0047_d184;

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

pub(in crate::app::client) struct ChatComponent {
    pub(in crate::app::client) transcript: ui_workspace::TranscriptState,
    pub(in crate::app::client) expanded_digest: Option<ExpandedDigest>,
}

/// the parts of a transcript render that are the same for every channel on
/// screen: the reader's preferences, who people are to them, and the art.
pub(in crate::app::client) struct TranscriptChrome<'a> {
    pub(in crate::app::client) show_membership: bool,
    pub(in crate::app::client) show_timestamps: bool,
    pub(in crate::app::client) affinity: &'a RosterAffinity,
    pub(in crate::app::client) assets: &'a Sc2Assets,
}

/// everything a transcript row needs to know about the channel it is being
/// drawn for. passed as one piece because events resolve people, tone, and
/// portraits against live state rather than against what they were born with.
struct LineContext<'a> {
    scope: u64,
    channel_title: &'a str,
    /// the lines around this one, so a run of party talk from one speaker can
    /// be told from the start of a new one.
    transcript: &'a [ChatLine],
    online: usize,
    roster: &'a [UiUser],
    affinity: &'a RosterAffinity,
    show_membership: bool,
    show_timestamps: bool,
    assets: &'a Sc2Assets,
    expanded: Option<&'a ExpandedDigest>,
    now: Instant,
}

impl LineContext<'_> {
    fn follows_party_line(&self, index: usize, line: &ChatLine) -> bool {
        follows_party_line(self.transcript, index, line, self.show_membership)
    }

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
    /// the invitation as it arrives: who asked, what to, and the two answers.
    /// it wears the party's green because that is what it is an invitation to,
    /// and it sits in the transcript because declining should cost nothing and
    /// tell nobody.
    fn invitation_row(
        context: &LineContext<'_>,
        id: u64,
        inviter: &str,
        detail: &str,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let portrait = context
            .roster
            .iter()
            .find(|user| user.name == inviter)
            .and_then(|user| user.portrait.clone())
            .map(|portrait| Portrait::Image(portrait.into()));
        div()
            .id(("party-invitation", id as usize))
            .h(px(INVITATION_ROW_HEIGHT))
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(10.0))
            .my(px(2.0))
            .bg(rgba(INVITATION_FILL))
            .border_1()
            .border_color(rgba(INVITATION_BORDER))
            .rounded(px(2.0))
            .child(ui_roster::framed_portrait(
                portrait.as_ref(),
                context.assets,
                INVITATION_PORTRAIT,
                INVITATION_PORTRAIT - 4.0,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .gap(px(5.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(12.5))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(rgb(0x00f0_aa64))
                            .child(inviter.to_owned()),
                    )
                    .child(div().text_color(rgb(TEXT)).child(detail.to_owned())),
            )
            .child(invitation_answer(id, true, cx))
            .child(invitation_answer(id, false, cx))
            .into_any_element()
    }

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
            // an unanswered invitation is a thing to act on rather than read,
            // so it draws its own row; once answered it settles into the plain
            // line the shared transcript makes of it
            ChatLine::Invitation {
                id,
                inviter,
                detail,
                answered: None,
                ..
            } => {
                return Some(Self::invitation_row(context, *id, inviter, detail, cx));
            }
            _ => {}
        }
        let shared = context.shared(line);
        let rail = ui_chat::party_line(&shared);
        let mut row = ui_chat::selectable_transcript_row(
            &shared,
            context.show_timestamps,
            context.scope,
            index,
            &self.transcript.selection,
            context.assets,
            ui_chat::PartyMark {
                rail,
                chip: rail && !context.follows_party_line(index, line),
            },
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

    /// one channel's transcript, scrolled to wherever that channel's own
    /// handle says. `current` is the live layer; the other is the outgoing
    /// side of a tab crossfade, which draws at its own offset too — with one
    /// shared handle both layers jumped to the same place mid-fade.
    pub(in crate::app::client) fn transcript_view(
        &self,
        channel: Option<&ChannelState>,
        current: bool,
        reveal: Option<&ChatEntryReveal>,
        chrome: &TranscriptChrome<'_>,
        cx: &mut Context<SuperiorityView>,
    ) -> ui_chat::TranscriptViewport {
        let now = Instant::now();
        let mut transcript = ui_chat::TranscriptViewport::new(
            if current {
                "chat-transcript-scroll"
            } else {
                "chat-transcript-transition"
            },
            self.transcript.selection.clone(),
        );
        if let Some(channel) = channel {
            transcript = transcript.scroll(&channel.scroll);
        }
        if let Some(channel) = channel {
            let context = LineContext {
                scope: channel.id,
                channel_title: &channel.title,
                transcript: &channel.transcript,
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

/// the two answers. accepting is the loud one and wears the party's fill;
/// declining is a quiet outline, because it is the answer that tells nobody.
fn invitation_answer(
    id: u64,
    accept: bool,
    cx: &mut Context<SuperiorityView>,
) -> gpui::Stateful<Div> {
    let mut button = div()
        .id((
            "party-invitation-answer",
            id as usize * 2 + usize::from(accept),
        ))
        .flex_shrink_0()
        .px(px(9.0))
        .py(px(4.0))
        .rounded(px(2.0))
        .font_family(FONT_NAVIGATION)
        .font_weight(FontWeight::BOLD)
        .text_size(px(9.5))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, window, cx| {
            this.answer_invitation(id, accept, window, cx);
        }));
    if accept {
        button = button
            .bg(rgb(INVITATION_ACCENT))
            .text_color(rgb(0x000a_1f12))
            .hover(|style| style.bg(rgb(0x006b_e8a4)))
            .child("ACCEPT");
    } else {
        button = button
            .border_1()
            .border_color(rgba(INVITATION_BORDER))
            .text_color(rgb(MUTED))
            .hover(|style| style.text_color(rgb(TEXT)))
            .child("DECLINE");
    }
    button
}

/// whether the line above this one was the same person talking to the party. a
/// run from one speaker wears the chip once — repeating it down a back-and-forth
/// says nothing new. lines the reader has switched off are stepped over, since
/// the run is what they can see.
pub(in crate::app::client) fn follows_party_line(
    transcript: &[ChatLine],
    index: usize,
    line: &ChatLine,
    show_membership: bool,
) -> bool {
    let ChatLine::Message { sender, .. } = line else {
        return false;
    };
    if sender.tone != RosterUserTone::Party {
        return false;
    }
    let mut cursor = index.min(transcript.len());
    while cursor > 0 {
        cursor -= 1;
        let previous = &transcript[cursor];
        if !show_membership
            && matches!(
                previous,
                ChatLine::Membership { .. } | ChatLine::Digest { .. }
            )
        {
            continue;
        }
        return matches!(
            previous,
            ChatLine::Message { sender: above, .. }
                if above.tone == RosterUserTone::Party && above.handle == sender.handle
        );
    }
    false
}
