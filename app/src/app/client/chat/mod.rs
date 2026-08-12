use super::*;

mod model;
mod state;
mod view;

pub(in crate::app::client) use model::{ChatEntryReveal, ChatLine, shared_transcript_line};

pub(in crate::app::client) const CHAT_BOTTOM_TOLERANCE: f32 = 6.0;
pub(in crate::app::client) const CHAT_ENTRY_REVEAL_DURATION: Duration = Duration::from_millis(350);

pub(super) struct ChatComponent {
    pub(super) transcript: ui_workspace::TranscriptState,
}

impl ChatComponent {
    fn transcript_line(
        &self,
        scope: u64,
        channel_title: &str,
        index: usize,
        line: &ChatLine,
        show_membership: bool,
        show_timestamps: bool,
        assets: &UiAssets,
    ) -> Option<Stateful<Div>> {
        if matches!(line, ChatLine::Membership { .. }) && !show_membership {
            return None;
        }
        let mut row = ui_chat::selectable_transcript_row(
            &shared_transcript_line(line),
            show_timestamps,
            scope,
            index,
            &self.transcript.selection,
        );
        if let ChatLine::Message { sender, .. } = line {
            let tooltip_user = shared_roster_user(sender, assets);
            let tooltip_channel = channel_title.to_owned();
            let tooltip_assets = assets.clone();
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
        Some(row)
    }

    pub(super) fn transcript_view(
        &self,
        channel: Option<&ChannelState>,
        scrollable: bool,
        show_membership: bool,
        show_timestamps: bool,
        reveal: Option<&ChatEntryReveal>,
        assets: &UiAssets,
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
            for (index, line) in channel.transcript.iter().enumerate() {
                if let Some(line) = self.transcript_line(
                    channel.id,
                    &channel.title,
                    index,
                    line,
                    show_membership,
                    show_timestamps,
                    assets,
                ) {
                    let opacity = reveal.map_or(1.0, |reveal| {
                        if reveal.tab_id != channel.id || reveal.line_index != index {
                            return 1.0;
                        }
                        ease_in_out(
                            (now.saturating_duration_since(reveal.started).as_secs_f32()
                                / CHAT_ENTRY_REVEAL_DURATION.as_secs_f32())
                            .clamp(0.0, 1.0),
                        )
                    });
                    transcript = transcript.child(line.opacity(opacity));
                }
            }
        } else {
            transcript = transcript.empty("Join a channel with + to begin chatting.");
        }
        transcript
    }
}
