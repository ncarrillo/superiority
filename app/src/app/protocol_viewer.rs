use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, Bounds, Context, FocusHandle, Focusable, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, Subscription, TitlebarOptions, UniformListScrollHandle, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgb, size, uniform_list,
};
use superiority_ui::{
    UiAssets,
    components::{
        controls as ui_controls, inspector as ui_inspector, navigation as ui_navigation,
        roster as ui_roster, text_input as ui_text_input, workspace as ui_workspace,
    },
    theme::{FONT_INTERFACE, FONT_INTERNATIONAL, TAB_LEADING_SPACE},
};

use crate::native::inspect::{
    Capture, Field, FieldRole, capture_paused, clear_capture, live_capture_after, sample_capture,
    set_capture_paused,
};

use super::client::{chrome::Assets, platform};

const WINDOW_WIDTH: f32 = 1360.0;
const WINDOW_HEIGHT: f32 = 820.0;
const RECORD_PANE_WIDTH: f32 = 300.0;
const FIELD_PANE_WIDTH: f32 = 500.0;
const RECORD_ROW_HEIGHT: f32 = 72.0;
const FIELD_ROW_HEIGHT: f32 = 50.0;
const BIT_ROW_HEIGHT: f32 = 47.0;
const BYTE_ROW_HEIGHT: f32 = 22.0;
const SPLITTER_SIZE: f32 = 7.0;
const BIT_BYTE_WIDTH: f32 = 64.0;
const BIT_LABEL_WIDTH: f32 = 34.0;
const BIT_LABEL_GAP: f32 = 5.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FilterTarget {
    Records,
    Fields,
}

#[derive(Clone, Copy, Debug)]
enum PaneResize {
    Records { pointer: f32, width: f32 },
    Fields { pointer: f32, width: f32 },
    Bitstream { pointer: f32, height: f32 },
}

struct InspectorTooltip {
    title: &'static str,
    detail: &'static str,
    assets: UiAssets,
}

impl gpui::Render for InspectorTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let tooltip = ui_controls::tooltip_shell(330.0, 92.0, self.assets.tooltip_fill.clone())
            .font_family(FONT_INTERNATIONAL)
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .top(px(14.0))
                    .right(px(18.0))
                    .text_size(px(13.5))
                    .text_color(rgb(0xd6e0f0))
                    .child(self.title),
            )
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .top(px(42.0))
                    .right(px(18.0))
                    .text_size(px(11.5))
                    .line_height(px(15.0))
                    .text_color(rgb(0x85d1ff))
                    .child(self.detail),
            );
        ui_controls::animated_tooltip(tooltip, "protocol-tooltip-open", 0.0, 0.0, -8.0)
    }
}

fn with_inspector_tooltip(
    element: impl StatefulInteractiveElement + IntoElement,
    title: &'static str,
    detail: &'static str,
    assets: UiAssets,
) -> impl IntoElement {
    element.tooltip(move |_, cx| {
        cx.new(|_| InspectorTooltip {
            title,
            detail,
            assets: assets.clone(),
        })
        .into()
    })
}

fn field_label(path: &str) -> String {
    let leaf = path.rsplit('.').next().unwrap_or(path);
    if let Some((_, index)) = leaf.rsplit_once('[')
        && leaf.ends_with(']')
        && leaf[..leaf.len() - index.len() - 1]
            .chars()
            .all(|character| character != ']')
    {
        return format!("[{}", index);
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
            is_movable: false,
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
    field_filter: ui_text_input::TextInput,
    field_filter_value: String,
    filter_target: FilterTarget,
    record_transition: Option<ui_roster::TimedTransition<(), usize, Instant>>,
    field_transition: Option<ui_roster::TimedTransition<(), usize, Instant>>,
    record_scroll: UniformListScrollHandle,
    bit_scroll: UniformListScrollHandle,
    byte_scroll: UniformListScrollHandle,
    field_scroll: UniformListScrollHandle,
    ui_assets: UiAssets,
    live: bool,
    following: bool,
    paused: bool,
    collapsed_fields: HashSet<String>,
    record_pane_width: f32,
    field_pane_width: f32,
    bit_pane_width: f32,
    bitstream_height: Option<f32>,
    pane_resize: Option<PaneResize>,
    _subscriptions: Vec<Subscription>,
}

impl ProtocolViewer {
    fn new(cx: &mut Context<Self>) -> Self {
        let record_filter = ui_text_input::TextInput::new("Filter service or command", cx);
        let record_subscription = record_filter.subscribe(cx, |this, cx| {
            this.set_record_filter(this.record_filter.content(), cx);
        });
        let field_filter = ui_text_input::TextInput::new("Filter properties", cx);
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
            field_filter,
            field_filter_value: String::new(),
            filter_target: FilterTarget::Records,
            record_transition: None,
            field_transition: None,
            record_scroll: UniformListScrollHandle::new(),
            bit_scroll: UniformListScrollHandle::new(),
            byte_scroll: UniformListScrollHandle::new(),
            field_scroll: UniformListScrollHandle::new(),
            ui_assets: UiAssets::native(),
            live: has_live_records,
            following: true,
            paused: capture_paused(),
            collapsed_fields: HashSet::new(),
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
                if entity
                    .update(cx, |this, cx| this.refresh_capture(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        this
    }

    fn selected_record(&self) -> &crate::native::inspect::Record {
        &self.capture.records[self.selected_record]
    }

    fn selected_field(&self) -> &Field {
        &self.selected_record().fields[self.selected_field]
    }

    fn select_record(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_record = index;
        self.selected_field = 0;
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
        }
    }

    fn set_record_filter(&mut self, next: String, cx: &mut Context<Self>) {
        if self.record_filter_value == next {
            return;
        }
        let previous = self.filtered_record_indices();
        self.record_filter_value = next;
        let filtered = self.filtered_record_indices();
        self.record_transition = ui_roster::Transition::new(previous, &filtered, |index| {
            u32::try_from(*index).unwrap_or(u32::MAX)
        })
        .map(|transition| ui_roster::TimedTransition {
            scope: (),
            transition,
            started: Instant::now(),
        });
        cx.notify();
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

    fn update_resize(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(resize) = self.pane_resize else {
            return;
        };
        let pointer = event.position.x.as_f32();
        match resize {
            PaneResize::Records {
                pointer: start,
                width,
            } => {
                self.record_pane_width = (width + pointer - start).clamp(230.0, 480.0);
            }
            PaneResize::Fields {
                pointer: start,
                width,
            } => {
                self.field_pane_width = (width + start - pointer).clamp(340.0, 760.0);
            }
            PaneResize::Bitstream { .. } => return,
        }
        cx.notify();
    }

    fn update_vertical_resize(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(PaneResize::Bitstream { pointer, height }) = self.pane_resize else {
            return;
        };
        self.bitstream_height =
            Some((height + event.position.y.as_f32() - pointer).clamp(160.0, 680.0));
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
            .bg(rgb(0x07131d))
            .hover(|style| style.bg(rgb(0x1e789e)))
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
        self.filter_target = target;
        match target {
            FilterTarget::Records => self.record_filter.focus(window, cx),
            FilterTarget::Fields => self.field_filter.focus(window, cx),
        }
        cx.notify();
    }

    fn top_bar(&self) -> impl IntoElement {
        let tabs = ui_navigation::ChannelTabs::new(
            vec![ui_navigation::ChannelTab::new("protocol-viewer-tab", "Capture").active(true)],
            self.ui_assets.clone(),
        )
        .leading(TAB_LEADING_SPACE);
        ui_navigation::bar(Some(self.ui_assets.top_navigation_background.clone())).child(tabs)
    }

    fn capture_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let record_count = self.capture.records.len();
        div()
            .h(px(38.0))
            .w_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(7.0))
            .px(px(9.0))
            .border_b_1()
            .border_color(rgb(0x102a3b))
            .bg(rgb(0x050d15))
            .child(
                div()
                    .id("protocol-record-filter-focus")
                    .mr_auto()
                    .h_full()
                    .flex()
                    .items_center()
                    .cursor(gpui::CursorStyle::IBeam)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.focus_filter(FilterTarget::Records, window, cx);
                    }))
                    .font_family("monospace")
                    .text_size(px(11.0))
                    .text_color(if self.record_filter_value.is_empty() {
                        rgb(0x587287)
                    } else {
                        rgb(0x55c8f5)
                    })
                    .child(if self.record_filter_value.is_empty() {
                        format!("{record_count} records")
                    } else {
                        format!(
                            "{} / {record_count}  ·  {}",
                            self.filtered_record_indices().len(),
                            self.record_filter_value
                        )
                    }),
            )
            .child(with_inspector_tooltip(
                ui_inspector::toolbar_button(
                    "protocol-capture-toggle",
                    if self.paused {
                        "▶  Resume"
                    } else {
                        "Ⅱ  Pause"
                    },
                    self.paused,
                )
                .h(px(26.0))
                .px(px(9.0))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_capture(cx))),
                if self.paused {
                    "Resume capture"
                } else {
                    "Pause capture"
                },
                if self.paused {
                    "Continue collecting native protocol records."
                } else {
                    "Freeze this record list without disconnecting the client."
                },
                self.ui_assets.clone(),
            ))
            .child(with_inspector_tooltip(
                ui_inspector::toolbar_button("protocol-capture-clear", "×  Clear", false)
                    .h(px(26.0))
                    .px(px(9.0))
                    .on_click(cx.listener(|this, _, _, cx| this.clear(cx))),
                "Clear capture",
                "Remove the records currently held by this viewer.",
                self.ui_assets.clone(),
            ))
    }

    fn filtered_record_indices(&self) -> Vec<usize> {
        let filter = self.record_filter_value.to_lowercase();
        self.capture
            .records
            .iter()
            .enumerate()
            .filter_map(|(index, record)| {
                (filter.is_empty()
                    || record.service.to_lowercase().contains(&filter)
                    || record.command.to_lowercase().contains(&filter)
                    || record.type_name.to_lowercase().contains(&filter))
                .then_some(index)
            })
            .collect()
    }

    fn record_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let record = &self.capture.records[index];
        let selected = index == self.selected_record;
        ui_inspector::selectable_row(("protocol-record", index), selected)
            .h(px(RECORD_ROW_HEIGHT))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_record(index, cx);
                this.focus_filter(FilterTarget::Records, window, cx);
            }))
            .child(
                div()
                    .absolute()
                    .left(px(10.0))
                    .top(px(11.0))
                    .text_size(px(11.0))
                    .font_family("monospace")
                    .text_color(rgb(0x536d82))
                    .child(format!("{:02}", index + 1)),
            )
            .child(
                div()
                    .pl(px(40.0))
                    .pt(px(8.0))
                    .pr(px(10.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0xd5e5f4))
                            .child(record.service.clone()),
                    )
                    .child(
                        div()
                            .mt(px(2.0))
                            .text_size(px(12.0))
                            .text_color(rgb(0x4db8e9))
                            .child(record.command.clone()),
                    )
                    .child(
                        div()
                            .mt(px(5.0))
                            .flex()
                            .gap(px(10.0))
                            .font_family("monospace")
                            .text_size(px(10.0))
                            .text_color(rgb(0x536f84))
                            .child(format!("{} B", record.bytes.len()))
                            .child(format!("{} bits", record.logical_bits)),
                    ),
            )
            .into_any_element()
    }

    fn records_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            .relative()
            .w(px(self.record_pane_width))
            .flex_shrink_0()
            .border_r_1()
            .border_color(rgb(0x17384d))
            .child(self.capture_controls(cx))
            .child(
                div()
                    .absolute()
                    .right(px(4.0))
                    .top(px(4.0))
                    .size(px(1.0))
                    .opacity(0.001)
                    .child(self.record_filter.element()),
            )
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
                    .when_some(rows, |layer, rows| layer.overflow_y_scroll().children(rows))
                    .when(!animating, |layer| layer.child(list)),
            )
    }

    fn stream_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let record = self.selected_record();
        let selected = self.selected_field();
        let selection_is_exact = selected.exact_range;
        let bytes_per_row = self.bytes_per_bit_row();
        let bit_row_count = record.bytes.len().div_ceil(bytes_per_row);
        let bitstream_height = self
            .bitstream_height
            .unwrap_or((bit_row_count as f32 * BIT_ROW_HEIGHT + 24.0).clamp(118.0, 460.0));
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
            .bg(rgb(0x030a10))
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
                    .h(px(bitstream_height))
                    .min_h(px(0.0))
                    .child(bit_rows),
            )
            .child(
                div()
                    .id("protocol-bitstream-splitter")
                    .h(px(SPLITTER_SIZE))
                    .w_full()
                    .flex_shrink_0()
                    .cursor(gpui::CursorStyle::ResizeUpDown)
                    .bg(rgb(0x07131d))
                    .hover(|style| style.bg(rgb(0x1e789e)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
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
                    .min_h(px(128.0))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .border_t_1()
                    .border_color(rgb(0x17384d))
                    .bg(rgb(0x050d14))
                    .child(
                        div()
                            .h(px(34.0))
                            .flex()
                            .items_center()
                            .px(px(13.0))
                            .border_b_1()
                            .border_color(rgb(0x102a3b))
                            .font_family(FONT_INTERFACE)
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_size(px(11.0))
                            .text_color(rgb(0x6b8498))
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
                                        rgb(0x48bff0)
                                    } else {
                                        rgb(0x6b8498)
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
                            .flex_1()
                            .min_h(px(0.0))
                            .px(px(12.0))
                            .py(px(7.0))
                            .child(byte_rows),
                    ),
            )
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
                    .border_color(if active { rgb(0x3caee0) } else { rgb(0x18394d) })
                    .when(active, |cell| {
                        cell.border_y_1()
                            .when(bit == selected.start_bit.max(row_start_bit), |cell| {
                                cell.border_l_1()
                            })
                            .when(bit + 1 == selected.end_bit.min(row_end_bit), |cell| {
                                cell.border_r_1()
                            })
                    })
                    .when(!active, |cell| cell.border_1())
                    .bg(if active {
                        rgb(0x12374d)
                    } else {
                        match role {
                            FieldRole::Route => rgb(0x10293a),
                            FieldRole::Control => rgb(0x151f3a),
                            FieldRole::Payload => rgb(0x0c2424),
                            FieldRole::Padding => rgb(0x1b1624),
                        }
                    })
                    .font_family("monospace")
                    .text_size(px(11.0))
                    .text_color(if active { rgb(0xd9f2ff) } else { rgb(0x7894a9) })
                    .hover(|style| style.border_color(rgb(0x56c9ff)).text_color(rgb(0xffffff)))
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
                        .text_size(px(10.0))
                        .text_color(rgb(0x536d82))
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
                    .text_size(px(10.0))
                    .text_color(rgb(0x536d82))
                    .child(format!("+{:02x}", start)),
            )
            .child(div().flex().min_w(px(0.0)).children(byte_cells))
            .into_any_element()
    }

    fn byte_row(&self, row_index: usize) -> AnyElement {
        let record = self.selected_record();
        let selected = self.selected_field();
        let start = row_index * 16;
        let end = (start + 16).min(record.bytes.len());
        let hex = (start..end).map(|index| {
            let byte = record.bytes[index];
            let bit = index * 8;
            let active =
                selected.exact_range && bit < selected.end_bit && bit + 8 > selected.start_bit;
            div()
                .w(px(23.0))
                .flex_shrink_0()
                .text_center()
                .when(active, |cell| {
                    cell.bg(rgb(0x12374d)).text_color(rgb(0xd9f2ff))
                })
                .child(format!("{byte:02x}"))
        });
        let ascii = record.bytes[start..end]
            .iter()
            .map(|byte| {
                if byte.is_ascii_graphic() || *byte == b' ' {
                    char::from(*byte)
                } else {
                    '·'
                }
            })
            .collect::<String>();
        div()
            .h(px(BYTE_ROW_HEIGHT))
            .min_w(px(590.0))
            .flex()
            .items_center()
            .font_family("monospace")
            .text_size(px(11.0))
            .text_color(rgb(0x7893a9))
            .child(
                div()
                    .w(px(42.0))
                    .flex_shrink_0()
                    .text_color(rgb(0x536d82))
                    .child(format!("{start:04x}")),
            )
            .children(hex)
            .child(
                div()
                    .ml(px(10.0))
                    .pl(px(10.0))
                    .border_l_1()
                    .border_color(rgb(0x17384d))
                    .text_color(rgb(0x9db5c7))
                    .child(ascii),
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

    fn field_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let record = self.selected_record();
        let field = &record.fields[index];
        let container = record
            .fields
            .get(index + 1)
            .is_some_and(|next| next.depth > field.depth);
        let collapsed = container && self.collapsed_fields.contains(&field.path);
        let label = field_label(&field.path);
        let metadata = if field.exact_range {
            format!(
                "{}  ·  bits [{}, {})",
                field.kind, field.start_bit, field.end_bit
            )
        } else {
            field.kind.to_owned()
        };
        let toggle_glyph = div()
            .relative()
            .size(px(8.0))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(3.5))
                    .h(px(1.0))
                    .bg(rgb(0x62c7ee)),
            )
            .when(collapsed, |glyph| {
                glyph.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(3.5))
                        .w(px(1.0))
                        .bg(rgb(0x62c7ee)),
                )
            });
        let active = index == self.selected_field;
        ui_inspector::selectable_row(("protocol-field", index), active)
            .h(px(FIELD_ROW_HEIGHT))
            .when(container, |row| row.bg(rgb(0x07121b)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_field(index, cx);
                this.focus_filter(FilterTarget::Fields, window, cx);
            }))
            .child(
                div()
                    .relative()
                    .h_full()
                    .pl(px(10.0 + field.depth as f32 * 17.0))
                    .pr(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .when(field.depth > 0, |row| {
                        row.child(
                            div()
                                .absolute()
                                .left(px(16.0 + (field.depth - 1) as f32 * 17.0))
                                .top_0()
                                .bottom_0()
                                .w(px(1.0))
                                .bg(rgb(0x102c3d)),
                        )
                    })
                    .child(
                        div()
                            .id(("protocol-field-toggle", index))
                            .size(px(16.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(if container {
                                rgb(0x245c78)
                            } else {
                                rgb(0x142b39)
                            })
                            .bg(if container {
                                rgb(0x091d2a)
                            } else {
                                rgb(0x06121a)
                            })
                            .when(container, |toggle| {
                                let path = field.path.clone();
                                toggle
                                    .cursor_pointer()
                                    .hover(|style| {
                                        style
                                            .border_color(rgb(0x57c8f5))
                                            .bg(rgb(0x123247))
                                            .text_color(rgb(0xe2f7ff))
                                    })
                                    .active(|style| style.opacity(0.68))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.toggle_field(index, &path, cx);
                                        this.focus_filter(FilterTarget::Fields, window, cx);
                                    }))
                            })
                            .when(container, |toggle| toggle.child(toggle_glyph)),
                    )
                    .child(
                        div()
                            .min_w(px(0.0))
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                div()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .font_family(if label.starts_with('[') {
                                        "monospace"
                                    } else {
                                        FONT_INTERFACE
                                    })
                                    .text_size(px(if container { 12.0 } else { 11.5 }))
                                    .font_weight(if container {
                                        gpui::FontWeight::SEMIBOLD
                                    } else {
                                        gpui::FontWeight::NORMAL
                                    })
                                    .text_color(if container {
                                        rgb(0xcce2f2)
                                    } else {
                                        rgb(0xaebfce)
                                    })
                                    .child(label),
                            )
                            .child(
                                div()
                                    .mt(px(2.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .font_family("monospace")
                                    .text_size(px(9.5))
                                    .text_color(rgb(0x526f84))
                                    .child(metadata),
                            ),
                    )
                    .child(
                        div()
                            .max_w(px(205.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_right()
                            .font_family("monospace")
                            .text_size(px(11.0))
                            .text_color(rgb(0x67ceff))
                            .child(field.value.clone()),
                    ),
            )
            .into_any_element()
    }

    fn fields_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            .relative()
            .w(px(self.field_pane_width))
            .flex_shrink_0()
            .border_l_1()
            .border_color(rgb(0x17384d))
            .child(
                ui_inspector::header(
                    if self.field_filter_value.is_empty() {
                        "Decoded payload".to_owned()
                    } else {
                        format!("Decoded payload  /  {}", self.field_filter_value)
                    },
                    if self.field_filter_value.is_empty() {
                        record.type_name.clone()
                    } else {
                        format!("{visible_count} / {}", record.fields.len())
                    },
                )
                .id("protocol-field-filter-focus")
                .cursor(gpui::CursorStyle::IBeam)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.focus_filter(FilterTarget::Fields, window, cx);
                    }),
                ),
            )
            .child(
                div()
                    .absolute()
                    .right(px(4.0))
                    .top(px(4.0))
                    .size(px(1.0))
                    .opacity(0.001)
                    .child(self.field_filter.element()),
            )
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
                    .when_some(rows, |layer, rows| layer.overflow_y_scroll().children(rows))
                    .when(!animating, |layer| layer.child(list)),
            )
    }

    fn status_bar(&self) -> impl IntoElement {
        div()
            .h(px(29.0))
            .w_full()
            .flex()
            .items_center()
            .gap(px(20.0))
            .px(px(12.0))
            .border_t_1()
            .border_color(rgb(0x143c55))
            .bg(rgb(0x03080d))
            .font_family("monospace")
            .text_size(px(10.0))
            .text_color(rgb(0x4f697d))
            .child("Bit order  LSB-first within each byte")
            .child("Boundary  schema-delimited")
            .child("Ranges  [start, end)")
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
            .on_mouse_move(cx.listener(|this, event, _, cx| {
                this.update_resize(event, cx);
                this.update_vertical_resize(event, cx);
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
            .child(self.top_bar())
            .child(div().h(px(8.0)).flex_shrink_0().bg(rgb(0x040a10)))
            .child(if self.capture.records.is_empty() {
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .child(self.records_pane(cx))
                    .child(self.horizontal_splitter(true, cx))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(14.0))
                            .text_color(rgb(0x6b8498))
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
                    .child(self.records_pane(cx))
                    .child(self.horizontal_splitter(true, cx))
                    .child(self.stream_pane(cx))
                    .child(self.horizontal_splitter(false, cx))
                    .child(self.fields_pane(cx))
                    .into_any_element()
            })
            .child(self.status_bar())
    }
}
