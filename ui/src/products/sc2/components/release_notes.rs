use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    App, Bounds, DispatchPhase, Div, Element, ElementId, FontStyle, FontWeight, GlobalElementId,
    HighlightStyle, Hitbox, HitboxBehavior, InspectorElementId, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, StrikethroughStyle, StyledText, Window,
    div, prelude::*, px, rgb, rgba,
};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::products::sc2::theme::{FONT_INTERFACE, FONT_INTERNATIONAL};

const BODY_TEXT: u32 = 0x00db_e7f7;
const HEADING_TEXT: u32 = 0x0062_c9ff;
const MUTED_TEXT: u32 = 0x009f_b5cf;
const LINK_TEXT: u32 = 0x003e_b8ff;
const CODE_TEXT: u32 = 0x008e_daff;
const STRONG_TEXT: u32 = 0x00f2_f7ff;
use crate::foundation::text_input::SELECTION_BACKGROUND;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseNotesDocument {
    blocks: Vec<ReleaseNotesBlock>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReleaseNotesBlock {
    kind: BlockKind,
    inline: InlineText,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BlockKind {
    Heading(u8),
    Paragraph {
        quote_depth: usize,
    },
    ListItem {
        depth: usize,
        marker: String,
        quote_depth: usize,
    },
    CodeBlock {
        quote_depth: usize,
    },
    TableRow {
        header: bool,
    },
    Rule,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct InlineText {
    text: String,
    spans: Vec<InlineSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InlineSpan {
    range: Range<usize>,
    style: InlineStyle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum InlineStyle {
    Emphasis,
    Strong,
    Strikethrough,
    Code,
    Link(String),
}

impl ReleaseNotesDocument {
    #[must_use]
    pub fn parse(notes: &str, format: &str) -> Self {
        let format = format.to_ascii_lowercase();
        if format.contains("markdown") {
            return MarkdownDocumentBuilder::parse(notes);
        }
        if format.contains("html") {
            return Self::plain(&visible_html_text(notes));
        }
        Self::plain(notes)
    }

    #[must_use]
    pub fn plain(notes: &str) -> Self {
        let blocks = notes
            .split("\n\n")
            .filter_map(|paragraph| {
                let text = paragraph.trim();
                (!text.is_empty()).then(|| ReleaseNotesBlock {
                    kind: BlockKind::Paragraph { quote_depth: 0 },
                    inline: InlineText {
                        text: text.to_owned(),
                        spans: Vec::new(),
                    },
                })
            })
            .collect();
        Self { blocks }
    }

    #[must_use]
    pub fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .filter(|block| !matches!(block.kind, BlockKind::Rule))
            .map(|block| match &block.kind {
                BlockKind::ListItem { marker, .. } if !marker.is_empty() => {
                    format!("{marker} {}", block.inline.text)
                }
                _ => block.inline.text.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn selectable_rows(&self) -> impl Iterator<Item = (usize, &str)> {
        self.blocks.iter().enumerate().filter_map(|(row, block)| {
            (!block.inline.text.is_empty()).then_some((row, block.inline.text.as_str()))
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TextPosition {
    row: usize,
    offset: usize,
}

#[derive(Clone)]
struct PressedLink {
    row: usize,
    range: Range<usize>,
    destination: String,
}

#[derive(Default)]
struct SelectionState {
    anchor: Option<TextPosition>,
    focus: Option<TextPosition>,
    dragging: bool,
    pressed_link: Option<PressedLink>,
}

#[derive(Clone, Default)]
pub struct ReleaseNotesSelection(Rc<RefCell<SelectionState>>);

impl ReleaseNotesSelection {
    pub fn clear(&self) {
        *self.0.borrow_mut() = SelectionState::default();
    }

    #[must_use]
    pub fn has_selection(&self) -> bool {
        let state = self.0.borrow();
        state.anchor.is_some() && state.focus.is_some() && state.anchor != state.focus
    }

    #[must_use]
    pub fn selected_text(&self, document: &ReleaseNotesDocument) -> Option<String> {
        let state = self.0.borrow();
        let (start, end) = ordered_selection(&state)?;
        if start == end {
            return None;
        }
        let selected = document
            .selectable_rows()
            .filter_map(|(row, text)| {
                if row < start.row || row > end.row {
                    return None;
                }
                let from = if row == start.row {
                    start.offset.min(text.len())
                } else {
                    0
                };
                let to = if row == end.row {
                    end.offset.min(text.len())
                } else {
                    text.len()
                };
                (from <= to && text.is_char_boundary(from) && text.is_char_boundary(to))
                    .then(|| text[from..to].to_owned())
            })
            .collect::<Vec<_>>();
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

    fn update(&self, row: usize, offset: usize) -> bool {
        let mut state = self.0.borrow_mut();
        if !state.dragging {
            return false;
        }
        let position = TextPosition { row, offset };
        if state.focus == Some(position) {
            return false;
        }
        state.focus = Some(position);
        true
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

    fn end(&self) {
        self.0.borrow_mut().dragging = false;
    }
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

struct SelectableRichText {
    id: ElementId,
    row: usize,
    text: StyledText,
    links: Vec<(Range<usize>, String)>,
    selection: ReleaseNotesSelection,
}

impl IntoElement for SelectableRichText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectableRichText {
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

#[must_use]
pub fn view(document: &ReleaseNotesDocument, selection: &ReleaseNotesSelection) -> Div {
    let mut content = div()
        .w_full()
        .flex()
        .flex_col()
        .font_family(FONT_INTERNATIONAL)
        .text_size(px(14.0))
        .line_height(px(22.0))
        .text_color(rgb(BODY_TEXT));

    for (row, block) in document.blocks.iter().enumerate() {
        content = content.child(render_block(block, row, selection));
    }
    content
}

fn render_block(block: &ReleaseNotesBlock, row: usize, selection: &ReleaseNotesSelection) -> Div {
    let selected = selection.selection_for_row(row, block.inline.text.len());
    let rich_text = || SelectableRichText {
        id: format!("release-notes-text-{row}").into(),
        row,
        text: styled_inline(&block.inline, selected.as_ref()),
        links: block
            .inline
            .spans
            .iter()
            .filter_map(|span| match &span.style {
                InlineStyle::Link(destination) => Some((span.range.clone(), destination.clone())),
                _ => None,
            })
            .collect(),
        selection: selection.clone(),
    };

    match &block.kind {
        BlockKind::Heading(level) => {
            let (font_size, line_height, top, bottom) = match level {
                1 => (20.0, 28.0, 0.0, 20.0),
                2 => (17.0, 25.0, 22.0, 14.0),
                _ => (15.0, 23.0, 20.0, 12.0),
            };
            div()
                .w_full()
                .mt(px(top))
                .mb(px(bottom))
                .min_w_0()
                .font_family(FONT_INTERFACE)
                .font_weight(FontWeight::BOLD)
                .text_size(px(font_size))
                .line_height(px(line_height))
                .text_color(rgb(HEADING_TEXT))
                .child(rich_text())
        }
        BlockKind::Paragraph { quote_depth } if *quote_depth > 0 => div()
            .relative()
            .w_full()
            .my(px(20.0))
            .pl(px(14.0 + (*quote_depth as f32 - 1.0) * 12.0))
            .pr(px(14.0))
            .py(px(10.0))
            .text_color(rgb(MUTED_TEXT))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(3.0))
                    .bg(rgb(LINK_TEXT)),
            )
            .child(rich_text()),
        BlockKind::Paragraph { .. } => div().w_full().min_w_0().mb(px(16.0)).child(rich_text()),
        BlockKind::ListItem {
            depth,
            marker,
            quote_depth,
        } => div()
            .w_full()
            .flex()
            .items_start()
            .ml(px(26.0 + *depth as f32 * 18.0 + *quote_depth as f32 * 12.0))
            .mb(px(8.0))
            .child(
                div()
                    .w(px(22.0))
                    .flex_shrink_0()
                    .text_color(rgb(LINK_TEXT))
                    .child(marker.clone()),
            )
            .child(div().min_w_0().flex_1().child(rich_text())),
        BlockKind::CodeBlock { quote_depth } => div()
            .w_full()
            .ml(px(*quote_depth as f32 * 12.0))
            .mb(px(16.0))
            .px(px(12.0))
            .py(px(9.0))
            .bg(rgba(0x0611_1bd9))
            .border_1()
            .border_color(rgba(0x144f_78d9))
            .font_family("Menlo")
            .text_size(px(12.0))
            .line_height(px(20.0))
            .text_color(rgb(CODE_TEXT))
            .child(rich_text()),
        BlockKind::TableRow { header } => div()
            .w_full()
            .min_w_0()
            .px(px(8.0))
            .py(px(6.0))
            .border_b_1()
            .border_color(rgba(0x144f_78a8))
            .when(*header, |row| {
                row.font_weight(FontWeight::BOLD)
                    .text_color(rgb(HEADING_TEXT))
            })
            .child(rich_text()),
        BlockKind::Rule => div().w_full().h(px(1.0)).my(px(18.0)).bg(rgba(0x3eb8_ff66)),
    }
}

fn styled_inline(inline: &InlineText, selection: Option<&Range<usize>>) -> StyledText {
    let mut boundaries = vec![0, inline.text.len()];
    for span in &inline.spans {
        boundaries.extend([span.range.start, span.range.end]);
    }
    if let Some(selection) = selection {
        boundaries.extend([selection.start, selection.end]);
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
        for span in &inline.spans {
            if !span.range.contains(&range.start) {
                continue;
            }
            match &span.style {
                InlineStyle::Emphasis => style.font_style = Some(FontStyle::Italic),
                InlineStyle::Strong => {
                    style.font_weight = Some(FontWeight::BOLD);
                    style.color = Some(rgb(STRONG_TEXT).into());
                }
                InlineStyle::Strikethrough => {
                    style.strikethrough = Some(StrikethroughStyle {
                        thickness: px(1.0),
                        color: Some(rgb(MUTED_TEXT).into()),
                    });
                }
                InlineStyle::Code => style.color = Some(rgb(CODE_TEXT).into()),
                InlineStyle::Link(_) => style.color = Some(rgb(LINK_TEXT).into()),
            }
        }
        if selection.is_some_and(|selection| selection.contains(&range.start)) {
            style.background_color = Some(rgba(SELECTION_BACKGROUND).into());
        }
        if style != HighlightStyle::default() {
            highlights.push((range, style));
        }
    }

    let mut text = StyledText::new(inline.text.clone()).with_highlights(highlights);
    let code = merge_ranges(
        inline
            .spans
            .iter()
            .filter(|&span| matches!(span.style, InlineStyle::Code))
            .map(|span| span.range.clone()),
    );
    if !code.is_empty() {
        text =
            text.with_font_family_overrides(code.into_iter().map(|range| (range, "Menlo".into())));
    }
    text
}

fn merge_ranges(ranges: impl IntoIterator<Item = Range<usize>>) -> Vec<Range<usize>> {
    let mut ranges = ranges.into_iter().collect::<Vec<_>>();
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        if let Some(previous) = merged.last_mut()
            && range.start <= previous.end
        {
            previous.end = previous.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    merged
}

#[derive(Clone)]
struct ListState {
    next: Option<u64>,
}

impl ListState {
    fn marker(&mut self) -> String {
        match &mut self.next {
            Some(next) => {
                let marker = format!("{next}.");
                *next += 1;
                marker
            }
            None => "•".to_owned(),
        }
    }
}

struct ItemState {
    depth: usize,
    marker: String,
    emitted: bool,
}

struct BlockBuilder {
    kind: BlockKind,
    inline: InlineText,
}

impl BlockBuilder {
    fn new(kind: BlockKind) -> Self {
        Self {
            kind,
            inline: InlineText::default(),
        }
    }

    fn append(&mut self, value: &str, active: &[InlineStyle], extra: Option<InlineStyle>) {
        if value.is_empty() {
            return;
        }
        let start = self.inline.text.len();
        self.inline.text.push_str(value);
        let range = start..self.inline.text.len();
        self.inline
            .spans
            .extend(active.iter().cloned().map(|style| InlineSpan {
                range: range.clone(),
                style,
            }));
        if let Some(style) = extra {
            self.inline.spans.push(InlineSpan { range, style });
        }
    }

    fn finish(self) -> Option<ReleaseNotesBlock> {
        (!self.inline.text.trim().is_empty()).then_some(ReleaseNotesBlock {
            kind: self.kind,
            inline: self.inline,
        })
    }
}

struct MarkdownDocumentBuilder {
    blocks: Vec<ReleaseNotesBlock>,
    current: Option<BlockBuilder>,
    lists: Vec<ListState>,
    item: Option<ItemState>,
    quote_depth: usize,
    active: Vec<InlineStyle>,
    table_header: bool,
    table_cells: Vec<InlineText>,
    table_cell: Option<BlockBuilder>,
}

impl MarkdownDocumentBuilder {
    fn parse(notes: &str) -> ReleaseNotesDocument {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_TASKLISTS);
        let mut this = Self {
            blocks: Vec::new(),
            current: None,
            lists: Vec::new(),
            item: None,
            quote_depth: 0,
            active: Vec::new(),
            table_header: false,
            table_cells: Vec::new(),
            table_cell: None,
        };
        for event in Parser::new_ext(notes, options) {
            this.event(event);
        }
        this.flush_current();
        this.finish_table_row();
        ReleaseNotesDocument {
            blocks: this.blocks,
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.append(&text, None),
            Event::Code(text) => self.append(&text, Some(InlineStyle::Code)),
            Event::InlineMath(text) | Event::DisplayMath(text) => self.append(&text, None),
            Event::Html(html) => self.append(&visible_html_text(&html), None),
            Event::InlineHtml(_) => {}
            Event::FootnoteReference(label) => self.append(&format!("[{label}]"), None),
            Event::SoftBreak => self.append(" ", None),
            Event::HardBreak => self.append("\n", None),
            Event::Rule => {
                self.flush_current();
                self.blocks.push(ReleaseNotesBlock {
                    kind: BlockKind::Rule,
                    inline: InlineText::default(),
                });
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "☑" } else { "☐" }.to_owned();
                if let Some(item) = &mut self.item {
                    item.marker.clone_from(&marker);
                }
                if let Some(BlockBuilder {
                    kind: BlockKind::ListItem { marker: active, .. },
                    ..
                }) = &mut self.current
                {
                    active.clone_from(&marker);
                }
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.ensure_current(),
            Tag::Heading { level, .. } => {
                self.flush_current();
                self.current = Some(BlockBuilder::new(BlockKind::Heading(heading_level(level))));
            }
            Tag::BlockQuote(_) => {
                self.flush_current();
                self.quote_depth += 1;
            }
            Tag::CodeBlock(_) => {
                self.flush_current();
                self.current = Some(BlockBuilder::new(BlockKind::CodeBlock {
                    quote_depth: self.quote_depth,
                }));
            }
            Tag::List(start) => {
                self.flush_current();
                self.lists.push(ListState { next: start });
            }
            Tag::Item => {
                self.flush_current();
                let depth = self.lists.len().saturating_sub(1);
                let marker = self
                    .lists
                    .last_mut()
                    .map_or_else(|| "•".to_owned(), ListState::marker);
                self.item = Some(ItemState {
                    depth,
                    marker,
                    emitted: false,
                });
            }
            Tag::Table(_) => {
                self.flush_current();
                self.table_cells.clear();
            }
            Tag::TableHead => {
                self.table_header = true;
                self.table_cells.clear();
            }
            Tag::TableRow => self.table_cells.clear(),
            Tag::TableCell => {
                self.table_cell = Some(BlockBuilder::new(BlockKind::Paragraph { quote_depth: 0 }));
            }
            Tag::Emphasis => self.active.push(InlineStyle::Emphasis),
            Tag::Strong => self.active.push(InlineStyle::Strong),
            Tag::Strikethrough => self.active.push(InlineStyle::Strikethrough),
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                self.active.push(InlineStyle::Link(dest_url.into_string()));
            }
            Tag::HtmlBlock
            | Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Superscript
            | Tag::Subscript
            | Tag::MetadataBlock(_) => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock => self.flush_current(),
            TagEnd::BlockQuote(_) => {
                self.flush_current();
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            TagEnd::List(_) => {
                self.flush_current();
                self.lists.pop();
            }
            TagEnd::Item => {
                self.flush_current();
                self.item = None;
            }
            TagEnd::TableHead => {
                self.finish_table_row();
                self.table_header = false;
            }
            TagEnd::TableRow => self.finish_table_row(),
            TagEnd::TableCell => {
                if let Some(cell) = self.table_cell.take() {
                    self.table_cells.push(cell.inline);
                }
            }
            TagEnd::Table => self.finish_table_row(),
            TagEnd::Emphasis => self.pop_inline(|style| matches!(style, InlineStyle::Emphasis)),
            TagEnd::Strong => self.pop_inline(|style| matches!(style, InlineStyle::Strong)),
            TagEnd::Strikethrough => {
                self.pop_inline(|style| matches!(style, InlineStyle::Strikethrough));
            }
            TagEnd::Link | TagEnd::Image => {
                self.pop_inline(|style| matches!(style, InlineStyle::Link(_)));
            }
            TagEnd::HtmlBlock
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    fn ensure_current(&mut self) {
        if self.current.is_some() || self.table_cell.is_some() {
            return;
        }
        let kind = self.item.as_mut().map_or(
            BlockKind::Paragraph {
                quote_depth: self.quote_depth,
            },
            |item| {
                let marker = if item.emitted {
                    String::new()
                } else {
                    item.emitted = true;
                    item.marker.clone()
                };
                BlockKind::ListItem {
                    depth: item.depth,
                    marker,
                    quote_depth: self.quote_depth,
                }
            },
        );
        self.current = Some(BlockBuilder::new(kind));
    }

    fn append(&mut self, value: &str, extra: Option<InlineStyle>) {
        if let Some(cell) = &mut self.table_cell {
            cell.append(value, &self.active, extra);
            return;
        }
        self.ensure_current();
        if let Some(current) = &mut self.current {
            current.append(value, &self.active, extra);
        }
    }

    fn flush_current(&mut self) {
        if let Some(block) = self.current.take().and_then(BlockBuilder::finish) {
            self.blocks.push(block);
        }
    }

    fn finish_table_row(&mut self) {
        if self.table_cells.is_empty() {
            return;
        }
        let mut inline = InlineText::default();
        for (index, cell) in self.table_cells.drain(..).enumerate() {
            if index > 0 {
                inline.text.push_str("    |    ");
            }
            let offset = inline.text.len();
            inline.text.push_str(&cell.text);
            inline
                .spans
                .extend(cell.spans.into_iter().map(|span| InlineSpan {
                    range: span.range.start + offset..span.range.end + offset,
                    style: span.style,
                }));
        }
        self.blocks.push(ReleaseNotesBlock {
            kind: BlockKind::TableRow {
                header: self.table_header,
            },
            inline,
        });
    }

    fn pop_inline(&mut self, predicate: impl Fn(&InlineStyle) -> bool) {
        if let Some(index) = self.active.iter().rposition(predicate) {
            self.active.remove(index);
        }
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn visible_html_text(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut inside_tag = false;
    for character in html.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => output.push(character),
            _ => {}
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_preserves_blocks_and_readable_copy() {
        let document = ReleaseNotesDocument::parse(
            "# Update\n\nA **strong** paragraph with `code`.\n\n1. First\n2. Second",
            "markdown",
        );
        assert!(matches!(document.blocks[0].kind, BlockKind::Heading(1)));
        assert!(
            document.blocks[1]
                .inline
                .spans
                .iter()
                .any(|span| matches!(span.style, InlineStyle::Strong))
        );
        assert!(
            document.blocks[1]
                .inline
                .spans
                .iter()
                .any(|span| matches!(span.style, InlineStyle::Code))
        );
        assert_eq!(
            document.plain_text(),
            "Update\nA strong paragraph with code.\n1. First\n2. Second"
        );
    }

    #[test]
    fn links_keep_their_destination_and_display_text() {
        let document = ReleaseNotesDocument::parse(
            "Read the [release notes](https://example.com/release).",
            "markdown",
        );
        assert_eq!(document.blocks[0].inline.text, "Read the release notes.");
        assert!(document.blocks[0].inline.spans.iter().any(|span| {
            matches!(
                &span.style,
                InlineStyle::Link(destination) if destination == "https://example.com/release"
            )
        }));
    }

    #[test]
    fn tables_and_task_lists_remain_structured() {
        let document = ReleaseNotesDocument::parse(
            "| Area | State |\n| --- | --- |\n| Chat | Ready |\n\n- [x] Signed\n- [ ] Installed",
            "markdown",
        );
        assert!(
            document
                .blocks
                .iter()
                .any(|block| matches!(block.kind, BlockKind::TableRow { header: true }))
        );
        assert!(document.plain_text().contains("☑ Signed"));
        assert!(document.plain_text().contains("☐ Installed"));
    }
}
