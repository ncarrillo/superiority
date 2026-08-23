use std::{cell::RefCell, ops::Range, rc::Rc, time::Duration};

use gpui::{
    App, Bounds, ClipboardItem, Context, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, GlobalElementId, Hsla, InspectorElementId,
    KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, ShapedLine, Style, Subscription, Task, TextRun, UTF16Selection, UnderlineStyle,
    Window, actions, div, fill, point, prelude::*, px, relative, rgb, rgba, size,
};
use unicode_segmentation::UnicodeSegmentation as _;

actions!(
    superiority_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        Home,
        End,
        SelectHome,
        SelectEnd,
        SelectAll,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        Undo,
        Redo,
    ]
);

const PLACEHOLDER: u32 = 0x005e_8291;
const CURSOR: u32 = 0x0089_d5ff;

#[derive(Clone, Copy)]
pub struct FieldInk {
    pub placeholder: u32,
    pub cursor: u32,
    pub cursor_width: f32,
}

impl Default for FieldInk {
    fn default() -> Self {
        Self {
            placeholder: PLACEHOLDER,
            cursor: CURSOR,
            cursor_width: 1.0,
        }
    }
}

pub const SELECTION_BACKGROUND: u32 = 0x1769_9dcc;
const HISTORY_LIMIT: usize = 100;
const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone)]
struct Snapshot {
    content: String,
    selection: Range<usize>,
    selection_reversed: bool,
}

struct InputState {
    content: String,
    selection: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    composition_origin: Option<Snapshot>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    scroll_x: Pixels,
    selecting: bool,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            content: String::new(),
            selection: 0..0,
            selection_reversed: false,
            marked_range: None,
            composition_origin: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_layout: None,
            last_bounds: None,
            scroll_x: px(0.0),
            selecting: false,
        }
    }
}

impl InputState {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            content: self.content.clone(),
            selection: self.selection.clone(),
            selection_reversed: self.selection_reversed,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.content = snapshot.content;
        self.selection = snapshot.selection;
        self.selection_reversed = snapshot.selection_reversed;
        self.marked_range = None;
        self.composition_origin = None;
    }

    fn remember(&mut self, snapshot: Snapshot) {
        if self.undo.len() == HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(snapshot);
        self.redo.clear();
    }

    fn cursor(&self) -> usize {
        if self.selection_reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    fn move_to(&mut self, offset: usize) {
        let offset = valid_boundary(&self.content, offset);
        self.selection = offset..offset;
        self.selection_reversed = false;
    }

    fn select_to(&mut self, offset: usize) {
        let offset = valid_boundary(&self.content, offset);
        if self.selection_reversed {
            self.selection.start = offset;
        } else {
            self.selection.end = offset;
        }
        if self.selection.end < self.selection.start {
            self.selection_reversed = !self.selection_reversed;
            self.selection = self.selection.end..self.selection.start;
        }
    }

    fn replace(&mut self, range: Range<usize>, text: &str, remember: bool) {
        let range = valid_range(&self.content, range);
        if remember {
            let snapshot = self.snapshot();
            self.remember(snapshot);
        }
        let text = single_line(text);
        self.content.replace_range(range.clone(), &text);
        let cursor = range.start + text.len();
        self.selection = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
    }

    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(line)) = (self.last_bounds, self.last_layout.as_ref()) else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left() + self.scroll_x)
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for character in self.content.chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += character.len_utf16();
            utf8 += character.len_utf8();
        }
        utf8
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for character in self.content.chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += character.len_utf8();
            utf16 += character.len_utf16();
        }
        utf16
    }

    fn range_from_utf16(&self, range: Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    fn range_to_utf16(&self, range: Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }
}

#[derive(Clone)]
pub struct TextInput {
    state: Rc<RefCell<InputState>>,
    placeholder: Rc<RefCell<String>>,
    accents: Rc<RefCell<Vec<AccentSpan>>>,
    ink: Rc<RefCell<FieldInk>>,
    focus_handle: FocusHandle,
    view: Entity<TextInputView>,
}

/// a run of the line painted apart from the rest — the `/join` word of a
/// command, or a `@mention` token mid-message. the composer shows at a glance
/// which part of what you typed is an instruction and which is the message.
#[derive(Clone)]
pub struct AccentSpan {
    pub range: Range<usize>,
    pub color: u32,
    /// a token is a solid block rather than coloured text, so a mention reads
    /// as one object you cannot type into the middle of.
    pub background: Option<u32>,
}

impl TextInput {
    #[must_use]
    pub fn new(placeholder: impl Into<String>, cx: &mut App) -> Self {
        let state = Rc::new(RefCell::new(InputState::default()));
        let placeholder = Rc::new(RefCell::new(placeholder.into()));
        let accents = Rc::new(RefCell::new(Vec::new()));
        let ink = Rc::new(RefCell::new(FieldInk::default()));
        let focus_handle = cx.focus_handle();
        let view = cx.new({
            let state = state.clone();
            let placeholder = placeholder.clone();
            let accents = accents.clone();
            let ink = ink.clone();
            let focus_handle = focus_handle.clone();
            move |_| TextInputView {
                state,
                focus_handle,
                placeholder,
                accents,
                ink,
                cursor_visible: false,
                cursor_blinking: false,
                cursor_blink_task: Task::ready(()),
                focus_subscriptions: Vec::new(),
            }
        });
        Self {
            state,
            placeholder,
            accents,
            ink,
            focus_handle,
            view,
        }
    }

    /// dresses the input in a host's ink. Idempotent, like
    /// `set_placeholder`: surfaces call it at render.
    pub fn set_ink(&self, ink: FieldInk) {
        *self.ink.borrow_mut() = ink;
    }

    #[must_use]
    pub fn content(&self) -> String {
        self.state.borrow().content.clone()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.state.borrow().content.is_empty()
    }

    /// where the caret sits, as a byte offset. what is being typed is the token
    /// under it, which is not always the last one on the line.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.state.borrow().cursor()
    }

    #[must_use]
    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
        self.view.update(cx, TextInputView::start_cursor_blink);
    }

    pub fn clear(&self) {
        let mut state = self.state.borrow_mut();
        let snapshot = state.snapshot();
        if !snapshot.content.is_empty() {
            state.remember(snapshot);
        }
        state.content.clear();
        state.selection = 0..0;
        state.selection_reversed = false;
        state.marked_range = None;
        state.composition_origin = None;
        state.scroll_x = px(0.0);
    }

    pub fn set_content(&self, content: impl Into<String>) {
        let content = single_line(&content.into());
        let mut state = self.state.borrow_mut();
        let snapshot = state.snapshot();
        if snapshot.content != content {
            state.remember(snapshot);
        }
        state.content = content;
        let end = state.content.len();
        state.selection = end..end;
        state.selection_reversed = false;
        state.marked_range = None;
        state.composition_origin = None;
    }

    /// puts the caret at a byte offset. setting content alone parks it at the
    /// end, which is wrong for text spliced into the middle of a line.
    pub fn set_cursor(&self, offset: usize) {
        let mut state = self.state.borrow_mut();
        let offset = valid_boundary(&state.content, offset);
        state.selection = offset..offset;
        state.selection_reversed = false;
    }

    pub fn set_placeholder(&self, placeholder: impl Into<String>) {
        *self.placeholder.borrow_mut() = placeholder.into();
    }

    /// paints the given runs of the line apart from the rest. an empty list
    /// returns the line to one colour.
    pub fn set_accents(&self, accents: Vec<AccentSpan>) {
        *self.accents.borrow_mut() = accents;
    }

    pub fn subscribe<V: 'static>(
        &self,
        cx: &mut Context<V>,
        mut listener: impl FnMut(&mut V, &mut Context<V>) + 'static,
    ) -> Subscription {
        cx.subscribe(&self.view, move |owner, _, _: &TextInputEvent, cx| {
            listener(owner, cx);
        })
    }

    #[must_use]
    pub fn element(&self) -> gpui::AnyElement {
        self.view.clone().into_any_element()
    }
}

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("SuperiorityInput")),
        KeyBinding::new("delete", Delete, Some("SuperiorityInput")),
        KeyBinding::new("left", Left, Some("SuperiorityInput")),
        KeyBinding::new("right", Right, Some("SuperiorityInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("SuperiorityInput")),
        KeyBinding::new("shift-right", SelectRight, Some("SuperiorityInput")),
        KeyBinding::new("alt-left", WordLeft, Some("SuperiorityInput")),
        KeyBinding::new("alt-right", WordRight, Some("SuperiorityInput")),
        KeyBinding::new("alt-shift-left", SelectWordLeft, Some("SuperiorityInput")),
        KeyBinding::new("alt-shift-right", SelectWordRight, Some("SuperiorityInput")),
        KeyBinding::new("cmd-left", Home, Some("SuperiorityInput")),
        KeyBinding::new("cmd-right", End, Some("SuperiorityInput")),
        KeyBinding::new("cmd-shift-left", SelectHome, Some("SuperiorityInput")),
        KeyBinding::new("cmd-shift-right", SelectEnd, Some("SuperiorityInput")),
        KeyBinding::new("cmd-a", SelectAll, Some("SuperiorityInput")),
        KeyBinding::new("cmd-v", Paste, Some("SuperiorityInput")),
        KeyBinding::new("cmd-c", Copy, Some("SuperiorityInput")),
        KeyBinding::new("cmd-x", Cut, Some("SuperiorityInput")),
        KeyBinding::new("cmd-z", Undo, Some("SuperiorityInput")),
        KeyBinding::new("cmd-shift-z", Redo, Some("SuperiorityInput")),
        KeyBinding::new(
            "ctrl-cmd-space",
            ShowCharacterPalette,
            Some("SuperiorityInput"),
        ),
    ]);
}

struct TextInputView {
    state: Rc<RefCell<InputState>>,
    focus_handle: FocusHandle,
    placeholder: Rc<RefCell<String>>,
    accents: Rc<RefCell<Vec<AccentSpan>>>,
    ink: Rc<RefCell<FieldInk>>,
    cursor_visible: bool,
    cursor_blinking: bool,
    cursor_blink_task: Task<()>,
    focus_subscriptions: Vec<Subscription>,
}

struct TextInputEvent;

impl EventEmitter<TextInputEvent> for TextInputView {}

impl TextInputView {
    fn start_cursor_blink(&mut self, cx: &mut Context<Self>) {
        self.cursor_blinking = true;
        self.reset_cursor_blink(cx);
    }

    fn stop_cursor_blink(&mut self, cx: &mut Context<Self>) {
        self.cursor_blinking = false;
        self.cursor_visible = false;
        self.cursor_blink_task = Task::ready(());
        cx.notify();
    }

    fn reset_cursor_blink(&mut self, cx: &mut Context<Self>) {
        if !self.cursor_blinking {
            return;
        }
        self.cursor_visible = true;
        self.cursor_blink_task = Self::spawn_cursor_blink(cx);
        cx.notify();
    }

    fn spawn_cursor_blink(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(CURSOR_BLINK_INTERVAL).await;
                if this
                    .update(cx, |input, cx| {
                        input.cursor_visible = !input.cursor_visible;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    fn update(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut InputState)) {
        self.reset_cursor_blink(cx);
        let changed = {
            let mut state = self.state.borrow_mut();
            let previous = state.content.clone();
            change(&mut state);
            previous != state.content
        };
        if changed {
            cx.emit(TextInputEvent);
        }
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            let target = if state.selection.is_empty() {
                previous_boundary(&state.content, state.cursor())
            } else {
                state.selection.start
            };
            state.move_to(target);
        });
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            let target = if state.selection.is_empty() {
                next_boundary(&state.content, state.cursor())
            } else {
                state.selection.end
            };
            state.move_to(target);
        });
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            state.select_to(previous_boundary(&state.content, state.cursor()));
        });
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            state.select_to(next_boundary(&state.content, state.cursor()));
        });
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            state.move_to(previous_word_boundary(&state.content, state.cursor()));
        });
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            state.move_to(next_word_boundary(&state.content, state.cursor()));
        });
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            state.select_to(previous_word_boundary(&state.content, state.cursor()));
        });
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            state.select_to(next_word_boundary(&state.content, state.cursor()));
        });
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| state.move_to(0));
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| state.move_to(state.content.len()));
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| state.select_to(0));
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| state.select_to(state.content.len()));
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            state.selection = 0..state.content.len();
            state.selection_reversed = false;
        });
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = false;
        self.update(cx, |state| {
            let range = if state.selection.is_empty() {
                let cursor = state.cursor();
                previous_boundary(&state.content, cursor)..cursor
            } else {
                state.selection.clone()
            };
            if !range.is_empty() {
                state.replace(range, "", true);
                changed = true;
            }
        });
        if changed {
            window.refresh();
        } else {
            window.play_system_bell();
        }
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = false;
        self.update(cx, |state| {
            let range = if state.selection.is_empty() {
                let cursor = state.cursor();
                cursor..next_boundary(&state.content, cursor)
            } else {
                state.selection.clone()
            };
            if !range.is_empty() {
                state.replace(range, "", true);
                changed = true;
            }
        });
        if changed {
            window.refresh();
        } else {
            window.play_system_bell();
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let state = self.state.borrow();
        if !state.selection.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                state.content[state.selection.clone()].to_owned(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        let selected = {
            let state = self.state.borrow();
            (!state.selection.is_empty()).then(|| state.content[state.selection.clone()].to_owned())
        };
        if let Some(selected) = selected {
            cx.write_to_clipboard(ClipboardItem::new_string(selected));
            self.update(cx, |state| state.replace(state.selection.clone(), "", true));
            window.refresh();
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.update(cx, |state| {
                state.replace(state.selection.clone(), &text, true);
            });
            window.refresh();
        }
    }

    fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            if let Some(snapshot) = state.undo.pop() {
                let current = state.snapshot();
                state.redo.push(current);
                state.restore(snapshot);
            }
        });
        window.refresh();
    }

    fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            if let Some(snapshot) = state.redo.pop() {
                let current = state.snapshot();
                state.undo.push(current);
                state.restore(snapshot);
            }
        });
        window.refresh();
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        self.update(cx, |state| {
            let index = state.index_for_position(event.position);
            if event.modifiers.shift {
                state.select_to(index);
            } else {
                state.move_to(index);
            }
            state.selecting = true;
        });
        self.start_cursor_blink(cx);
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            if state.selecting {
                state.select_to(state.index_for_position(event.position));
            }
        });
    }

    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| state.selecting = false);
    }
}

impl EntityInputHandler for TextInputView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let state = self.state.borrow();
        let range = state.range_from_utf16(range);
        adjusted_range.replace(state.range_to_utf16(range.clone()));
        state.content.get(range).map(ToOwned::to_owned)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let state = self.state.borrow();
        Some(UTF16Selection {
            range: state.range_to_utf16(state.selection.clone()),
            reversed: state.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let state = self.state.borrow();
        state
            .marked_range
            .clone()
            .map(|range| state.range_to_utf16(range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |state| {
            if let Some(origin) = state.composition_origin.take() {
                state.remember(origin);
            }
            state.marked_range = None;
        });
        window.refresh();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.update(cx, |state| {
            let range = range
                .map(|range| state.range_from_utf16(range))
                .or_else(|| state.marked_range.clone())
                .unwrap_or_else(|| state.selection.clone());
            if let Some(origin) = state.composition_origin.take() {
                state.remember(origin);
                state.replace(range, text, false);
            } else {
                state.replace(range, text, true);
            }
        });
        window.refresh();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.update(cx, |state| {
            if state.composition_origin.is_none() {
                state.composition_origin = Some(state.snapshot());
            }
            let range = range
                .map(|range| state.range_from_utf16(range))
                .or_else(|| state.marked_range.clone())
                .unwrap_or_else(|| state.selection.clone());
            let text = single_line(text);
            let start = range.start;
            state
                .content
                .replace_range(valid_range(&state.content, range), &text);
            state.marked_range = (!text.is_empty()).then_some(start..start + text.len());
            state.selection = selected
                .map(|range| state.range_from_utf16(range))
                .map_or_else(
                    || start + text.len()..start + text.len(),
                    |range| start + range.start..start + range.end,
                );
            state.selection_reversed = false;
        });
        window.refresh();
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let state = self.state.borrow();
        let line = state.last_layout.as_ref()?;
        let range = state.range_from_utf16(range);
        Some(Bounds::from_corners(
            point(
                bounds.left() + line.x_for_index(range.start) - state.scroll_x,
                bounds.top(),
            ),
            point(
                bounds.left() + line.x_for_index(range.end) - state.scroll_x,
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let state = self.state.borrow();
        let line = state.last_layout.as_ref()?;
        let bounds = state.last_bounds?;
        let index = line.index_for_x(point.x - bounds.left() + state.scroll_x)?;
        Some(state.offset_to_utf16(index))
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.update(cx, |state| {
            state.selection = state.range_from_utf16(range);
            state.selection_reversed = false;
        });
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.state.borrow().content.encode_utf16().count())
    }
}

impl Render for TextInputView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_subscriptions.is_empty() {
            let focus_handle = self.focus_handle.clone();
            let focus = cx.on_focus(&focus_handle, window, |input, _, cx| {
                input.start_cursor_blink(cx);
            });
            let blur = cx.on_blur(&focus_handle, window, |input, _, cx| {
                input.stop_cursor_blink(cx);
            });
            self.focus_subscriptions = vec![focus, blur];
            if self.focus_handle.is_focused(window) {
                self.start_cursor_blink(cx);
            }
        }
        div()
            .id(("superiority-input", cx.entity_id()))
            .key_context("SuperiorityInput")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .items_center()
            .overflow_hidden()
            .cursor(gpui::CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .child(TextElement {
                input: cx.entity(),
                placeholder: self.placeholder.borrow().clone(),
                accents: self.accents.borrow().clone(),
            })
    }
}

struct TextElement {
    input: Entity<TextInputView>,
    placeholder: String,
    accents: Vec<AccentSpan>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll_x: Pixels,
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let state = input.state.borrow();
        let style = window.text_style();
        let focused = input.focus_handle.is_focused(window);
        let ink = *input.ink.borrow();
        let (text, color) = if state.content.is_empty() {
            (self.placeholder.clone(), rgb(ink.placeholder).into())
        } else {
            (state.content.clone(), style.color)
        };
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked) = &state.marked_range {
            [
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: text.len().saturating_sub(marked.end),
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect::<Vec<_>>()
        } else {
            vec![run]
        };
        // accents are applied after the fact rather than shaped separately, so
        // they compose with an in-flight IME range instead of fighting it.
        let runs = if state.content.is_empty() {
            runs
        } else {
            self.accents.iter().fold(runs, |runs, accent| {
                restyle(
                    runs,
                    valid_boundary(&text, accent.range.start)
                        ..valid_boundary(&text, accent.range.end),
                    rgb(accent.color).into(),
                    accent.background.map(|color| rgba(color).into()),
                )
            })
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(text.into(), font_size, &runs, None);
        let cursor_x = line.x_for_index(state.cursor().min(line.text.len()));
        let viewport = bounds.size.width.max(px(1.0));
        let max_scroll = (line.width - viewport).max(px(0.0));
        let mut scroll_x = state.scroll_x.min(max_scroll);
        if cursor_x < scroll_x {
            scroll_x = cursor_x;
        } else if cursor_x > scroll_x + viewport - px(2.0) {
            scroll_x = (cursor_x - viewport + px(2.0)).min(max_scroll);
        }
        let x = |offset| bounds.left() + line.x_for_index(offset) - scroll_x;
        let cursor = (focused && input.cursor_visible && state.selection.is_empty()).then(|| {
            fill(
                Bounds::new(
                    point(x(state.cursor()), bounds.top()),
                    size(px(ink.cursor_width), bounds.size.height),
                ),
                rgb(ink.cursor),
            )
        });
        let selection = (!state.selection.is_empty()).then(|| {
            let left = x(state.selection.start).max(bounds.left());
            let right = x(state.selection.end).min(bounds.right());
            fill(
                Bounds::from_corners(point(left, bounds.top()), point(right, bounds.bottom())),
                rgba(SELECTION_BACKGROUND),
            )
        });
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
            scroll_x,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        state: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = state.selection.take() {
            window.paint_quad(selection);
        }
        let line = state.line.take().expect("input line must be shaped");
        line.paint(
            point(bounds.left() - state.scroll_x, bounds.top()),
            window.line_height(),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        )
        .expect("input text must paint");
        if let Some(cursor) = state.cursor.take() {
            window.paint_quad(cursor);
        }
        self.input.update(cx, |input, _| {
            let mut input = input.state.borrow_mut();
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
            input.scroll_x = state.scroll_x;
        });
    }
}

/// repaints one byte range of an already-split run list, cutting the runs it
/// straddles rather than replacing them, so an IME underline survives it.
fn restyle(
    runs: Vec<TextRun>,
    range: Range<usize>,
    color: Hsla,
    background: Option<Hsla>,
) -> Vec<TextRun> {
    if range.is_empty() {
        return runs;
    }
    let mut restyled = Vec::with_capacity(runs.len() + 2);
    let mut offset = 0;
    for run in runs {
        let end = offset + run.len;
        let start = range.start.max(offset);
        let stop = range.end.min(end);
        if start >= stop {
            offset = end;
            restyled.push(run);
            continue;
        }
        if start > offset {
            restyled.push(TextRun {
                len: start - offset,
                ..run.clone()
            });
        }
        restyled.push(TextRun {
            len: stop - start,
            color,
            background_color: background,
            ..run.clone()
        });
        if stop < end {
            restyled.push(TextRun {
                len: end - stop,
                ..run.clone()
            });
        }
        offset = end;
    }
    restyled
}

fn single_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

fn valid_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn valid_range(text: &str, range: Range<usize>) -> Range<usize> {
    let start = valid_boundary(text, range.start);
    let end = valid_boundary(text, range.end.max(start));
    start..end
}

fn previous_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .find_map(|(index, _)| (index < offset).then_some(index))
        .unwrap_or(0)
}

fn next_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .find_map(|(index, _)| (index > offset).then_some(index))
        .unwrap_or(text.len())
}

fn previous_word_boundary(text: &str, offset: usize) -> usize {
    let before = &text[..valid_boundary(text, offset)];
    before
        .split_word_bound_indices()
        .rev()
        .find_map(|(index, segment)| (!segment.trim().is_empty()).then_some(index))
        .unwrap_or(0)
}

fn next_word_boundary(text: &str, offset: usize) -> usize {
    let offset = valid_boundary(text, offset);
    text[offset..]
        .split_word_bound_indices()
        .skip_while(|(_, segment)| segment.trim().is_empty())
        .nth(1)
        .map_or(text.len(), |(index, _)| offset + index)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct InputHarness {
        input: TextInput,
        mirrored: String,
        _subscription: Subscription,
    }

    impl Render for InputHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size(px(320.0)).child(self.input.element())
        }
    }

    #[test]
    fn grapheme_navigation_keeps_combining_sequences_together() {
        let text = "a\u{301}b";
        assert_eq!(next_boundary(text, 0), "a\u{301}".len());
        assert_eq!(previous_boundary(text, text.len()), "a\u{301}".len());
    }

    #[test]
    fn utf16_ranges_round_trip_supplementary_characters() {
        let state = InputState {
            content: "a😀b".to_owned(),
            ..InputState::default()
        };
        assert_eq!(state.range_from_utf16(1..3), 1..5);
        assert_eq!(state.range_to_utf16(1..5), 1..3);
    }

    #[test]
    fn replacement_is_single_line_and_undoable() {
        let mut state = InputState::default();
        state.replace(0..0, "alpha\nbeta", true);
        assert_eq!(state.content, "alpha beta");
        let original = state.undo.pop().expect("edit should create history");
        state.restore(original);
        assert!(state.content.is_empty());
    }

    #[gpui::test]
    fn focused_input_accepts_platform_text(cx: &mut gpui::TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| {
            let input = TextInput::new("placeholder", cx);
            let subscription = input.subscribe(cx, |view: &mut InputHarness, cx| {
                view.mirrored = view.input.content();
                cx.notify();
            });
            InputHarness {
                input,
                mirrored: String::new(),
                _subscription: subscription,
            }
        });
        view.update_in(cx, |view, window, cx| view.input.focus(window, cx));
        cx.simulate_input("hello world");
        view.update(cx, |view, _| {
            assert_eq!(view.input.content(), "hello world");
            assert_eq!(view.mirrored, "hello world");
        });
    }

    #[gpui::test]
    fn focused_cursor_blinks_and_input_restores_visibility(cx: &mut gpui::TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| {
            let input = TextInput::new("placeholder", cx);
            let subscription = input.subscribe(cx, |_: &mut InputHarness, _| {});
            InputHarness {
                input,
                mirrored: String::new(),
                _subscription: subscription,
            }
        });
        view.update_in(cx, |view, window, cx| view.input.focus(window, cx));
        let input_view = view.update(cx, |view, _| view.input.view.clone());
        assert!(input_view.update(cx, |input, _| input.cursor_visible));

        cx.executor().advance_clock(CURSOR_BLINK_INTERVAL);
        cx.run_until_parked();
        assert!(!input_view.update(cx, |input, _| input.cursor_visible));

        cx.simulate_input("x");
        assert!(input_view.update(cx, |input, _| input.cursor_visible));
    }
}
