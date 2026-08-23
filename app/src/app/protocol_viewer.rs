use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use gpui::WindowControlArea;
use gpui::{
    AnyElement, App, Bounds, Context, Div, FocusHandle, Focusable, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, Rgba, ScrollStrategy, Subscription, TitlebarOptions,
    UniformListScrollHandle, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
    uniform_list,
};
use superiority_ui::{
    foundation::{WithScrollbar as _, text_input as ui_text_input},
    products::sc2::{
        Sc2Assets,
        components::{
            controls as ui_controls, inspector as ui_inspector, roster as ui_roster,
            workspace as ui_workspace,
        },
        theme::{FONT_INTERFACE, FONT_INTERNATIONAL},
    },
};

use superiority_core::native::inspect::{
    Capture, Direction, Field, FieldRole, capture_paused, clear_capture, live_capture_after,
    sample_capture, set_capture_paused,
};

use super::client::{chrome::Assets, platform};

mod field_help;

const WINDOW_WIDTH: f32 = 1360.0;
const WINDOW_HEIGHT: f32 = 820.0;
const RECORD_PANE_WIDTH: f32 = 300.0;
const FIELD_PANE_WIDTH: f32 = 500.0;
const RECORD_ROW_HEIGHT: f32 = 50.0;
const FIELD_ROW_HEIGHT: f32 = 28.0;
const FIELD_INDENT: f32 = 17.0;
const FIELD_TOGGLE_SIZE: f32 = 14.0;
const BIT_RANGE_GUTTER: f32 = 96.0;
const BIT_ROW_HEIGHT: f32 = 51.0;
const BYTE_ROW_HEIGHT: f32 = 24.0;
const SPLITTER_SIZE: f32 = 7.0;
const BIT_BYTE_WIDTH: f32 = 64.0;
const BIT_LABEL_WIDTH: f32 = 34.0;
const BIT_LABEL_GAP: f32 = 5.0;
const MIN_STREAM_WIDTH: f32 = 360.0;
const MIN_BYTE_PANE_HEIGHT: f32 = 128.0;
const MIN_RECORD_PANE_WIDTH: f32 = 270.0;
const CONTENT_TOP: f32 = 40.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FilterTarget {
    Records,
    Fields,
}

fn filter_target_at(
    x: f32,
    y: f32,
    viewport_width: f32,
    record_width: f32,
    field_width: f32,
    has_fields: bool,
) -> Option<FilterTarget> {
    if y < CONTENT_TOP {
        None
    } else if x < record_width {
        Some(FilterTarget::Records)
    } else if has_fields && x >= viewport_width - field_width {
        Some(FilterTarget::Fields)
    } else {
        None
    }
}

fn moved_position(items: &[usize], selected: usize, delta: isize) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let position = items.iter().position(|item| *item == selected);
    Some(match position {
        Some(position) => position
            .saturating_add_signed(delta)
            .min(items.len().saturating_sub(1)),
        None if delta < 0 => items.len() - 1,
        None => 0,
    })
}

fn boundary_position(item_count: usize, end: bool) -> Option<usize> {
    (item_count > 0).then_some(if end { item_count - 1 } else { 0 })
}

#[derive(Clone, Copy, Debug)]
enum PaneResize {
    Records { pointer: f32, width: f32 },
    Fields { pointer: f32, width: f32 },
    Bitstream { pointer: f32, height: f32 },
}

struct InspectorTooltip {
    title: String,
    detail: String,
    assets: Sc2Assets,
}

impl gpui::Render for InspectorTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let title_lines = self.title.len().div_ceil(46).clamp(1, 3) as f32;
        let detail_lines = self.detail.len().div_ceil(58).clamp(2, 5) as f32;
        let height = 36.0 + title_lines * 19.0 + detail_lines * 17.0;
        let tooltip = ui_controls::tooltip_shell(410.0, height, self.assets.tooltip_fill.clone())
            .font_family(FONT_INTERNATIONAL)
            .child(
                div()
                    .relative()
                    .w_full()
                    .h_full()
                    .px(px(18.0))
                    .py(px(14.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w_full()
                            .flex_shrink_0()
                            .font_family("monospace")
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_size(px(12.5))
                            .line_height(px(19.0))
                            .text_color(rgb(0x00d6_e0f0))
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex_shrink_0()
                            .text_size(px(12.5))
                            .line_height(px(17.0))
                            .text_color(rgb(0x0085_d1ff))
                            .child(self.detail.clone()),
                    ),
            );
        ui_controls::animated_tooltip(tooltip, "protocol-tooltip-open", 0.0, 0.0, -8.0)
    }
}

fn with_inspector_tooltip(
    element: impl StatefulInteractiveElement + IntoElement,
    title: impl Into<String>,
    detail: impl Into<String>,
    assets: Sc2Assets,
) -> impl IntoElement {
    let title = title.into();
    let detail = detail.into();
    element.tooltip(move |_, cx| {
        cx.new(|_| InspectorTooltip {
            title: title.clone(),
            detail: detail.clone(),
            assets: assets.clone(),
        })
        .into()
    })
}

fn capture_toggle_icon(paused: bool) -> AnyElement {
    if paused {
        div()
            .w(px(13.0))
            .h(px(14.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .font_family(FONT_INTERFACE)
            .font_weight(gpui::FontWeight::BOLD)
            .text_size(px(11.0))
            .child("▶")
            .into_any_element()
    } else {
        div()
            .w(px(13.0))
            .h(px(14.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(3.0))
            .child(
                div()
                    .w(px(2.0))
                    .h(px(10.0))
                    .rounded(px(1.0))
                    .bg(rgb(0x008a_a2b5)),
            )
            .child(
                div()
                    .w(px(2.0))
                    .h(px(10.0))
                    .rounded(px(1.0))
                    .bg(rgb(0x008a_a2b5)),
            )
            .into_any_element()
    }
}

fn clear_icon() -> impl IntoElement {
    div()
        .relative()
        .w(px(14.0))
        .h(px(14.0))
        .flex_shrink_0()
        .child(
            div()
                .absolute()
                .left(px(2.0))
                .right(px(2.0))
                .top(px(3.0))
                .h(px(1.0))
                .bg(rgb(0x008a_a2b5)),
        )
        .child(
            div()
                .absolute()
                .left(px(4.0))
                .right(px(4.0))
                .top(px(1.0))
                .h(px(1.0))
                .bg(rgb(0x008a_a2b5)),
        )
        .child(
            div()
                .absolute()
                .left(px(3.0))
                .right(px(3.0))
                .top(px(5.0))
                .bottom(px(1.0))
                .rounded(px(1.0))
                .border_1()
                .border_color(rgb(0x008a_a2b5)),
        )
}

/// direction reads before service does: outgoing is the tone orange, incoming
/// the online green. nothing else in the rail wears either colour.
fn direction_color(direction: Direction) -> Rgba {
    match direction {
        Direction::Outgoing => rgb(0x00f0_aa64),
        Direction::Incoming => rgb(0x0047_d184),
    }
}

fn direction_badge(direction: Direction) -> impl IntoElement {
    let color = direction_color(direction);
    div()
        .flex_shrink_0()
        .px(px(6.0))
        .py(px(1.0))
        .border_1()
        .border_color(color.alpha(0.5))
        .font_family("monospace")
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_size(px(9.5))
        .text_color(color)
        .child(direction.label().to_uppercase())
}

fn search_icon(color: Rgba) -> impl IntoElement {
    div()
        .relative()
        .size(px(12.0))
        .flex_shrink_0()
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(1.0))
                .size(px(8.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(color),
        )
        .child(
            div()
                .absolute()
                .left(px(7.0))
                .top(px(8.0))
                .size(px(2.0))
                .bg(color),
        )
        .child(
            div()
                .absolute()
                .left(px(8.5))
                .top(px(9.5))
                .size(px(2.0))
                .bg(color),
        )
}

fn service_color(service: &str) -> Rgba {
    match service {
        "Authentication" => rgb(0x00f0_b35a),
        "Connection" => rgb(0x0063_b8f2),
        "Chat" => rgb(0x005f_d1d0),
        "Friends" => rgb(0x0072_d69b),
        "Presence" => rgb(0x00a5_d86c),
        "Profile" => rgb(0x00b9_9af2),
        "Toon" => rgb(0x00e8_8bc8),
        "Cache" => rgb(0x0077_cdb4),
        "Club" | "S2Multiplayer" => rgb(0x00f0_9667),
        "S2Master" => rgb(0x00e8_7b8e),
        "GameUtilities" => rgb(0x00d8_c66a),
        _ => {
            const COLORS: [u32; 8] = [
                0x0062_b7e8,
                0x0068_c7b0,
                0x009c_cf70,
                0x00e0_bd67,
                0x00e3_8b73,
                0x00d1_81b4,
                0x00a5_94e6,
                0x007a_a7e8,
            ];
            let index = service.bytes().fold(0usize, |hash, byte| {
                hash.wrapping_mul(31).wrapping_add(usize::from(byte))
            }) % COLORS.len();
            rgb(COLORS[index])
        }
    }
}

/// booleans are the one value worth colouring: `true` is the thing you scan a
/// decoded header for. everything else stays the one value tone.
fn field_value_color(value: &str, active: bool) -> Rgba {
    match value {
        "true" => rgb(0x0047_d184),
        "false" => rgb(0x007d_8fa8),
        _ if active => rgb(0x00bd_eaff),
        _ => rgb(0x0067_ceff),
    }
}

/// a record survives both filters at once: the chips narrow the capture to a
/// set of services, the search narrows what is left. neither drops a record
/// from the capture — the header keeps counting all of them.
fn record_matches(
    record: &superiority_core::native::inspect::Record,
    filter: &str,
    services: &HashSet<String>,
) -> bool {
    if !services.is_empty() && !services.contains(&record.service) {
        return false;
    }
    filter.is_empty()
        || record.service.to_lowercase().contains(filter)
        || record.command.to_lowercase().contains(filter)
        || record.type_name.to_lowercase().contains(filter)
        || record.direction.label().contains(filter)
}

/// every service present in the capture, busiest first. chips are a legend as
/// much as a filter, so the order has to hold still while traffic arrives —
/// ties break alphabetically rather than by arrival.
fn service_counts(records: &[superiority_core::native::inspect::Record]) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for record in records {
        match counts
            .iter_mut()
            .find(|(service, _)| *service == record.service)
        {
            Some((_, count)) => *count += 1,
            None => counts.push((record.service.clone(), 1)),
        }
    }
    counts.sort_by(|(left_service, left_count), (right_service, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_service.cmp(right_service))
    });
    counts
}

fn field_label(path: &str) -> String {
    let leaf = path.rsplit('.').next().unwrap_or(path);
    if let Some((_, index)) = leaf.rsplit_once('[')
        && leaf.ends_with(']')
        && leaf[..leaf.len() - index.len() - 1]
            .chars()
            .all(|character| character != ']')
    {
        return format!("[{index}");
    }
    leaf.to_owned()
}

pub fn run() {
    let resources = platform::resource_directory();
    platform::application()
        .with_assets(Assets {
            base: resources.clone(),
        })
        .run(move |cx: &mut App| {
            super::client::chrome::load_fonts(&resources, cx);
            ui_text_input::init(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            open(cx);
            cx.activate(true);
        });
}

pub(super) fn open(cx: &mut App) {
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
                cx,
            ))),
            window_min_size: Some(size(px(1020.0), px(620.0))),
            titlebar: Some(TitlebarOptions {
                title: Some("Superiority Protocol Viewer".into()),
                appears_transparent: true,
                ..Default::default()
            }),
            is_movable: true,
            app_owns_titlebar_drag: cfg!(target_os = "macos"),
            ..Default::default()
        },
        move |window, cx| {
            platform::configure_window(window);
            cx.new(ProtocolViewer::new)
        },
    )
    .expect("the protocol viewer must be able to open a window");
}

struct ProtocolViewer {
    focus_handle: FocusHandle,
    capture: Capture,
    selected_record: usize,
    selected_field: usize,
    record_filter: ui_text_input::TextInput,
    record_filter_value: String,
    service_filters: HashSet<String>,
    field_filter: ui_text_input::TextInput,
    field_filter_value: String,
    filter_target: FilterTarget,
    pointer_filter_target: Option<FilterTarget>,
    record_transition: Option<ui_roster::TimedTransition<(), usize, Instant>>,
    field_transition: Option<ui_roster::TimedTransition<(), usize, Instant>>,
    record_scroll: UniformListScrollHandle,
    bit_scroll: UniformListScrollHandle,
    byte_scroll: UniformListScrollHandle,
    field_scroll: UniformListScrollHandle,
    ui_assets: Sc2Assets,
    live: bool,
    following: bool,
    paused: bool,
    collapsed_fields: HashSet<String>,
    hovered_field: Option<usize>,
    record_pane_width: f32,
    field_pane_width: f32,
    bit_pane_width: f32,
    bitstream_height: Option<f32>,
    pane_resize: Option<PaneResize>,
    _subscriptions: Vec<Subscription>,
}

impl ProtocolViewer {
    fn new(cx: &mut Context<Self>) -> Self {
        let record_filter = ui_text_input::TextInput::new("Search records", cx);
        let record_subscription = record_filter.subscribe(cx, |this, cx| {
            this.set_record_filter(this.record_filter.content(), cx);
        });
        let field_filter = ui_text_input::TextInput::new("Search properties", cx);
        let field_subscription = field_filter.subscribe(cx, |this, cx| {
            this.set_field_filter(this.field_filter.content(), cx);
        });
        let live = live_capture_after(None);
        let has_live_records = !live.records.is_empty();
        let capture = if has_live_records {
            live
        } else {
            sample_capture()
        };
        let this = Self {
            focus_handle: cx.focus_handle(),
            capture,
            selected_record: 0,
            selected_field: 0,
            record_filter,
            record_filter_value: String::new(),
            service_filters: HashSet::new(),
            field_filter,
            field_filter_value: String::new(),
            filter_target: FilterTarget::Records,
            pointer_filter_target: None,
            record_transition: None,
            field_transition: None,
            record_scroll: UniformListScrollHandle::new(),
            bit_scroll: UniformListScrollHandle::new(),
            byte_scroll: UniformListScrollHandle::new(),
            field_scroll: UniformListScrollHandle::new(),
            ui_assets: Sc2Assets::load(&superiority_ui::foundation::assets::NativeAssetResolver),
            live: has_live_records,
            following: true,
            paused: capture_paused(),
            collapsed_fields: HashSet::new(),
            hovered_field: None,
            record_pane_width: RECORD_PANE_WIDTH,
            field_pane_width: FIELD_PANE_WIDTH,
            bit_pane_width: WINDOW_WIDTH - RECORD_PANE_WIDTH - FIELD_PANE_WIDTH,
            bitstream_height: None,
            pane_resize: None,
            _subscriptions: vec![record_subscription, field_subscription],
        };
        let executor = cx.background_executor().clone();
        cx.spawn(async move |entity, cx| {
            loop {
                executor.timer(Duration::from_millis(100)).await;
                if entity.update(cx, ProtocolViewer::refresh_capture).is_err() {
                    break;
                }
            }
        })
        .detach();
        this
    }

    fn selected_record(&self) -> &superiority_core::native::inspect::Record {
        &self.capture.records[self.selected_record]
    }

    fn selected_field(&self) -> &Field {
        &self.selected_record().fields[self.selected_field]
    }

    fn select_record(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_record = index;
        self.selected_field = 0;
        self.hovered_field = None;
        self.following = index + 1 == self.capture.records.len();
        self.bit_scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui::point(px(0.0), px(0.0)));
        self.byte_scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui::point(px(0.0), px(0.0)));
        self.field_scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui::point(px(0.0), px(0.0)));
        cx.notify();
    }

    fn refresh_capture(&mut self, cx: &mut Context<Self>) {
        if self.paused {
            return;
        }
        let last_sequence = self
            .live
            .then(|| self.capture.records.last().map(|record| record.sequence))
            .flatten();
        let mut incoming = live_capture_after(last_sequence);
        if incoming.records.is_empty() {
            return;
        }
        let selected_sequence = self
            .capture
            .records
            .get(self.selected_record)
            .map(|record| record.sequence);
        if self.live {
            self.capture.records.append(&mut incoming.records);
            if self.capture.records.len() > 512 {
                self.capture
                    .records
                    .drain(..self.capture.records.len().saturating_sub(512));
            }
        } else {
            self.capture = incoming;
        }
        self.live = true;
        self.selected_record = if self.following {
            self.capture.records.len().saturating_sub(1)
        } else {
            selected_sequence
                .and_then(|sequence| {
                    self.capture
                        .records
                        .iter()
                        .position(|record| record.sequence == sequence)
                })
                .unwrap_or_else(|| self.capture.records.len().saturating_sub(1))
        };
        self.selected_field = self
            .selected_field
            .min(self.selected_record().fields.len().saturating_sub(1));
        self.hovered_field = None;
        cx.notify();
    }

    fn toggle_capture(&mut self, cx: &mut Context<Self>) {
        self.paused = !self.paused;
        set_capture_paused(self.paused);
        cx.notify();
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        clear_capture();
        self.capture.records.clear();
        self.live = true;
        self.selected_record = 0;
        self.selected_field = 0;
        cx.notify();
    }

    fn select_field(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_field = index;
        if let Some(position) = self
            .visible_field_indices()
            .iter()
            .position(|visible| *visible == index)
        {
            self.field_scroll
                .scroll_to_item(position, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    fn toggle_field(&mut self, index: usize, path: &str, cx: &mut Context<Self>) {
        let previous = self.visible_field_indices();
        self.selected_field = index;
        if !self.collapsed_fields.remove(path) {
            self.collapsed_fields.insert(path.to_owned());
        }
        let next = self.visible_field_indices();
        self.begin_field_transition(previous, &next);
        cx.notify();
    }

    fn select_bit(&mut self, bit: usize, cx: &mut Context<Self>) {
        let record = self.selected_record();
        if let Some(index) = record
            .fields
            .iter()
            .enumerate()
            .filter(|(_, field)| field.exact_range && bit >= field.start_bit && bit < field.end_bit)
            .max_by_key(|(_, field)| field.depth)
            .map(|(index, _)| index)
        {
            self.select_field(index, cx);
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" {
            match self.filter_target {
                FilterTarget::Records => {
                    self.record_filter.clear();
                    self.set_record_filter(String::new(), cx);
                }
                FilterTarget::Fields => {
                    self.field_filter.clear();
                    self.set_field_filter(String::new(), cx);
                }
            }
            self.focus_handle.focus(window, cx);
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if event.keystroke.modifiers.modified() {
            return;
        }
        let handled = match self.filter_target {
            FilterTarget::Records => match event.keystroke.key.as_str() {
                "up" => self.move_record_selection(-1, cx),
                "down" => self.move_record_selection(1, cx),
                "home" => self.select_record_boundary(false, cx),
                "end" => self.select_record_boundary(true, cx),
                _ => false,
            },
            FilterTarget::Fields => match event.keystroke.key.as_str() {
                "up" => self.move_field_selection(-1, cx),
                "down" => self.move_field_selection(1, cx),
                "home" => self.select_field_boundary(false, cx),
                "end" => self.select_field_boundary(true, cx),
                "left" if self.field_filter_value.is_empty() => self.navigate_field_left(cx),
                "right" if self.field_filter_value.is_empty() => self.navigate_field_right(cx),
                _ => false,
            },
        };
        if handled {
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn move_record_selection(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        let visible = self.filtered_record_indices();
        let Some(position) = moved_position(&visible, self.selected_record, delta) else {
            return false;
        };
        self.select_record(visible[position], cx);
        self.record_scroll
            .scroll_to_item(position, ScrollStrategy::Nearest);
        true
    }

    fn select_record_boundary(&mut self, end: bool, cx: &mut Context<Self>) -> bool {
        let visible = self.filtered_record_indices();
        let Some(position) = boundary_position(visible.len(), end) else {
            return false;
        };
        self.select_record(visible[position], cx);
        self.record_scroll.scroll_to_item(
            position,
            if end {
                ScrollStrategy::Bottom
            } else {
                ScrollStrategy::Top
            },
        );
        true
    }

    fn move_field_selection(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        if self.capture.records.is_empty() {
            return false;
        }
        let visible = self.visible_field_indices();
        let Some(position) = moved_position(&visible, self.selected_field, delta) else {
            return false;
        };
        self.select_field(visible[position], cx);
        true
    }

    fn select_field_boundary(&mut self, end: bool, cx: &mut Context<Self>) -> bool {
        if self.capture.records.is_empty() {
            return false;
        }
        let visible = self.visible_field_indices();
        let Some(position) = boundary_position(visible.len(), end) else {
            return false;
        };
        self.selected_field = visible[position];
        self.field_scroll.scroll_to_item(
            position,
            if end {
                ScrollStrategy::Bottom
            } else {
                ScrollStrategy::Top
            },
        );
        cx.notify();
        true
    }

    fn navigate_field_left(&mut self, cx: &mut Context<Self>) -> bool {
        if self.capture.records.is_empty() {
            return false;
        }
        let index = self.selected_field;
        let record = self.selected_record();
        let Some(field) = record.fields.get(index) else {
            return false;
        };
        let path = field.path.clone();
        let depth = field.depth;
        let container = record
            .fields
            .get(index + 1)
            .is_some_and(|next| next.depth > depth);
        if container && !self.collapsed_fields.contains(&path) {
            self.toggle_field(index, &path, cx);
            return true;
        }
        let Some(parent) = (0..index)
            .rev()
            .find(|candidate| record.fields[*candidate].depth < depth)
        else {
            return false;
        };
        self.select_field(parent, cx);
        true
    }

    fn navigate_field_right(&mut self, cx: &mut Context<Self>) -> bool {
        if self.capture.records.is_empty() {
            return false;
        }
        let index = self.selected_field;
        let record = self.selected_record();
        let Some(field) = record.fields.get(index) else {
            return false;
        };
        let path = field.path.clone();
        let depth = field.depth;
        let container = record
            .fields
            .get(index + 1)
            .is_some_and(|next| next.depth > depth);
        if !container {
            return false;
        }
        if self.collapsed_fields.contains(&path) {
            self.toggle_field(index, &path, cx);
        } else {
            self.select_field(index + 1, cx);
        }
        true
    }

    fn set_record_filter(&mut self, next: String, cx: &mut Context<Self>) {
        if self.record_filter_value == next {
            return;
        }
        let previous = self.filtered_record_indices();
        self.record_filter_value = next;
        let filtered = self.filtered_record_indices();
        self.begin_record_transition(previous, &filtered);
        cx.notify();
    }

    fn begin_record_transition(&mut self, previous: Vec<usize>, next: &[usize]) {
        self.record_transition = ui_roster::Transition::new(previous, next, |index| {
            u32::try_from(*index).unwrap_or(u32::MAX)
        })
        .map(|transition| ui_roster::TimedTransition {
            scope: (),
            transition,
            started: Instant::now(),
        });
    }

    fn set_field_filter(&mut self, next: String, cx: &mut Context<Self>) {
        if self.field_filter_value == next {
            return;
        }
        let previous = self.visible_field_indices();
        self.field_filter_value = next;
        let visible = self.visible_field_indices();
        self.begin_field_transition(previous, &visible);
        cx.notify();
    }

    fn begin_field_transition(&mut self, previous: Vec<usize>, next: &[usize]) {
        self.field_transition = ui_roster::Transition::new(previous, next, |index| {
            u32::try_from(*index).unwrap_or(u32::MAX)
        })
        .map(|transition| ui_roster::TimedTransition {
            scope: (),
            transition,
            started: Instant::now(),
        });
    }

    fn finish_resize(&mut self, cx: &mut Context<Self>) {
        if self.pane_resize.take().is_some() {
            cx.notify();
        }
    }

    fn normalize_pane_widths(&mut self, viewport_width: f32) {
        let available = (viewport_width - MIN_STREAM_WIDTH - SPLITTER_SIZE * 2.0).max(610.0);
        let excess = (self.record_pane_width + self.field_pane_width - available).max(0.0);
        let field_reduction = excess.min((self.field_pane_width - 340.0).max(0.0));
        self.field_pane_width -= field_reduction;
        self.record_pane_width =
            (self.record_pane_width - (excess - field_reduction)).max(MIN_RECORD_PANE_WIDTH);
    }

    fn update_resize(&mut self, event: &MouseMoveEvent, window: &Window, cx: &mut Context<Self>) {
        let Some(resize) = self.pane_resize else {
            return;
        };
        let pointer = event.position.x.as_f32();
        let viewport_width = window.viewport_size().width.as_f32();
        match resize {
            PaneResize::Records {
                pointer: start,
                width,
            } => {
                self.record_pane_width =
                    (width + pointer - start).clamp(MIN_RECORD_PANE_WIDTH, 480.0);
            }
            PaneResize::Fields {
                pointer: start,
                width,
            } => {
                self.field_pane_width = (width + start - pointer).clamp(340.0, 760.0);
            }
            PaneResize::Bitstream { .. } => return,
        }
        self.normalize_pane_widths(viewport_width);
        cx.notify();
    }

    fn update_vertical_resize(
        &mut self,
        event: &MouseMoveEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(PaneResize::Bitstream { pointer, height }) = self.pane_resize else {
            return;
        };
        let maximum = (window.viewport_size().height.as_f32() - 260.0).max(160.0);
        self.bitstream_height =
            Some((height + event.position.y.as_f32() - pointer).clamp(118.0, maximum));
        cx.notify();
    }

    fn horizontal_splitter(&self, records: bool, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(if records {
                "record-splitter"
            } else {
                "field-splitter"
            })
            .w(px(SPLITTER_SIZE))
            .h_full()
            .flex_shrink_0()
            .cursor(gpui::CursorStyle::ResizeLeftRight)
            .bg(rgb(0x0007_131d))
            .hover(|style| style.bg(rgb(0x001e_789e)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.pane_resize = Some(if records {
                        PaneResize::Records {
                            pointer: event.position.x.as_f32(),
                            width: this.record_pane_width,
                        }
                    } else {
                        PaneResize::Fields {
                            pointer: event.position.x.as_f32(),
                            width: this.field_pane_width,
                        }
                    });
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                if event.click_count() >= 2 {
                    if records {
                        this.record_pane_width = RECORD_PANE_WIDTH;
                    } else {
                        this.field_pane_width = FIELD_PANE_WIDTH;
                    }
                    cx.notify();
                }
            }))
    }

    fn focus_filter(&mut self, target: FilterTarget, window: &mut Window, cx: &mut Context<Self>) {
        let focused = match target {
            FilterTarget::Records => self.record_filter.is_focused(window),
            FilterTarget::Fields => self.field_filter.is_focused(window),
        };
        if self.filter_target == target && focused {
            return;
        }
        self.filter_target = target;
        self.pointer_filter_target = Some(target);
        match target {
            FilterTarget::Records => self.record_filter.focus(window, cx),
            FilterTarget::Fields => self.field_filter.focus(window, cx),
        }
        cx.notify();
    }

    fn update_filter_focus(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pane_resize.is_some() {
            return;
        }
        let target = filter_target_at(
            event.position.x.as_f32(),
            event.position.y.as_f32(),
            window.viewport_size().width.as_f32(),
            self.record_pane_width,
            self.field_pane_width,
            !self.capture.records.is_empty(),
        );
        if self.pointer_filter_target == target {
            return;
        }
        self.pointer_filter_target = target;
        if let Some(target) = target {
            self.focus_filter(target, window, cx);
        } else {
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    fn capture_controls(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let record_count = self.capture.records.len();
        let filtered_count = self.filtered_record_indices().len();
        let filtering = filtered_count != record_count;
        div()
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .child(
                ui_inspector::header_shell()
                    .id("protocol-record-filter-focus")
                    .cursor(gpui::CursorStyle::IBeam)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.focus_filter(FilterTarget::Records, window, cx);
                    }))
                    .child(ui_inspector::header_title("Records"))
                    .child(
                        ui_inspector::header_detail()
                            .font_family("monospace")
                            .text_size(px(12.0))
                            .child(record_count.to_string())
                            .when(filtering, |detail| {
                                detail
                                    .child(div().text_color(rgb(0x003f_5b6d)).child("·"))
                                    .child(
                                        div()
                                            .text_color(rgb(0x008f_a8bb))
                                            .child(format!("{filtered_count} shown")),
                                    )
                            }),
                    ),
            )
            .child(self.search_row(FilterTarget::Records, window, cx))
            .child(self.service_chips(cx))
    }

    /// the search box each pane wears under its header. the pointer already
    /// routes keystrokes to whichever column it is over, so this is the caret
    /// that tells you which one that is.
    fn search_row(
        &self,
        target: FilterTarget,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (input, focused) = match target {
            FilterTarget::Records => (&self.record_filter, self.record_filter.is_focused(window)),
            FilterTarget::Fields => (&self.field_filter, self.field_filter.is_focused(window)),
        };
        div()
            .w_full()
            .flex_shrink_0()
            .px(px(12.0))
            .pt(px(9.0))
            .pb(px(8.0))
            .child(
                ui_inspector::search_field(focused)
                    .id(match target {
                        FilterTarget::Records => "protocol-record-search",
                        FilterTarget::Fields => "protocol-field-search",
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.focus_filter(target, window, cx);
                        }),
                    )
                    .child(search_icon(if focused {
                        rgb(0x006b_c2f2)
                    } else {
                        rgb(0x005e_8291)
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .h_full()
                            .font_family("monospace")
                            .text_size(px(12.0))
                            .text_color(rgb(0x00d6_e0f0))
                            .child(input.element()),
                    ),
            )
    }

    /// service chips double as the capture's legend. filtering hides records,
    /// it never drops them — the header keeps counting the whole capture.
    fn service_chips(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let services = self.record_services();
        div()
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .gap(px(4.0))
            .px(px(12.0))
            .pb(px(9.0))
            .children(
                services
                    .into_iter()
                    .enumerate()
                    .map(|(position, (service, count))| {
                        let selected = self.service_filters.contains(&service);
                        let label = service.to_uppercase();
                        ui_inspector::filter_chip(("protocol-service", position), label, selected)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.toggle_service_filter(&service, cx);
                                this.focus_filter(FilterTarget::Records, window, cx);
                            }))
                            .child(
                                div()
                                    .ml(px(5.0))
                                    .font_family("monospace")
                                    .text_size(px(9.5))
                                    .text_color(if selected {
                                        rgb(0x008f_c9ea)
                                    } else {
                                        rgb(0x004c_657a)
                                    })
                                    .child(count.to_string()),
                            )
                    }),
            )
    }

    fn titlebar(&self, _window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let titlebar = div()
            .id("protocol-titlebar")
            .relative()
            .h(px(34.0))
            .w_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(rgb(0x0010_2a3b))
            .bg(rgb(0x0007_111d))
            .child(div().h_full().flex_1())
            .child(
                div()
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(self.capture_toggle_button(cx))
                    .child(self.clear_capture_button(cx)),
            )
            .child(div().h_full().flex_1());
        #[cfg(target_os = "windows")]
        let titlebar = titlebar
            .window_control_area(WindowControlArea::Drag)
            .pr(px(platform::WINDOW_CONTROLS_WIDTH))
            .child(platform::window_controls_with_height(_window, 34.0));
        #[cfg(target_os = "macos")]
        let titlebar = titlebar.on_mouse_down(MouseButton::Left, |_, window, cx| {
            cx.stop_propagation();
            platform::begin_window_drag(window);
        });
        titlebar
    }

    fn capture_toggle_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        with_inspector_tooltip(
            ui_inspector::toolbar_icon_button(
                "protocol-capture-toggle",
                if self.paused { "Resume" } else { "Pause" },
                capture_toggle_icon(self.paused),
                self.paused,
            )
            .on_click(cx.listener(|this, _, _, cx| this.toggle_capture(cx))),
            if self.paused {
                "Resume capture"
            } else {
                "Pause capture"
            },
            if self.paused {
                "Continue collecting HTTP, BGS, and native protocol records."
            } else {
                "Freeze this record list without disconnecting the client."
            },
            self.ui_assets.clone(),
        )
    }

    fn clear_capture_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        with_inspector_tooltip(
            ui_inspector::toolbar_icon_button(
                "protocol-capture-clear",
                "Clear",
                clear_icon(),
                false,
            )
            .on_click(cx.listener(|this, _, _, cx| this.clear(cx))),
            "Clear capture",
            "Remove the records currently held by this viewer.",
            self.ui_assets.clone(),
        )
    }

    fn filtered_record_indices(&self) -> Vec<usize> {
        let filter = self.record_filter_value.to_lowercase();
        self.capture
            .records
            .iter()
            .enumerate()
            .filter_map(|(index, record)| {
                record_matches(record, &filter, &self.service_filters).then_some(index)
            })
            .collect()
    }

    fn record_services(&self) -> Vec<(String, usize)> {
        service_counts(&self.capture.records)
    }

    fn toggle_service_filter(&mut self, service: &str, cx: &mut Context<Self>) {
        let previous = self.filtered_record_indices();
        if !self.service_filters.remove(service) {
            self.service_filters.insert(service.to_owned());
        }
        let filtered = self.filtered_record_indices();
        self.begin_record_transition(previous, &filtered);
        cx.notify();
    }

    fn record_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let record = &self.capture.records[index];
        let selected = index == self.selected_record;
        let category_color = service_color(&record.service);
        let category_text =
            rgb(0x0078_93a9).blend(category_color.alpha(if selected { 0.62 } else { 0.48 }));
        let direction = direction_color(record.direction);
        ui_inspector::rail_row(("protocol-record", index), selected)
            .h(px(RECORD_ROW_HEIGHT))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_record(index, cx);
                this.focus_filter(FilterTarget::Records, window, cx);
            }))
            .child(
                div()
                    .h_full()
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .child(
                        div()
                            .w(px(32.0))
                            .flex_shrink_0()
                            .font_family("monospace")
                            .text_size(px(10.5))
                            .text_color(direction.alpha(if selected { 1.0 } else { 0.82 }))
                            .child(format!("{} {:02}", record.direction.marker(), index + 1)),
                    )
                    .child(
                        div()
                            .min_w(px(0.0))
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(13.5))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(if selected {
                                        rgb(0x00e6_f9ff)
                                    } else {
                                        rgb(0x00d5_e5f4)
                                    })
                                    .child(record.command.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.5))
                                    .text_color(category_text)
                                    .child(record.service.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .font_family("monospace")
                            .text_size(px(10.5))
                            .text_color(if selected {
                                rgb(0x008e_bbd0)
                            } else {
                                rgb(0x0053_6f84)
                            })
                            .child(format!("{} B", record.bytes.len())),
                    ),
            )
            .into_any_element()
    }

    fn records_pane(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered = self.filtered_record_indices();
        let filtered_count = filtered.len();
        let now = Instant::now();
        let animating = self
            .record_transition
            .as_ref()
            .is_some_and(|transition| transition.is_running(now));
        let rows = animating.then(|| {
            ui_roster::animated_rows_with_height(
                filtered.clone(),
                self.record_transition.as_ref(),
                now,
                |index| u32::try_from(*index).unwrap_or(u32::MAX),
                RECORD_ROW_HEIGHT,
                0.0,
                |index, _| self.record_row(*index, cx),
            )
        });
        let record_scroll = self.record_scroll.0.borrow().base_handle.clone();
        let list = uniform_list(
            "protocol-record-scroll",
            filtered_count,
            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                let indices = this.filtered_record_indices();
                range
                    .filter_map(|position| indices.get(position).copied())
                    .map(|index| this.record_row(index, cx))
                    .collect::<Vec<_>>()
            }),
        )
        .size_full()
        .track_scroll(&self.record_scroll);
        ui_inspector::pane()
            .id("protocol-record-pane")
            .relative()
            .w(px(self.record_pane_width))
            .flex_shrink_0()
            .border_r_1()
            .border_color(rgb(0x0017_384d))
            .child(self.capture_controls(window, cx))
            .child(
                div()
                    .id("protocol-record-viewport")
                    .flex_1()
                    .min_h(px(0.0))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.focus_filter(FilterTarget::Records, window, cx);
                        }),
                    )
                    .when_some(rows, |layer, rows| {
                        layer
                            .overflow_y_scroll()
                            .track_scroll(&record_scroll)
                            .children(rows)
                    })
                    .when(!animating, |layer| layer.child(list))
                    .vertical_scrollbar_for(&record_scroll, window, cx),
            )
            .when(
                self.pointer_filter_target == Some(FilterTarget::Records),
                |pane| pane.child(ui_inspector::focus_outline()),
            )
    }

    fn stream_pane(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let record = self.selected_record();
        let selected = self.selected_field();
        let selection_is_exact = selected.exact_range;
        let bytes_per_row = self.bytes_per_bit_row();
        let bit_row_count = record.bytes.len().div_ceil(bytes_per_row);
        let bitstream_height = self
            .bitstream_height
            .unwrap_or((bit_row_count as f32 * BIT_ROW_HEIGHT + 24.0).clamp(118.0, 460.0));
        let bit_scroll = self.bit_scroll.0.borrow().base_handle.clone();
        let bit_rows = uniform_list(
            "protocol-bit-scroll",
            bit_row_count,
            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|row_index| this.bit_row(row_index, this.bytes_per_bit_row(), cx))
                    .collect::<Vec<_>>()
            }),
        )
        .h_full()
        .p(px(12.0))
        .track_scroll(&self.bit_scroll);
        let byte_scroll = self.byte_scroll.0.borrow().base_handle.clone();
        let byte_row_count = record.bytes.len().div_ceil(16);
        let byte_rows = uniform_list(
            "protocol-byte-scroll",
            byte_row_count,
            cx.processor(|this, range: std::ops::Range<usize>, _, _| {
                range.map(|row_index| this.byte_row(row_index)).collect()
            }),
        )
        .size_full()
        .track_scroll(&self.byte_scroll);
        ui_inspector::pane()
            .flex_1()
            .min_w(px(0.0))
            .bg(rgb(0x0003_0a10))
            .on_children_prepainted({
                let viewer = cx.weak_entity();
                move |bounds, _, cx| {
                    let Some(bounds) = bounds.first() else {
                        return;
                    };
                    let width = bounds.size.width.as_f32();
                    let _ = viewer.update(cx, |this, cx| {
                        if (this.bit_pane_width - width).abs() > 1.0 {
                            this.bit_pane_width = width;
                            cx.notify();
                        }
                    });
                }
            })
            .child(with_inspector_tooltip(
                ui_inspector::header(
                    "Bitstream",
                    format!(
                        "{} bytes · {} logical bits",
                        record.bytes.len(),
                        record.logical_bits
                    ),
                )
                .id("protocol-bitstream-help"),
                "Bitstream",
                "Select a decoded property or click any bit to inspect its exact wire range.",
                self.ui_assets.clone(),
            ))
            .child(
                div()
                    .id("protocol-bit-viewport")
                    .relative()
                    .h(px(bitstream_height))
                    .min_h(px(0.0))
                    .child(bit_rows)
                    .vertical_scrollbar_for(&bit_scroll, window, cx),
            )
            .child(
                div()
                    .id("protocol-bitstream-splitter")
                    .h(px(SPLITTER_SIZE))
                    .w_full()
                    .flex_shrink_0()
                    .cursor(gpui::CursorStyle::ResizeUpDown)
                    .bg(rgb(0x0007_131d))
                    .hover(|style| style.bg(rgb(0x001e_789e)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.pane_resize = Some(PaneResize::Bitstream {
                                pointer: event.position.y.as_f32(),
                                height: bitstream_height,
                            });
                            cx.stop_propagation();
                        }),
                    )
                    .on_click(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
                        if event.click_count() >= 2 {
                            this.bitstream_height = None;
                            cx.notify();
                        }
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(MIN_BYTE_PANE_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .border_t_1()
                    .border_color(rgb(0x0017_384d))
                    .bg(rgb(0x0005_0d14))
                    .child(
                        div()
                            .h(px(34.0))
                            .flex()
                            .items_center()
                            .px(px(13.0))
                            .border_b_1()
                            .border_color(rgb(0x0010_2a3b))
                            .font_family(FONT_INTERFACE)
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_size(px(12.5))
                            .text_color(rgb(0x006b_8498))
                            .child(if selection_is_exact {
                                "Selected range"
                            } else {
                                "Payload bytes"
                            })
                            .child(
                                div()
                                    .ml_auto()
                                    .font_family("monospace")
                                    .text_color(if selection_is_exact {
                                        rgb(0x0048_bff0)
                                    } else {
                                        rgb(0x006b_8498)
                                    })
                                    .child(if selection_is_exact {
                                        format!(
                                            "bits [{}, {}) · {} bits",
                                            selected.start_bit,
                                            selected.end_bit,
                                            selected.end_bit - selected.start_bit
                                        )
                                    } else {
                                        "Select a field with a traced range".to_owned()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h(px(0.0))
                            .px(px(12.0))
                            .py(px(7.0))
                            .child(byte_rows)
                            .vertical_scrollbar_for(&byte_scroll, window, cx),
                    ),
            )
    }

    /// the row the pointer is resting on, when it traces a real wire range and
    /// isn't already the selected one. both the bitstream and the hex pane read
    /// this, so a hover lights up the same span in each at once.
    fn hinted_range(&self) -> Option<(usize, usize)> {
        let hovered = self.hovered_field?;
        if hovered == self.selected_field {
            return None;
        }
        let field = self.selected_record().fields.get(hovered)?;
        field
            .exact_range
            .then_some((field.start_bit, field.end_bit))
    }

    fn bytes_per_bit_row(&self) -> usize {
        (((self.bit_pane_width - 24.0 - BIT_LABEL_WIDTH - BIT_LABEL_GAP) / BIT_BYTE_WIDTH).floor()
            as usize)
            .clamp(1, 32)
    }

    fn bit_row(
        &self,
        row_index: usize,
        bytes_per_row: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let record = self.selected_record();
        let selected = self.selected_field();
        let selection_is_exact = selected.exact_range;
        let hinted_range = self.hinted_range();
        let start = row_index * bytes_per_row;
        let end = (start + bytes_per_row).min(record.bytes.len());
        let row_start_bit = start * 8;
        let row_end_bit = end * 8;
        let byte_cells = (start..end).map(|byte_index| {
            let byte = record.bytes[byte_index];
            let bits = (0..8).map(|bit_in_byte| {
                let bit = byte_index * 8 + bit_in_byte;
                let value = (byte >> bit_in_byte) & 1;
                let active =
                    selection_is_exact && bit >= selected.start_bit && bit < selected.end_bit;
                let hinted = !active
                    && hinted_range
                        .is_some_and(|(start_bit, end_bit)| bit >= start_bit && bit < end_bit);
                let role = record
                    .fields
                    .iter()
                    .filter(|field| {
                        field.exact_range && bit >= field.start_bit && bit < field.end_bit
                    })
                    .max_by_key(|field| field.depth)
                    .map_or(FieldRole::Payload, |field| field.role);
                div()
                    .id(("protocol-bit", bit))
                    .h(px(27.0))
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .border_color(if active {
                        rgb(0x003c_aee0)
                    } else if hinted {
                        rgb(0x002b_7ea8)
                    } else {
                        rgb(0x0018_394d)
                    })
                    .when(active, |cell| {
                        cell.border_y_1()
                            .when(bit == selected.start_bit.max(row_start_bit), |cell| {
                                cell.border_l_1()
                            })
                            .when(bit + 1 == selected.end_bit.min(row_end_bit), |cell| {
                                cell.border_r_1()
                            })
                    })
                    .when(!active, gpui::Styled::border_1)
                    .bg(if active {
                        rgb(0x0014_5b80)
                    } else if hinted {
                        rgb(0x000f_3f58)
                    } else {
                        match role {
                            FieldRole::Route => rgb(0x0010_293a),
                            FieldRole::Control => rgb(0x0015_1f3a),
                            FieldRole::Payload => rgb(0x000c_2424),
                            FieldRole::Padding => rgb(0x001b_1624),
                        }
                    })
                    .font_family("monospace")
                    .text_size(px(11.0))
                    .text_color(if active {
                        rgb(0x00d9_f2ff)
                    } else if hinted {
                        rgb(0x00a9_cde0)
                    } else {
                        rgb(0x0078_94a9)
                    })
                    .hover(|style| {
                        style
                            .border_color(rgb(0x0056_c9ff))
                            .text_color(rgb(0x00ff_ffff))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.select_bit(bit, cx)))
                    .child(value.to_string())
            });
            div()
                .w(px(BIT_BYTE_WIDTH))
                .flex_shrink_0()
                .child(
                    div()
                        .h(px(14.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .font_family("monospace")
                        .text_size(px(11.0))
                        .text_color(rgb(0x0053_6d82))
                        .child(format!("{byte_index:04x}")),
                )
                .child(div().flex().children(bits))
        });
        div()
            .h(px(BIT_ROW_HEIGHT))
            .w_full()
            .flex()
            .gap(px(BIT_LABEL_GAP))
            .child(
                div()
                    .w(px(BIT_LABEL_WIDTH))
                    .flex_shrink_0()
                    .pt(px(17.0))
                    .font_family("monospace")
                    .text_size(px(11.0))
                    .text_color(rgb(0x0053_6d82))
                    .child(format!("+{start:02x}")),
            )
            .child(div().flex().min_w(px(0.0)).children(byte_cells))
            .into_any_element()
    }

    /// one hex cell. the selected range is a solid wash; the hovered one is the
    /// same wash at a whisper, so a hover previews without pretending to be a
    /// selection.
    fn hex_cell(byte: u8, index: usize, selected: &Field, hinted: Option<(usize, usize)>) -> Div {
        let byte_start = index * 8;
        let overlap_start = selected.start_bit.max(byte_start);
        let overlap_end = selected.end_bit.min(byte_start + 8);
        let active = selected.exact_range && overlap_start < overlap_end;
        let hint = hinted.and_then(|(start_bit, end_bit)| {
            let hint_start = start_bit.max(byte_start);
            let hint_end = end_bit.min(byte_start + 8);
            (!active && hint_start < hint_end).then_some((hint_start, hint_end))
        });
        let cell = div()
            .relative()
            .w(px(23.0))
            .flex_shrink_0()
            .text_center()
            .when(active, |cell| cell.text_color(rgb(0x00d9_f2ff)))
            .when(hint.is_some(), |cell| cell.text_color(rgb(0x00a9_cde0)))
            .child(format!("{byte:02x}"));
        let (wash, span) = if active {
            (gpui::rgba(0x174e_6fa8), Some((overlap_start, overlap_end)))
        } else {
            (gpui::rgba(0x174e_6f4d), hint)
        };
        match span {
            Some((span_start, span_end)) => cell.child(
                div()
                    .absolute()
                    .left(px(23.0 * (span_start - byte_start) as f32 / 8.0))
                    .top_0()
                    .bottom_0()
                    .w(px(23.0 * (span_end - span_start) as f32 / 8.0))
                    .bg(wash),
            ),
            None => cell,
        }
    }

    fn byte_row(&self, row_index: usize) -> AnyElement {
        let record = self.selected_record();
        let selected = self.selected_field();
        let hinted_range = self.hinted_range();
        let start = row_index * 16;
        let end = (start + 16).min(record.bytes.len());
        let hex = (start..end)
            .map(|index| Self::hex_cell(record.bytes[index], index, selected, hinted_range));
        let ascii = (start..end).map(|index| {
            let byte = record.bytes[index];
            let active = selected.exact_range
                && index * 8 < selected.end_bit
                && index * 8 + 8 > selected.start_bit;
            let hinted = !active
                && hinted_range.is_some_and(|(start_bit, end_bit)| {
                    index * 8 < end_bit && index * 8 + 8 > start_bit
                });
            div()
                .w(px(7.0))
                .text_center()
                .when(active, |cell| {
                    cell.bg(gpui::rgba(0x174e_6f80))
                        .text_color(rgb(0x00d9_f2ff))
                })
                .when(hinted, |cell| {
                    cell.bg(gpui::rgba(0x174e_6f40))
                        .text_color(rgb(0x00a9_cde0))
                })
                .child(if byte.is_ascii_graphic() || byte == b' ' {
                    char::from(byte).to_string()
                } else {
                    "·".to_owned()
                })
        });
        div()
            .h(px(BYTE_ROW_HEIGHT))
            .min_w(px(590.0))
            .flex()
            .items_center()
            .font_family("monospace")
            .text_size(px(12.0))
            .text_color(rgb(0x0078_93a9))
            .child(
                div()
                    .w(px(42.0))
                    .flex_shrink_0()
                    .text_color(rgb(0x0053_6d82))
                    .child(format!("{start:04x}")),
            )
            .children(hex)
            .child(
                div()
                    .ml(px(10.0))
                    .pl(px(10.0))
                    .flex()
                    .border_l_1()
                    .border_color(rgb(0x0017_384d))
                    .text_color(rgb(0x009d_b5c7))
                    .children(ascii),
            )
            .into_any_element()
    }

    fn visible_field_indices(&self) -> Vec<usize> {
        let record = self.selected_record();
        let filter = self.field_filter_value.to_lowercase();
        if !filter.is_empty() {
            let mut included = HashSet::new();
            for (index, field) in record.fields.iter().enumerate() {
                if !format!("{} {} {}", field.path, field.kind, field.value)
                    .to_lowercase()
                    .contains(&filter)
                {
                    continue;
                }
                included.insert(index);
                let mut depth = field.depth;
                for parent in (0..index).rev() {
                    if depth == 0 {
                        break;
                    }
                    let candidate = &record.fields[parent];
                    if candidate.depth < depth {
                        included.insert(parent);
                        depth = candidate.depth;
                    }
                }
            }
            return (0..record.fields.len())
                .filter(|index| included.contains(index))
                .collect();
        }
        let mut hidden_below = None;
        let mut visible = Vec::new();
        for (index, field) in record.fields.iter().enumerate() {
            if hidden_below.is_some_and(|depth| field.depth > depth) {
                continue;
            }
            hidden_below = None;
            let container = record
                .fields
                .get(index + 1)
                .is_some_and(|next| next.depth > field.depth);
            let collapsed = container && self.collapsed_fields.contains(&field.path);
            if collapsed {
                hidden_below = Some(field.depth);
            }
            visible.push(index);
        }
        visible
    }

    /// the expand/collapse box. leaves keep the same footprint so names stay on
    /// one ruler, but only containers draw a target.
    fn field_toggle(
        &self,
        index: usize,
        container: bool,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let path = self.selected_record().fields[index].path.clone();
        div()
            .id(("protocol-field-toggle", index))
            .size(px(FIELD_TOGGLE_SIZE))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .when(container, |toggle| {
                toggle
                    .border_1()
                    .border_color(rgb(0x0024_5c78))
                    .bg(rgb(0x0009_1d2a))
                    .cursor_pointer()
                    .hover(|style| {
                        style
                            .border_color(rgb(0x0057_c8f5))
                            .bg(rgb(0x0012_3247))
                            .text_color(rgb(0x00e2_f7ff))
                    })
                    .active(|style| style.opacity(0.68))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.toggle_field(index, &path, cx);
                        this.focus_filter(FilterTarget::Fields, window, cx);
                    }))
                    .child(
                        div()
                            .relative()
                            .size(px(8.0))
                            .child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .right_0()
                                    .top(px(3.5))
                                    .h(px(1.0))
                                    .bg(rgb(0x0062_c7ee)),
                            )
                            .when(collapsed, |glyph| {
                                glyph.child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .bottom_0()
                                        .left(px(3.5))
                                        .w(px(1.0))
                                        .bg(rgb(0x0062_c7ee)),
                                )
                            }),
                    )
            })
    }

    fn field_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let record = self.selected_record();
        let field = &record.fields[index];
        let container = record
            .fields
            .get(index + 1)
            .is_some_and(|next| next.depth > field.depth);
        let collapsed = container && self.collapsed_fields.contains(&field.path);
        let label = field_label(&field.path);
        let range = if field.exact_range {
            format!("bits [{}, {})", field.start_bit, field.end_bit)
        } else {
            format!("within [{}, {})", field.start_bit, field.end_bit)
        };
        let active = index == self.selected_field;
        let traced = active || self.hovered_field == Some(index);
        let tooltip_title = field.path.clone();
        let tooltip_detail = field_help::tooltip_detail(field, container);
        let row = ui_inspector::selectable_row(("protocol-field", index), active)
            .h(px(FIELD_ROW_HEIGHT))
            .when(container && !active, |row| row.bg(rgb(0x0007_121b)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_field(index, cx);
                this.focus_filter(FilterTarget::Fields, window, cx);
            }))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                let next = if *hovered {
                    Some(index)
                } else if this.hovered_field == Some(index) {
                    None
                } else {
                    return;
                };
                if this.hovered_field != next {
                    this.hovered_field = next;
                    cx.notify();
                }
            }))
            .child(
                div()
                    .relative()
                    .h_full()
                    .pl(px(10.0 + field.depth as f32 * FIELD_INDENT))
                    .pr(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .when(field.depth > 0 && !active, |row| {
                        row.child(
                            div()
                                .absolute()
                                .left(px(10.0
                                    + FIELD_TOGGLE_SIZE / 2.0
                                    + (field.depth - 1) as f32 * FIELD_INDENT))
                                .top_0()
                                .bottom_0()
                                .w(px(1.0))
                                .bg(rgb(0x0010_2c3d)),
                        )
                    })
                    .child(self.field_toggle(index, container, collapsed, cx))
                    .child(
                        div()
                            .flex_shrink_0()
                            .max_w(px(190.0))
                            .truncate()
                            .font_family(if label.starts_with('[') {
                                "monospace"
                            } else {
                                FONT_INTERFACE
                            })
                            .text_size(px(12.5))
                            .font_weight(if container {
                                gpui::FontWeight::SEMIBOLD
                            } else {
                                gpui::FontWeight::NORMAL
                            })
                            .text_color(if active {
                                rgb(0x00f1_f8ff)
                            } else if container {
                                rgb(0x00cc_e2f2)
                            } else {
                                rgb(0x00ae_bfce)
                            })
                            .child(label),
                    )
                    .child(
                        div()
                            .min_w(px(0.0))
                            .flex_1()
                            .truncate()
                            .font_family("monospace")
                            .text_size(px(12.5))
                            .text_color(field_value_color(&field.value, active))
                            .child(field.value.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .font_family("monospace")
                            .text_size(px(10.0))
                            .text_color(if active {
                                rgb(0x007f_9cb0)
                            } else {
                                rgb(0x0056_718a)
                            })
                            .child(field.kind),
                    )
                    .child(
                        div()
                            .w(px(BIT_RANGE_GUTTER))
                            .flex_shrink_0()
                            .text_right()
                            .truncate()
                            .font_family("monospace")
                            .text_size(px(10.0))
                            .text_color(if traced {
                                rgb(0x00a9_b8cc)
                            } else {
                                rgb(0x004c_657a)
                            })
                            .child(range),
                    ),
            );
        with_inspector_tooltip(row, tooltip_title, tooltip_detail, self.ui_assets.clone())
            .into_any_element()
    }

    fn fields_pane(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let record = self.selected_record();
        let visible = self.visible_field_indices();
        let visible_count = visible.len();
        let now = Instant::now();
        let animating = self
            .field_transition
            .as_ref()
            .is_some_and(|transition| transition.is_running(now));
        let rows = animating.then(|| {
            ui_roster::animated_rows_with_height(
                visible.clone(),
                self.field_transition.as_ref(),
                now,
                |index| u32::try_from(*index).unwrap_or(u32::MAX),
                FIELD_ROW_HEIGHT,
                0.0,
                |index, _| self.field_row(*index, cx),
            )
        });
        let field_scroll = self.field_scroll.0.borrow().base_handle.clone();
        let list = uniform_list(
            "protocol-field-scroll",
            visible_count,
            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                let visible = this.visible_field_indices();
                range
                    .filter_map(|position| visible.get(position).copied())
                    .map(|index| this.field_row(index, cx))
                    .collect::<Vec<_>>()
            }),
        )
        .size_full()
        .track_scroll(&self.field_scroll);
        ui_inspector::pane()
            .id("protocol-field-pane")
            .relative()
            .w(px(self.field_pane_width))
            .flex_shrink_0()
            .border_l_1()
            .border_color(rgb(0x0017_384d))
            .child(
                ui_inspector::header_shell()
                    .id("protocol-field-filter-focus")
                    .cursor(gpui::CursorStyle::IBeam)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.focus_filter(FilterTarget::Fields, window, cx);
                        }),
                    )
                    .child(ui_inspector::header_title("Decoded"))
                    .child(direction_badge(record.direction))
                    .child(
                        ui_inspector::header_detail()
                            .font_family("monospace")
                            .text_size(px(12.0))
                            .text_color(rgb(0x008f_a8bb))
                            .child(if self.field_filter_value.is_empty() {
                                record.type_name.clone()
                            } else {
                                format!("{visible_count} / {}", record.fields.len())
                            }),
                    ),
            )
            .child(self.search_row(FilterTarget::Fields, window, cx))
            .child(
                div()
                    .id("protocol-field-viewport")
                    .flex_1()
                    .min_h(px(0.0))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.focus_filter(FilterTarget::Fields, window, cx);
                        }),
                    )
                    .when_some(rows, |layer, rows| {
                        layer
                            .overflow_y_scroll()
                            .track_scroll(&field_scroll)
                            .children(rows)
                    })
                    .when(!animating, |layer| layer.child(list))
                    .vertical_scrollbar_for(&field_scroll, window, cx),
            )
            .when(
                self.pointer_filter_target == Some(FilterTarget::Fields),
                |pane| pane.child(ui_inspector::focus_outline()),
            )
    }
}

impl Focusable for ProtocolViewer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Drop for ProtocolViewer {
    fn drop(&mut self) {
        if self.paused {
            set_capture_paused(false);
        }
    }
}

#[cfg(test)]
mod filter_tests {
    use std::collections::HashSet;

    use super::{Direction, record_matches, service_counts};
    use superiority_core::native::inspect::Record;

    fn record(service: &str, command: &str, direction: Direction) -> Record {
        Record {
            sequence: 0,
            captured_at_millis: 0,
            direction,
            service: service.to_owned(),
            command: command.to_owned(),
            type_name: format!("{service}::{command}"),
            service_slot: 0,
            command_id: 0,
            bytes: Vec::new(),
            logical_bits: 0,
            fields: Vec::new(),
        }
    }

    fn services(names: &[&str]) -> HashSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn chips_and_search_narrow_together() {
        let chat = record("Chat", "MembershipChangeNotify", Direction::Incoming);
        let presence = record("Presence", "UpdateNotify", Direction::Incoming);
        let selected = services(&["Chat"]);
        assert!(record_matches(&chat, "", &selected));
        assert!(!record_matches(&presence, "", &selected));
        assert!(record_matches(&chat, "membership", &selected));
        assert!(!record_matches(&chat, "presence", &selected));
    }

    #[test]
    fn no_selected_chips_means_every_service() {
        let presence = record("Presence", "UpdateNotify", Direction::Incoming);
        assert!(record_matches(&presence, "", &HashSet::new()));
        assert!(record_matches(&presence, "presence", &HashSet::new()));
    }

    #[test]
    fn search_still_reaches_type_name_and_direction() {
        let profile = record("Profile", "ReadRequest", Direction::Outgoing);
        assert!(record_matches(&profile, "profile::read", &HashSet::new()));
        assert!(record_matches(&profile, "outgoing", &HashSet::new()));
        assert!(!record_matches(&profile, "incoming", &HashSet::new()));
    }

    #[test]
    fn chips_order_by_traffic_then_name() {
        let records = [
            record("Presence", "UpdateNotify", Direction::Incoming),
            record("Chat", "MembershipChangeNotify", Direction::Incoming),
            record("Presence", "UpdateNotify", Direction::Incoming),
            record("Authentication", "LogonRequest", Direction::Outgoing),
        ];
        assert_eq!(
            service_counts(&records),
            vec![
                ("Presence".to_owned(), 2),
                ("Authentication".to_owned(), 1),
                ("Chat".to_owned(), 1),
            ]
        );
    }
}

#[cfg(test)]
mod focus_tests {
    use super::{FilterTarget, boundary_position, filter_target_at, moved_position};

    #[test]
    fn filter_focus_uses_full_column_bounds() {
        assert_eq!(
            filter_target_at(100.0, 700.0, 1_360.0, 300.0, 500.0, true),
            Some(FilterTarget::Records)
        );
        assert_eq!(
            filter_target_at(1_000.0, 700.0, 1_360.0, 300.0, 500.0, true),
            Some(FilterTarget::Fields)
        );
        assert_eq!(
            filter_target_at(500.0, 700.0, 1_360.0, 300.0, 500.0, true),
            None
        );
    }

    #[test]
    fn filter_focus_excludes_chrome_and_absent_fields() {
        assert_eq!(
            filter_target_at(100.0, 20.0, 1_360.0, 300.0, 500.0, true),
            None
        );
        assert_eq!(
            filter_target_at(1_000.0, 700.0, 1_360.0, 300.0, 500.0, false),
            None
        );
    }

    #[test]
    fn navigation_moves_through_visible_items_only() {
        let visible = [1, 4, 9];
        assert_eq!(moved_position(&visible, 4, -1), Some(0));
        assert_eq!(moved_position(&visible, 4, 1), Some(2));
        assert_eq!(moved_position(&visible, 1, -1), Some(0));
        assert_eq!(moved_position(&visible, 9, 1), Some(2));
        assert_eq!(moved_position(&visible, 7, 1), Some(0));
        assert_eq!(moved_position(&visible, 7, -1), Some(2));
        assert_eq!(moved_position(&[], 0, 1), None);
    }

    #[test]
    fn navigation_finds_list_boundaries() {
        assert_eq!(boundary_position(3, false), Some(0));
        assert_eq!(boundary_position(3, true), Some(2));
        assert_eq!(boundary_position(0, false), None);
    }
}

impl gpui::Render for ProtocolViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        platform::configure_window(window);
        self.refresh_capture(cx);
        let now = Instant::now();
        if self
            .record_transition
            .as_ref()
            .is_some_and(|transition| transition.is_running(now))
            || self
                .field_transition
                .as_ref()
                .is_some_and(|transition| transition.is_running(now))
        {
            window.request_animation_frame();
        }
        if self
            .record_transition
            .as_ref()
            .is_some_and(|transition| !transition.is_running(now))
        {
            self.record_transition = None;
        }
        if self
            .field_transition
            .as_ref()
            .is_some_and(|transition| !transition.is_running(now))
        {
            self.field_transition = None;
        }
        ui_workspace::root()
            .id("protocol-viewer")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_move(cx.listener(|this, event, window, cx| {
                this.update_resize(event, window, cx);
                this.update_vertical_resize(event, window, cx);
                this.update_filter_focus(event, window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_resize(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_resize(cx)),
            )
            .flex()
            .flex_col()
            .font_family(FONT_INTERFACE)
            .child(self.titlebar(window, cx))
            .child(div().h(px(6.0)).flex_shrink_0().bg(rgb(0x0004_0a10)))
            .child(if self.capture.records.is_empty() {
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .child(self.records_pane(window, cx))
                    .child(self.horizontal_splitter(true, cx))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(14.0))
                            .text_color(rgb(0x006b_8498))
                            .child(if self.paused {
                                "Capture paused"
                            } else {
                                "Waiting for native traffic"
                            }),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .child(self.records_pane(window, cx))
                    .child(self.horizontal_splitter(true, cx))
                    .child(self.stream_pane(window, cx))
                    .child(self.horizontal_splitter(false, cx))
                    .child(self.fields_pane(window, cx))
                    .into_any_element()
            })
    }
}
