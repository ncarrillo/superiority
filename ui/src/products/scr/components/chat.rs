//! SC:R transcript presentation with the same scrolling, selection, and row
//! rhythm as the SC2 transcript, expressed in the Terran-console palette.

use std::ops::Range;

use gpui::{
    AnyElement, App, Div, ElementId, HighlightStyle, IntoElement, ObjectFit, RenderOnce,
    ScrollHandle, StyledText, Window, div, img, prelude::*, px, rgb, rgba,
};

use crate::{
    patterns::transcript::{self, SelectableText, TranscriptSelection},
    products::scr::{
        TranscriptLine, TranscriptMessageKind, TranscriptNoticeKind,
        theme::{ACCENT, FONT_INTERFACE, FONT_INTERNATIONAL, MUTED},
    },
};

/// spoken text is ivory so the dim-rust timestamps recede and the message
/// leads; server notices sit a step darker on the same ramp.
const BODY: u32 = 0x00f0_e6da;
const NOTICE: u32 = 0x00b8_aca2;
const ERROR: u32 = 0x00e3_5f4e;
const SELECTION_BACKGROUND: u32 = 0x4a14_0eeb;
const RULE_COLOR: u32 = 0xc93a_2c99;
const RULE_FADE: u32 = 0xc93a_2c00;
const TIME: u32 = 0x005e_564e;
const RULE_TEXT: u32 = 0x008a_7c70;

pub struct TranscriptState {
    pub selection: TranscriptSelection,
    pub scroll: ScrollHandle,
}

impl Default for TranscriptState {
    fn default() -> Self {
        Self {
            selection: TranscriptSelection::default(),
            scroll: ScrollHandle::new(),
        }
    }
}

struct ContentStyle {
    text: String,
    color: u32,
    emphasis: Option<Range<usize>>,
    emphasis_color: u32,
}

fn content_style(line: &TranscriptLine) -> ContentStyle {
    match line {
        TranscriptLine::SessionStart {
            channel, online, ..
        } => ContentStyle {
            text: format!("YOU JOINED {} /// {online} ONLINE", channel.to_uppercase()),
            color: ACCENT,
            emphasis: None,
            emphasis_color: ACCENT,
        },
        TranscriptLine::Message {
            kind, sender, text, ..
        } => match kind {
            TranscriptMessageKind::Emote => ContentStyle {
                text: format!("* {} {text}", sender.name),
                color: BODY,
                emphasis: Some(2..2 + sender.name.len()),
                emphasis_color: ACCENT,
            },
            TranscriptMessageKind::Talk | TranscriptMessageKind::Whisper => {
                let prefix = if *kind == TranscriptMessageKind::Whisper {
                    format!("WHISPER  {}:", sender.name)
                } else {
                    format!("{}:", sender.name)
                };
                let emphasis_start = if *kind == TranscriptMessageKind::Whisper {
                    "WHISPER  ".len()
                } else {
                    0
                };
                ContentStyle {
                    text: format!("{prefix} {text}"),
                    color: BODY,
                    emphasis: Some(emphasis_start..prefix.len()),
                    emphasis_color: ACCENT,
                }
            }
        },
        TranscriptLine::Notice { kind, text, .. } => match kind {
            TranscriptNoticeKind::Broadcast => ContentStyle {
                text: format!("BROADCAST: {text}"),
                color: NOTICE,
                emphasis: Some(0.."BROADCAST:".len()),
                emphasis_color: ACCENT,
            },
            // the notice colour already says the server is speaking; a bold
            // INFO: on every line of the welcome was furniture
            TranscriptNoticeKind::Information => ContentStyle {
                text: text.clone(),
                color: NOTICE,
                emphasis: None,
                emphasis_color: ACCENT,
            },
        },
        TranscriptLine::Error { text, .. } => ContentStyle {
            text: format!("!  {text}"),
            color: ERROR,
            emphasis: Some(0..1),
            emphasis_color: ERROR,
        },
    }
}

fn styled_content(content: &ContentStyle, selection: Option<&Range<usize>>) -> StyledText {
    let mut boundaries = vec![0, content.text.len()];
    for range in [content.emphasis.as_ref(), selection].into_iter().flatten() {
        boundaries.extend([range.start, range.end]);
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    let mut highlights = Vec::new();
    for points in boundaries.windows(2) {
        let range = points[0]..points[1];
        if range.is_empty() {
            continue;
        }
        let mut style = HighlightStyle::default();
        if content
            .emphasis
            .as_ref()
            .is_some_and(|emphasis| emphasis.contains(&range.start))
        {
            style.color = Some(rgb(content.emphasis_color).into());
            style.font_weight = Some(gpui::FontWeight::BOLD);
        }
        if selection.is_some_and(|selection| selection.contains(&range.start)) {
            style.background_color = Some(rgba(SELECTION_BACKGROUND).into());
        }
        if style != HighlightStyle::default() {
            highlights.push((range, style));
        }
    }
    let mut text = StyledText::new(content.text.clone()).with_highlights(highlights);
    if let Some(emphasis) = content.emphasis.clone() {
        text = text.with_font_family_overrides([(emphasis, FONT_INTERFACE.into())]);
    }
    text
}

#[must_use]
pub fn transcript_text(line: &TranscriptLine) -> String {
    content_style(line).text
}

#[must_use]
pub fn selectable_transcript_row(
    line: &TranscriptLine,
    scope: u64,
    row: usize,
    selection: &TranscriptSelection,
) -> AnyElement {
    if let TranscriptLine::SessionStart {
        time,
        channel,
        online,
    } = line
    {
        return session_start_row(time, channel, *online).into_any_element();
    }
    let content = content_style(line);
    let selected = selection.selection_for_row(row, content.text.len());
    let time = line_time(line);
    let text = SelectableText::new(
        format!("scr-transcript-text-{scope}-{row}"),
        row,
        styled_content(&content, selected.as_ref()),
        Vec::new(),
        selection.clone(),
    );
    let portrait = match line {
        TranscriptLine::Message { sender, .. } => Some(sender.portrait.clone()),
        _ => None,
    };
    transcript_row_shell(time, content.color, portrait, text)
        .id(format!("scr-transcript-row-{scope}-{row}"))
        .cursor(gpui::CursorStyle::IBeam)
        .into_any_element()
}

fn line_time(line: &TranscriptLine) -> &str {
    match line {
        TranscriptLine::SessionStart { time, .. }
        | TranscriptLine::Message { time, .. }
        | TranscriptLine::Notice { time, .. }
        | TranscriptLine::Error { time, .. } => time,
    }
}

fn transcript_row_shell(
    time: &str,
    color: u32,
    portrait: Option<gpui::ImageSource>,
    content: impl IntoElement,
) -> Div {
    let portrait = portrait.map(|portrait| {
        div()
            .relative()
            .size(px(transcript::SPEAKER_AVATAR_SIZE))
            .flex_shrink_0()
            .overflow_hidden()
            .child(
                img(portrait)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Contain),
            )
            .into_any_element()
    });
    transcript::row_shell(
        true,
        time_gutter(time),
        Vec::new(),
        portrait,
        FONT_INTERNATIONAL,
        color,
        content,
    )
}

fn time_gutter(time: &str) -> Div {
    transcript::time_gutter(format!("[{time}]"), FONT_INTERFACE, TIME)
}

fn session_start_row(time: &str, channel: &str, online: usize) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(event_rule(true))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(7.0))
                .flex_shrink_0()
                .font_family(FONT_INTERFACE)
                .text_size(px(transcript::ROW_TEXT_SIZE))
                .child(transcript::inline_time(
                    format!("{time} ///"),
                    FONT_INTERFACE,
                    RULE_TEXT,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .text_color(rgb(ACCENT))
                        .child("YOU JOINED")
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(rgb(BODY))
                                .child(channel.to_uppercase()),
                        ),
                )
                .child(
                    div()
                        .text_color(rgb(RULE_TEXT))
                        .child(format!("/// {online} ONLINE")),
                ),
        )
        .child(event_rule(false))
}

fn event_rule(leading: bool) -> Div {
    let (from, to) = if leading {
        (RULE_FADE, RULE_COLOR)
    } else {
        (RULE_COLOR, RULE_FADE)
    };
    div().flex_1().h(px(1.0)).bg(gpui::linear_gradient(
        90.0,
        gpui::linear_color_stop(rgba(from), 0.0),
        gpui::linear_color_stop(rgba(to), 1.0),
    ))
}

#[derive(IntoElement)]
pub struct TranscriptViewport {
    inner: transcript::TranscriptViewport,
}

impl TranscriptViewport {
    #[must_use]
    pub fn new(id: impl Into<ElementId>, selection: TranscriptSelection) -> Self {
        Self {
            inner: transcript::TranscriptViewport::new(id, selection, FONT_INTERNATIONAL)
                .scrollbar_colors(crate::products::modal::ModalVariant::Remastered.scrollbar()),
        }
    }

    #[must_use]
    pub fn scroll(mut self, scroll: &ScrollHandle) -> Self {
        self.inner = self.inner.scroll(scroll);
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    #[must_use]
    pub fn empty(self, message: impl Into<gpui::SharedString>) -> Self {
        self.child(
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .font_family(FONT_INTERFACE)
                .text_size(px(11.0))
                .text_color(rgb(MUTED))
                .child(message.into()),
        )
    }
}

impl RenderOnce for TranscriptViewport {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speaker() -> crate::products::scr::TranscriptSpeaker {
        crate::products::scr::TranscriptSpeaker {
            name: "Raynor".to_owned(),
            portrait: gpui::ImageSource::Resource(gpui::Resource::Embedded(
                "products/scr/avatars/0.png".into(),
            )),
        }
    }

    #[test]
    fn transcript_copy_uses_the_same_words_the_row_displays() {
        let talk = TranscriptLine::Message {
            time: "12:00 AM".to_owned(),
            kind: TranscriptMessageKind::Talk,
            sender: speaker(),
            text: "Need a light?".to_owned(),
        };
        let whisper = TranscriptLine::Message {
            time: "12:01 AM".to_owned(),
            kind: TranscriptMessageKind::Whisper,
            sender: speaker(),
            text: "Meet me in channel.".to_owned(),
        };

        assert_eq!(transcript_text(&talk), "Raynor: Need a light?");
        assert_eq!(
            transcript_text(&whisper),
            "WHISPER  Raynor: Meet me in channel."
        );
    }

    #[test]
    fn session_rule_carries_current_online_count() {
        let line = TranscriptLine::SessionStart {
            time: "12:00 AM".to_owned(),
            channel: "Op Superiority".to_owned(),
            online: 11,
        };

        assert_eq!(
            transcript_text(&line),
            "YOU JOINED OP SUPERIORITY /// 11 ONLINE"
        );
    }
}
