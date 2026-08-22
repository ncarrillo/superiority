use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Div, Element, ElementId, GlobalElementId, Hitbox,
    HitboxBehavior, InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, RenderOnce, ScrollHandle, SharedString, StyledText,
    Window, div, prelude::*, px, rgb,
};

use crate::foundation::scrollbar::{
    ScrollbarAxes, ScrollbarColors, Scrollbars, WithScrollbar as _,
};

pub const VIEWPORT_PADDING: f32 = 18.0;
pub const ROW_GAP: f32 = 5.0;
pub const ROW_LINE_HEIGHT: f32 = 19.0;
pub const ROW_TEXT_SIZE: f32 = 13.0;
pub const TIME_GUTTER_WIDTH: f32 = 68.0;
pub const TIME_TEXT_SIZE: f32 = 12.0;
pub const SPEAKER_AVATAR_SIZE: f32 = 16.0;
pub const SPEAKER_GAP: f32 = 6.0;
pub const SPEAKER_NUDGE: f32 = 2.0;

/// A transcript's content column. Its minimum height is the viewport height,
/// so a short history rests against the composer; once it grows taller it
/// becomes ordinary top-to-bottom scroll content. Keeping this product-neutral
/// prevents desktop and Live (and each game skin) from disagreeing about where
/// a quiet channel belongs.
#[must_use]
pub fn bottom_anchored_column(content: Vec<AnyElement>) -> Div {
    div()
        .w_full()
        .min_h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .justify_end()
        .children(content)
}

/// The one transcript viewport used by every product and host. Product
/// wrappers supply their font and empty-state treatment, while padding,
/// scrolling, selection termination, scrollbars, row spacing, and bottom
/// anchoring remain one implementation.
#[derive(IntoElement)]
pub struct TranscriptViewport {
    id: ElementId,
    scroll: Option<ScrollHandle>,
    selection: TranscriptSelection,
    font_family: SharedString,
    scrollbar: ScrollbarColors,
    content: Vec<AnyElement>,
}

impl TranscriptViewport {
    #[must_use]
    pub fn new(
        id: impl Into<ElementId>,
        selection: TranscriptSelection,
        font_family: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            scroll: None,
            selection,
            font_family: font_family.into(),
            scrollbar: ScrollbarColors::default(),
            content: Vec::new(),
        }
    }

    #[must_use]
    pub fn scroll(mut self, scroll: &ScrollHandle) -> Self {
        self.scroll = Some(scroll.clone());
        self
    }

    /// The realm's scrollbar colours; the product wrapper always sets them.
    #[must_use]
    pub fn scrollbar_colors(mut self, colors: ScrollbarColors) -> Self {
        self.scrollbar = colors;
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
}

impl RenderOnce for TranscriptViewport {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let selection = self.selection;
        let scroll = self.scroll;
        let scrollbar = self.scrollbar;
        let scrollbar_id = self.id.clone();
        let mut viewport = div()
            .id(self.id)
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .p(px(VIEWPORT_PADDING))
            .font_family(self.font_family)
            .text_size(px(ROW_TEXT_SIZE))
            .on_mouse_up(MouseButton::Left, move |_, _, _| selection.end());
        if let Some(scroll) = &scroll {
            viewport = viewport.overflow_y_scroll().track_scroll(scroll);
        } else {
            viewport = viewport.overflow_hidden();
        }
        let viewport = viewport.child(bottom_anchored_column(self.content).gap(px(ROW_GAP)));
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
                        .tracked_scroll_handle(scroll)
                        .colors(scrollbar),
                    window,
                    cx,
                )
        } else {
            div()
                .id((scrollbar_id, "viewport-static"))
                .absolute()
                .inset_0()
                .overflow_hidden()
                .child(viewport)
        }
    }
}

#[must_use]
pub fn time_gutter(
    text: impl Into<SharedString>,
    font_family: impl Into<SharedString>,
    color: u32,
) -> Div {
    div()
        .w(px(TIME_GUTTER_WIDTH))
        .flex_shrink_0()
        .font_family(font_family.into())
        .text_size(px(TIME_TEXT_SIZE))
        .text_color(rgb(color))
        .child(text.into())
}

#[must_use]
pub fn inline_time(
    text: impl Into<SharedString>,
    font_family: impl Into<SharedString>,
    color: u32,
) -> Div {
    div()
        .flex_shrink_0()
        .font_family(font_family.into())
        .text_size(px(TIME_TEXT_SIZE))
        .text_color(rgb(color))
        .child(text.into())
}

/// Shared geometry for an ordinary transcript row. The portrait itself and
/// all colors remain product-owned; their placement does not.
#[must_use]
pub fn row_shell(
    show_timestamp: bool,
    timestamp: Div,
    leading: Vec<AnyElement>,
    portrait: Option<AnyElement>,
    font_family: impl Into<SharedString>,
    color: u32,
    content: impl IntoElement,
) -> Div {
    div()
        .flex()
        .items_start()
        .line_height(px(ROW_LINE_HEIGHT))
        .font_family(font_family.into())
        .text_size(px(ROW_TEXT_SIZE))
        .children(show_timestamp.then_some(timestamp))
        .children(leading)
        .children(portrait.map(|portrait| {
            div()
                .flex_shrink_0()
                .mr(px(SPEAKER_GAP))
                .mt(px(SPEAKER_NUDGE))
                .child(portrait)
        }))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .text_color(rgb(color))
                .child(content),
        )
}

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

    pub fn selection_for_row(&self, row: usize, len: usize) -> Option<Range<usize>> {
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

pub struct SelectableText {
    id: ElementId,
    row: usize,
    text: StyledText,
    links: Vec<(Range<usize>, String)>,
    selection: TranscriptSelection,
}

impl SelectableText {
    pub fn new(
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
