//! Reforged transcript presentation over the shared transcript machinery.
//!
//! Scrolling, bottom anchoring, selection, copy ranges, row rhythm, and the
//! viewport scrollbar come from `patterns::transcript`, exactly as they do for
//! SC:R and SC2. This module owns only Reforged's words and visual treatment.

use std::ops::Range;

use gpui::{
    AnyElement, App, Div, ElementId, HighlightStyle, IntoElement, RenderOnce, ScrollHandle,
    StyledText, Window, div, prelude::*, px, rgb, rgba,
};

use crate::{
    patterns::transcript::{self, SelectableText, TranscriptSelection},
    products::wc3::{
        TranscriptLine,
        theme::{BLOOD, FONT_INTERFACE, GOLD, GOLD_BRIGHT, MUTED, PARCHMENT, QUIET},
    },
};

const SELECTION_BACKGROUND: u32 = 0x5e4a_26eb;
const RULE_COLOR: u32 = 0x8a6d_3bb0;
const RULE_FADE: u32 = 0x8a6d_3b00;

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
            text: format!("You entered {channel} · {online} players"),
            color: GOLD,
            emphasis: None,
            emphasis_color: GOLD_BRIGHT,
        },
        TranscriptLine::Message {
            sender: Some(sender),
            text,
            ..
        } => {
            let prefix = format!("{sender}:");
            ContentStyle {
                text: format!("{prefix} {text}"),
                color: PARCHMENT,
                emphasis: Some(0..prefix.len()),
                emphasis_color: GOLD,
            }
        }
        TranscriptLine::Message {
            sender: None, text, ..
        } => ContentStyle {
            // The recovered WC3 callback does not yet expose a sender. Keep
            // exactly the words the server supplied instead of inventing one.
            text: text.clone(),
            color: PARCHMENT,
            emphasis: None,
            emphasis_color: GOLD,
        },
        TranscriptLine::Notice { text, .. } => ContentStyle {
            text: text.clone(),
            color: MUTED,
            emphasis: None,
            emphasis_color: GOLD,
        },
        TranscriptLine::Error { text, .. } => ContentStyle {
            text: format!("!  {text}"),
            color: BLOOD,
            emphasis: Some(0..1),
            emphasis_color: BLOOD,
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
    StyledText::new(content.text.clone()).with_highlights(highlights)
}

#[must_use]
pub fn transcript_text(line: &TranscriptLine) -> String {
    content_style(line).text
}

#[must_use]
pub fn selectable_transcript_row(
    line: &TranscriptLine,
    scope: &str,
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
    let text = SelectableText::new(
        format!("wc3-transcript-text-{scope}-{row}"),
        row,
        styled_content(&content, selected.as_ref()),
        Vec::new(),
        selection.clone(),
    );
    transcript::row_shell(
        true,
        time_gutter(line_time(line)),
        Vec::new(),
        None,
        FONT_INTERFACE,
        content.color,
        text,
    )
    .id(format!("wc3-transcript-row-{scope}-{row}"))
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

fn time_gutter(time: &str) -> Div {
    transcript::time_gutter(format!("[{time}]"), FONT_INTERFACE, MUTED)
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
                    time.to_owned(),
                    FONT_INTERFACE,
                    MUTED,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .text_color(rgb(GOLD))
                        .child("You entered")
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(rgb(PARCHMENT))
                                .child(channel.to_owned()),
                        ),
                )
                .child(
                    div()
                        .text_color(rgb(MUTED))
                        .child(format!("· {online} players")),
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
            inner: transcript::TranscriptViewport::new(id, selection, FONT_INTERFACE)
                .scrollbar_colors(crate::products::modal::ModalVariant::Reforged.scrollbar()),
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
                .text_color(rgb(QUIET))
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

    #[test]
    fn transcript_copy_uses_the_words_the_row_displays() {
        let message = TranscriptLine::Message {
            time: "12:00 AM".to_owned(),
            sender: Some("Arthas".to_owned()),
            text: "For Lordaeron.".to_owned(),
        };
        let realm = TranscriptLine::Message {
            time: "12:01 AM".to_owned(),
            sender: None,
            text: "The hall stirs.".to_owned(),
        };
        assert_eq!(transcript_text(&message), "Arthas: For Lordaeron.");
        assert_eq!(transcript_text(&realm), "The hall stirs.");
    }

    #[test]
    fn session_rule_carries_the_channel_population() {
        let line = TranscriptLine::SessionStart {
            time: "12:00 AM".to_owned(),
            channel: "w3 general".to_owned(),
            online: 70,
        };
        assert_eq!(
            transcript_text(&line),
            "You entered w3 general · 70 players"
        );
    }
}
