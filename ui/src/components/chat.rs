use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Div, Element, ElementId, GlobalElementId,
    HighlightStyle, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, RenderOnce, ScrollHandle, Stateful,
    StyledText, Window, div, prelude::*, px, rgb, rgba,
};

use crate::{
    TranscriptLine,
    components::scrollbar::{ScrollbarAxes, Scrollbars, WithScrollbar as _},
    theme::{ACCENT, FONT_INTERFACE, FONT_INTERNATIONAL, MUTED, NOTICE, ONLINE, TEXT},
};
use url::Url;

const SELECTION_BACKGROUND: u32 = 0x1769_9dcc;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TextPosition {
    row: usize,
    offset: usize,
}

#[derive(Default)]
struct SelectionState {
    anchor: Option<TextPosition>,
    focus: Option<TextPosition>,
    dragging: bool,
    pressed_link: Option<PressedLink>,
}

#[derive(Clone)]
struct PressedLink {
    row: usize,
    range: Range<usize>,
    destination: String,
}

#[derive(Clone, Default)]
pub struct TranscriptSelection(Rc<RefCell<SelectionState>>);

impl TranscriptSelection {
    pub fn clear(&self) {
        *self.0.borrow_mut() = SelectionState::default();
    }

    #[must_use]
    pub fn has_selection(&self) -> bool {
        let state = self.0.borrow();
        state.anchor.is_some() && state.focus.is_some() && state.anchor != state.focus
    }

    #[must_use]
    pub fn selected_text(&self, rows: &[(usize, String)]) -> Option<String> {
        let state = self.0.borrow();
        let (start, end) = ordered_selection(&state)?;
        if start == end {
            return None;
        }

        let mut selected = Vec::new();
        for (row, text) in rows {
            if *row < start.row || *row > end.row {
                continue;
            }
            let from = if *row == start.row {
                start.offset.min(text.len())
            } else {
                0
            };
            let to = if *row == end.row {
                end.offset.min(text.len())
            } else {
                text.len()
            };
            if from <= to && text.is_char_boundary(from) && text.is_char_boundary(to) {
                selected.push(text[from..to].to_owned());
            }
        }
        (!selected.is_empty()).then(|| selected.join("\n"))
    }

    fn selection_for_row(&self, row: usize, len: usize) -> Option<Range<usize>> {
        let state = self.0.borrow();
        let (start, end) = ordered_selection(&state)?;
        if row < start.row || row > end.row || start == end {
            return None;
        }
        let from = if row == start.row {
            start.offset.min(len)
        } else {
            0
        };
        let to = if row == end.row {
            end.offset.min(len)
        } else {
            len
        };
        (from < to).then_some(from..to)
    }

    fn update(&self, row: usize, offset: usize) -> bool {
        let mut state = self.0.borrow_mut();
        let position = TextPosition { row, offset };
        if state.dragging && state.focus != Some(position) {
            state.focus = Some(position);
            return true;
        }
        false
    }

    fn begin(&self, row: usize, offset: usize, extend: bool) {
        let mut state = self.0.borrow_mut();
        let position = TextPosition { row, offset };
        if !extend || state.anchor.is_none() {
            state.anchor = Some(position);
        }
        state.focus = Some(position);
        state.dragging = true;
        state.pressed_link = None;
    }

    fn press_link(&self, row: usize, range: Range<usize>, destination: String) {
        let mut state = self.0.borrow_mut();
        state.dragging = false;
        state.pressed_link = Some(PressedLink {
            row,
            range,
            destination,
        });
    }

    fn release_link(&self, row: usize, offset: usize) -> Option<String> {
        let pressed = self.0.borrow_mut().pressed_link.take()?;
        (pressed.row == row && pressed.range.contains(&offset)).then_some(pressed.destination)
    }

    pub fn end(&self) {
        self.0.borrow_mut().dragging = false;
    }
}

struct SelectableText {
    id: ElementId,
    row: usize,
    text: StyledText,
    links: Vec<(Range<usize>, String)>,
    selection: TranscriptSelection,
}

impl SelectableText {
    fn new(
        id: impl Into<ElementId>,
        row: usize,
        text: StyledText,
        links: Vec<(Range<usize>, String)>,
        selection: TranscriptSelection,
    ) -> Self {
        Self {
            id: id.into(),
            row,
            text,
            links,
            selection,
        }
    }
}

impl IntoElement for SelectableText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectableText {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.text.request_layout(None, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.text
            .prepaint(None, inspector_id, bounds, state, window, cx);
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.text.layout().clone();
        let hovered_offset = hitbox
            .is_hovered(window)
            .then(|| layout.index_for_position(window.mouse_position()).ok())
            .flatten();
        if hovered_offset.is_some_and(|offset| link_at(&self.links, offset).is_some()) {
            window.set_cursor_style(gpui::CursorStyle::PointingHand, hitbox);
        } else if hitbox.is_hovered(window) {
            window.set_cursor_style(gpui::CursorStyle::IBeam, hitbox);
        }

        let row = self.row;
        let links = self.links.clone();
        let selection = self.selection.clone();
        let down_hitbox = hitbox.clone();
        let down_layout = layout.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble
                && event.button == MouseButton::Left
                && down_hitbox.is_hovered(window)
            {
                let offset = down_layout
                    .index_for_position(event.position)
                    .unwrap_or_else(|offset| offset);
                if let Some((range, destination)) = link_at(&links, offset) {
                    selection.press_link(row, range.clone(), destination.clone());
                } else {
                    selection.begin(row, offset, event.modifiers.shift);
                }
                window.refresh();
            }
        });

        let row = self.row;
        let selection = self.selection.clone();
        let move_hitbox = hitbox.clone();
        let move_layout = layout.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble && move_hitbox.is_hovered(window) {
                let offset = move_layout
                    .index_for_position(event.position)
                    .unwrap_or_else(|offset| offset);
                if selection.update(row, offset) {
                    window.refresh();
                }
            }
        });

        let row = self.row;
        let selection = self.selection.clone();
        let up_hitbox = hitbox.clone();
        let up_layout = layout.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                if up_hitbox.is_hovered(window) {
                    let offset = up_layout.index_for_position(event.position).ok();
                    if let Some(destination) =
                        offset.and_then(|offset| selection.release_link(row, offset))
                    {
                        cx.open_url(&destination);
                    }
                }
                selection.end();
                window.refresh();
            }
        });

        self.text
            .paint(None, inspector_id, bounds, &mut (), &mut (), window, cx);
    }
}

fn link_at(links: &[(Range<usize>, String)], offset: usize) -> Option<&(Range<usize>, String)> {
    links.iter().find(|(range, _)| range.contains(&offset))
}

fn ordered_selection(state: &SelectionState) -> Option<(TextPosition, TextPosition)> {
    let anchor = state.anchor?;
    let focus = state.focus?;
    Some(if anchor <= focus {
        (anchor, focus)
    } else {
        (focus, anchor)
    })
}

struct ContentStyle {
    text: String,
    color: u32,
    emphasis: Option<Range<usize>>,
    accents: Vec<Range<usize>>,
    links: Vec<(Range<usize>, String)>,
}

fn content_style(line: &TranscriptLine) -> ContentStyle {
    match line {
        TranscriptLine::Notice { text, .. } => {
            linked_content(text.clone(), NOTICE, None, Vec::new())
        }
        TranscriptLine::Membership { text, .. } => {
            linked_content(format!("• {text}"), MUTED, None, vec![0.."•".len()])
        }
        TranscriptLine::Message { sender, text, .. } => {
            let prefix = format!("{sender}:");
            let emphasis = 0..prefix.len();
            let (body, lobby_links) = display_message_body(text);
            let body_start = prefix.len() + 1;
            linked_content(
                format!("{prefix} {body}"),
                TEXT,
                Some(emphasis),
                lobby_links
                    .into_iter()
                    .map(|range| body_start + range.start..body_start + range.end)
                    .collect(),
            )
        }
        TranscriptLine::Error { text, .. } => {
            linked_content(format!("!  {text}"), 0xff6b63, Some(0..1), Vec::new())
        }
    }
}

fn linked_content(
    text: String,
    color: u32,
    emphasis: Option<Range<usize>>,
    mut accents: Vec<Range<usize>>,
) -> ContentStyle {
    let links = recognized_urls(&text);
    accents.extend(links.iter().map(|(range, _)| range.clone()));
    ContentStyle {
        text,
        color,
        emphasis,
        accents,
        links,
    }
}

fn recognized_urls(text: &str) -> Vec<(Range<usize>, String)> {
    const PREFIXES: [&str; 3] = ["https://", "http://", "www."];
    let lowercase = text.to_ascii_lowercase();
    let mut links = Vec::new();
    let mut cursor = 0;

    while cursor < text.len() {
        let Some((offset, prefix)) = PREFIXES
            .iter()
            .filter_map(|prefix| {
                lowercase[cursor..]
                    .find(prefix)
                    .map(|offset| (offset, *prefix))
            })
            .min_by_key(|(offset, _)| *offset)
        else {
            break;
        };
        let start = cursor + offset;
        if text[..start]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            cursor = start + prefix.len();
            continue;
        }

        let raw_end = text[start..]
            .char_indices()
            .skip(1)
            .find_map(|(offset, character)| {
                (character.is_whitespace() || matches!(character, '<' | '>' | '"' | '\''))
                    .then_some(start + offset)
            })
            .unwrap_or(text.len());
        let end = trim_url_end(text, start, raw_end);
        let displayed = &text[start..end];
        let destination = if prefix == "www." {
            format!("https://{displayed}")
        } else {
            displayed.to_owned()
        };
        if Url::parse(&destination)
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
        {
            links.push((start..end, destination));
        }
        cursor = raw_end.max(start + prefix.len());
    }
    links
}

fn trim_url_end(text: &str, start: usize, mut end: usize) -> usize {
    loop {
        let Some(character) = text[start..end].chars().next_back() else {
            return end;
        };
        let trim = matches!(character, '.' | ',' | '!' | '?' | ';' | ':')
            || matches!(character, ')' | ']' | '}')
                && unmatched_closer(&text[start..end], character);
        if !trim {
            return end;
        }
        end -= character.len_utf8();
    }
}

fn unmatched_closer(value: &str, closer: char) -> bool {
    let opener = match closer {
        ')' => '(',
        ']' => '[',
        '}' => '{',
        _ => return false,
    };
    value
        .chars()
        .filter(|character| *character == closer)
        .count()
        > value
            .chars()
            .filter(|character| *character == opener)
            .count()
}

#[must_use]
pub fn display_message_body(body: &str) -> (String, Vec<Range<usize>>) {
    const PREFIX: &str = "lobbyLink(";
    let mut output = String::new();
    let mut links = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = body[cursor..].find(PREFIX) {
        let at = cursor + offset;
        output.push_str(&body[cursor..at]);
        let value = &body[at + PREFIX.len()..];
        let Some((id, rest)) = value.split_once(',') else {
            output.push_str(&body[at..]);
            return (output, links);
        };
        let Some(close) = rest.find(')') else {
            output.push_str(&body[at..]);
            return (output, links);
        };
        let name = &rest[..close];
        if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) || name.is_empty() {
            output.push_str(&body[at..at + PREFIX.len()]);
            cursor = at + PREFIX.len();
            continue;
        }
        let start = output.len();
        output.push_str(name);
        links.push(start..output.len());
        cursor = at + PREFIX.len() + id.len() + 1 + close + 1;
        if body[cursor..].starts_with(';') {
            cursor += 1;
        }
    }
    output.push_str(&body[cursor..]);
    (output, links)
}

#[must_use]
pub fn split_media(body: &str) -> (String, Vec<String>) {
    let mut links = Vec::new();
    let mut text = String::with_capacity(body.len());
    for token in body.split_whitespace() {
        if is_media_link(token) {
            links.push(token.to_owned());
        } else {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(token);
        }
    }
    (text, links)
}

fn is_media_link(token: &str) -> bool {
    let Some(rest) = token
        .strip_prefix("https://")
        .or_else(|| token.strip_prefix("http://"))
    else {
        return false;
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let host = host.to_ascii_lowercase();
    let path = path.to_ascii_lowercase();
    [".gif", ".png", ".jpg", ".jpeg", ".webp"]
        .iter()
        .any(|suffix| path.ends_with(suffix))
        || host.ends_with("giphy.com")
        || host.ends_with("tenor.com")
}

fn styled_content(content: &ContentStyle, selection: Option<&Range<usize>>) -> StyledText {
    let mut boundaries = vec![0, content.text.len()];
    for range in [content.emphasis.as_ref(), selection].into_iter().flatten() {
        boundaries.extend([range.start, range.end]);
    }
    for range in &content.accents {
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
            style.color = Some(rgb(ACCENT).into());
            style.font_weight = Some(gpui::FontWeight::BOLD);
        } else if content
            .accents
            .iter()
            .any(|accent| accent.contains(&range.start))
        {
            style.color = Some(rgb(ONLINE).into());
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
fn panel() -> Div {
    div()
        .relative()
        .flex_1()
        .h_full()
        .min_w_0()
        .overflow_hidden()
}

#[derive(IntoElement)]
pub(crate) struct ChatPanel {
    content: Vec<AnyElement>,
}

impl ChatPanel {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            content: Vec::new(),
        }
    }

    #[must_use]
    pub(crate) fn child(mut self, child: impl IntoElement) -> Self {
        self.content.push(child.into_any_element());
        self
    }
}

impl RenderOnce for ChatPanel {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        panel().children(self.content)
    }
}

#[must_use]
fn transcript_viewport(id: impl Into<ElementId>, scroll: Option<&ScrollHandle>) -> Stateful<Div> {
    let mut viewport = div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .p(px(18.0));
    if let Some(scroll) = scroll {
        viewport = viewport.overflow_y_scroll().track_scroll(scroll);
    } else {
        viewport = viewport.overflow_hidden();
    }
    viewport
}

#[derive(IntoElement)]
pub struct TranscriptViewport {
    id: ElementId,
    scroll: Option<ScrollHandle>,
    selection: TranscriptSelection,
    content: Vec<AnyElement>,
}

impl TranscriptViewport {
    #[must_use]
    pub fn new(id: impl Into<ElementId>, selection: TranscriptSelection) -> Self {
        Self {
            id: id.into(),
            scroll: None,
            selection,
            content: Vec::new(),
        }
    }

    #[must_use]
    pub fn scroll(mut self, scroll: &ScrollHandle) -> Self {
        self.scroll = Some(scroll.clone());
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.content.push(child.into_any_element());
        self
    }

    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.content
            .extend(children.into_iter().map(IntoElement::into_any_element));
        self
    }

    #[must_use]
    pub fn empty(self, message: impl Into<gpui::SharedString>) -> Self {
        self.child(empty_transcript(message))
    }
}

impl RenderOnce for TranscriptViewport {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let selection = self.selection;
        let scroll = self.scroll;
        let scrollbar_id = self.id.clone();
        let viewport = transcript_viewport(self.id, scroll.as_ref())
            .font_family(FONT_INTERNATIONAL)
            .text_size(px(13.0))
            .on_mouse_up(MouseButton::Left, move |_, _, _| selection.end())
            .children(self.content);
        if let Some(scroll) = &scroll {
            div()
                .id((scrollbar_id.clone(), "viewport"))
                .absolute()
                .inset_0()
                .overflow_hidden()
                .child(viewport)
                .custom_scrollbars(
                    Scrollbars::new(ScrollbarAxes::Vertical)
                        .id((scrollbar_id, "scrollbar"))
                        .tracked_scroll_handle(scroll),
                    window,
                    cx,
                )
        } else {
            viewport
        }
    }
}

#[must_use]
fn empty_transcript(message: impl Into<gpui::SharedString>) -> Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .font_family(FONT_INTERFACE)
        .text_size(px(11.0))
        .text_color(rgb(MUTED))
        .child(message.into())
}

#[must_use]
pub fn transcript_row(line: &TranscriptLine, show_timestamps: bool) -> Div {
    transcript_row_inner(line, show_timestamps)
}

#[must_use]
pub fn selectable_transcript_row(
    line: &TranscriptLine,
    show_timestamps: bool,
    scope: u64,
    row: usize,
    selection: &TranscriptSelection,
) -> gpui::Stateful<Div> {
    let content = content_style(line);
    let selected_range = selection.selection_for_row(row, content.text.len());
    let time = match line {
        TranscriptLine::Notice { time, .. }
        | TranscriptLine::Membership { time, .. }
        | TranscriptLine::Message { time, .. }
        | TranscriptLine::Error { time, .. } => time,
    };
    let text = SelectableText::new(
        format!("transcript-text-{scope}-{row}"),
        row,
        styled_content(&content, selected_range.as_ref()),
        content.links,
        selection.clone(),
    );

    transcript_row_shell(time, show_timestamps, content.color, text)
        .id(format!("transcript-row-{scope}-{row}"))
        .cursor(gpui::CursorStyle::IBeam)
}

fn transcript_row_inner(line: &TranscriptLine, show_timestamps: bool) -> Div {
    let content = content_style(line);
    let time = match line {
        TranscriptLine::Notice { time, .. }
        | TranscriptLine::Membership { time, .. }
        | TranscriptLine::Message { time, .. }
        | TranscriptLine::Error { time, .. } => time,
    };
    transcript_row_shell(
        time,
        show_timestamps,
        content.color,
        styled_content(&content, None),
    )
}

fn transcript_row_shell(
    time: &str,
    show_timestamps: bool,
    color: u32,
    content: impl IntoElement,
) -> Div {
    let mut row = div()
        .flex()
        .items_start()
        .line_height(px(19.0))
        .font_family(FONT_INTERNATIONAL)
        .text_size(px(13.0));
    if show_timestamps {
        row = row.child(
            div()
                .w(px(68.0))
                .flex_shrink_0()
                .font_family(FONT_INTERFACE)
                .text_size(px(12.0))
                .text_color(rgb(MUTED))
                .child(format!("[{time}]")),
        );
    }
    row.child(
        div()
            .min_w_0()
            .flex_1()
            .text_color(rgb(color))
            .child(content),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_text_is_one_contiguous_string() {
        let line = TranscriptLine::Message {
            time: "7:31 PM".into(),
            sender: "Nova".into(),
            text: "This message wraps normally.".into(),
        };
        assert_eq!(transcript_text(&line), "Nova: This message wraps normally.");
    }

    #[test]
    fn selection_spans_rows_in_display_order() {
        let selection = TranscriptSelection::default();
        {
            let mut state = selection.0.borrow_mut();
            state.anchor = Some(TextPosition { row: 3, offset: 2 });
            state.focus = Some(TextPosition { row: 5, offset: 3 });
        }
        let rows = vec![
            (3, "alpha".to_owned()),
            (4, "bravo".to_owned()),
            (5, "charlie".to_owned()),
        ];
        assert_eq!(
            selection.selected_text(&rows).as_deref(),
            Some("pha\nbravo\ncha")
        );
    }

    #[test]
    fn lobby_links_show_only_their_names() {
        let (text, links) = display_message_body(
            "join lobbyLink(12345,Big Game Hunters); or lobbyLink(9,Metalopolis)",
        );
        assert_eq!(text, "join Big Game Hunters or Metalopolis");
        assert_eq!(
            links
                .iter()
                .map(|range| &text[range.clone()])
                .collect::<Vec<_>>(),
            vec!["Big Game Hunters", "Metalopolis"]
        );
    }

    #[test]
    fn malformed_lobby_links_remain_visible() {
        for text in [
            "lobbyLink(abc,Name);",
            "lobbyLink(12,);",
            "lobbyLink(12,unclosed",
        ] {
            assert_eq!(display_message_body(text), (text.to_owned(), Vec::new()));
        }
    }

    #[test]
    fn media_links_are_separated_from_conversation_text() {
        let (text, links) = split_media("look https://example.com/cat.gif now");
        assert_eq!(text, "look now");
        assert_eq!(links, vec!["https://example.com/cat.gif"]);
    }

    #[test]
    fn transcript_urls_exclude_sentence_punctuation() {
        let text = "see (https://example.com/a?q=1), then www.example.org/docs.";
        let links = recognized_urls(text);
        assert_eq!(
            links
                .iter()
                .map(|(range, destination)| (&text[range.clone()], destination.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("https://example.com/a?q=1", "https://example.com/a?q=1"),
                ("www.example.org/docs", "https://www.example.org/docs"),
            ]
        );
    }

    #[test]
    fn transcript_url_ranges_include_the_sender_prefix_offset() {
        let line = TranscriptLine::Message {
            time: "7:31 PM".into(),
            sender: "Nova".into(),
            text: "open https://example.com".into(),
        };
        let content = content_style(&line);
        assert_eq!(content.links.len(), 1);
        assert_eq!(
            &content.text[content.links[0].0.clone()],
            "https://example.com"
        );
    }
}
