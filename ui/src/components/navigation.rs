use std::{collections::HashMap, hash::Hash, time::Duration};

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Hsla, ImageSource, IntoElement, MouseButton,
    MouseDownEvent, ObjectFit, Render, RenderOnce, ScrollHandle, StyledImage as _, TextRun, Window,
    div, font, img, prelude::*, px, rgb, rgba,
};

use crate::{
    UiAssets,
    animation::{
        AnimationClock, OffsetAnimation, ScalarAnimation, animation_progress, interpolated_offsets,
    },
    theme::{
        FONT_INTERFACE, FONT_NAVIGATION, TAB_BAR_HEIGHT, TAB_HEIGHT, TAB_NAME_INSET, TAB_NAME_LEAD,
        TAB_TOP,
    },
};

const TAB_DRAG_SLOP: f32 = 4.0;
const TAB_NAME_SPEED: f32 = 55.0;
const TAB_REORDER_DURATION: Duration = Duration::from_millis(160);

#[derive(Clone, Copy, Debug, PartialEq)]
struct TabNameLayout {
    pub uses_marquee: bool,
    pub viewport_width: f32,
    pub travel: f32,
}

struct TabDragState<C> {
    from: usize,
    to: usize,
    travelled: f32,
    widths: Vec<f32>,
    origins: Vec<f32>,
    shift: Option<OffsetAnimation<C>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabRelease {
    Click(usize),
    Reorder { from: usize, to: usize },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChannelTabTone {
    #[default]
    Standard,
    Party,
    Group,
}

#[derive(Clone, Copy)]
pub struct TabDragPayload {
    pub index: usize,
}

pub struct TabDragPreview;

impl Render for TabDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(1.0)).opacity(0.0)
    }
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type MouseDownHandler = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;
type HoverHandler = Box<dyn Fn(&TabHoverEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, Debug)]
pub struct TabHoverEvent {
    pub hovered: bool,
    pub travel: f32,
}

pub struct ChannelTab {
    id: String,
    label: String,
    unread: bool,
    active: bool,
    hovered: bool,
    marquee_offset: f32,
    drag_offset: f32,
    dragged_travel: Option<f32>,
    close_progress: Option<f32>,
    effect_opacity: [f32; 3],
    tone: ChannelTabTone,
    on_mouse_down: Option<MouseDownHandler>,
    on_click: Option<ClickHandler>,
    on_hover: Option<HoverHandler>,
    on_close: Option<ClickHandler>,
}

impl ChannelTab {
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            unread: false,
            active: false,
            hovered: false,
            marquee_offset: 0.0,
            drag_offset: 0.0,
            dragged_travel: None,
            close_progress: None,
            effect_opacity: [1.0; 3],
            tone: ChannelTabTone::Standard,
            on_mouse_down: None,
            on_click: None,
            on_hover: None,
            on_close: None,
        }
    }

    #[must_use]
    pub const fn unread(mut self, unread: bool) -> Self {
        self.unread = unread;
        self
    }

    #[must_use]
    pub const fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    #[must_use]
    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    #[must_use]
    pub const fn marquee_offset(mut self, offset: f32) -> Self {
        self.marquee_offset = offset;
        self
    }

    #[must_use]
    pub const fn drag_offset(mut self, offset: f32) -> Self {
        self.drag_offset = offset;
        self
    }

    #[must_use]
    pub const fn dragged_travel(mut self, travel: Option<f32>) -> Self {
        self.dragged_travel = travel;
        self
    }

    #[must_use]
    pub const fn close_progress(mut self, progress: Option<f32>) -> Self {
        self.close_progress = progress;
        self
    }

    #[must_use]
    pub const fn effect_opacity(mut self, opacity: [f32; 3]) -> Self {
        self.effect_opacity = opacity;
        self
    }

    #[must_use]
    pub const fn tone(mut self, tone: ChannelTabTone) -> Self {
        self.tone = tone;
        self
    }

    #[must_use]
    pub fn on_mouse_down(
        mut self,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_mouse_down = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_hover(
        mut self,
        handler: impl Fn(&TabHoverEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_hover = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_close(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn width(&self) -> f32 {
        tab_width(&self.label, self.unread)
    }
}

#[derive(IntoElement)]
pub struct ChannelTabs {
    items: Vec<ChannelTab>,
    assets: UiAssets,
    leading: f32,
    compact: bool,
    scroll: Option<ScrollHandle>,
    on_add: Option<ClickHandler>,
}

impl ChannelTabs {
    #[must_use]
    pub fn new(items: Vec<ChannelTab>, assets: UiAssets) -> Self {
        Self {
            items,
            assets,
            leading: 0.0,
            compact: false,
            scroll: None,
            on_add: None,
        }
    }

    #[must_use]
    pub const fn leading(mut self, leading: f32) -> Self {
        self.leading = leading;
        self
    }

    #[must_use]
    pub fn compact(mut self, scroll: &ScrollHandle) -> Self {
        self.compact = true;
        self.scroll = Some(scroll.clone());
        self
    }

    #[must_use]
    pub fn on_add(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_add = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn tail(&self) -> f32 {
        self.items.iter().map(ChannelTab::width).sum::<f32>()
            - self
                .items
                .iter()
                .filter_map(|item| item.close_progress.map(|progress| item.width() * progress))
                .sum::<f32>()
    }
}

impl RenderOnce for ChannelTabs {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let widths = self.items.iter().map(ChannelTab::width).collect::<Vec<_>>();
        let origins = tab_origins(&widths, 0.0);
        let tail = self.tail();
        let mut regular = Vec::<AnyElement>::with_capacity(self.items.len());
        let mut carried = None;
        let mut closed_before = 0.0;

        for (index, item) in self.items.into_iter().enumerate() {
            let slot_width = widths[index];
            let close_progress = item.close_progress.unwrap_or(0.0).clamp(0.0, 1.0);
            let mut x = origins[index] + item.drag_offset - closed_before;
            let mut width = slot_width;
            let mut opacity = 1.0;
            if close_progress > 0.0 {
                x += slot_width * close_progress / 2.0;
                width *= 1.0 - close_progress;
                opacity = 1.0 - close_progress;
                closed_before += slot_width * close_progress;
            }
            if let Some(travel) = item.dragged_travel {
                x = origins[index] + travel;
            }

            let label = if item.unread {
                format!("{}  •", item.label.to_uppercase())
            } else {
                item.label.to_uppercase()
            };
            let tint = tab_text_tint(item.tone, item.active, item.hovered, item.unread);
            let measured = measure_tab_title(&label, window);
            let travel = tab_name_layout(measured, slot_width).travel;
            let mut tab = tab_slot(self.leading + x, width, opacity)
                .id(item.id)
                .cursor_pointer()
                .on_drag(TabDragPayload { index }, |_: &TabDragPayload, _, _, cx| {
                    cx.new(|_| TabDragPreview)
                });
            if let Some(handler) = item.on_mouse_down {
                tab = tab.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    handler(event, window, cx);
                });
            }
            if let Some(handler) = item.on_click {
                tab = tab.on_click(move |event, window, cx| handler(event, window, cx));
            }
            if let Some(handler) = item.on_hover {
                tab = tab.on_hover(move |hovered, window, cx| {
                    handler(
                        &TabHoverEvent {
                            hovered: *hovered,
                            travel,
                        },
                        window,
                        cx,
                    );
                });
            }
            tab = tab.child(TabVisual::new(
                label,
                measured,
                item.marquee_offset,
                tint,
                TabChrome {
                    width: slot_width,
                    active: item.active,
                    divider: index > 0,
                    effect_opacity: item.effect_opacity,
                    tone: item.tone,
                },
                self.assets.clone(),
            ));
            if let Some(handler) = item.on_close {
                tab = tab.child(
                    div()
                        .id(("close-channel-tab", index))
                        .absolute()
                        .right(px(6.0))
                        .top(px(5.0))
                        .size(px(26.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.0))
                        .font_family(FONT_INTERFACE)
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_size(px(15.0))
                        .text_color(tint)
                        .cursor_pointer()
                        .hover(|style| style.bg(rgba(0x315d8748)).text_color(rgb(0xffffff)))
                        .active(|style| style.bg(rgba(0x315d8790)).opacity(0.72))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(move |event, window, cx| {
                            cx.stop_propagation();
                            handler(event, window, cx);
                        })
                        .child("×"),
                );
            }
            if item.dragged_travel.is_some() {
                carried = Some(tab.into_any_element());
            } else {
                regular.push(tab.into_any_element());
            }
        }

        let mut strip = tab_strip().children(regular).children(carried);
        if let Some(on_add) = self.on_add {
            strip = strip
                .child(
                    img(self.assets.top_navigation_divider.clone())
                        .absolute()
                        .left(px(self.leading + tail))
                        .top(px(TAB_TOP + 2.0))
                        .w(px(1.0))
                        .h(px(32.0))
                        .object_fit(ObjectFit::Fill),
                )
                .child(
                    div()
                        .id("add-channel")
                        .absolute()
                        .left(px(self.leading + tail + 1.0))
                        .top(px(TAB_TOP))
                        .h(px(TAB_HEIGHT))
                        .w(px(46.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .font_family(FONT_INTERFACE)
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_size(px(16.0))
                        .text_color(rgb(0x7d8fa8))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgba(0x153e6640)).text_color(rgb(0xd6e0f0)))
                        .active(|style| style.bg(rgba(0x245d8b78)).text_color(rgb(0xffffff)))
                        .on_click(move |event, window, cx| {
                            cx.stop_propagation();
                            on_add(event, window, cx);
                        })
                        .child("+"),
                );
        }

        if self.compact {
            let content_width = self.leading + tail + 8.0;
            tab_strip()
                .id("channel-tabs-scroll")
                .overflow_x_scroll()
                .track_scroll(&self.scroll.expect("compact tabs require a scroll handle"))
                .child(
                    div()
                        .relative()
                        .h_full()
                        .w(px(
                            content_width.max(f32::from(window.viewport_size().width))
                        ))
                        .child(strip),
                )
                .into_any_element()
        } else {
            strip.into_any_element()
        }
    }
}

#[derive(Clone, Copy)]
struct TabPointerState {
    index: usize,
    start_x: f32,
}

pub struct TabStripState<K, C> {
    pointer: Option<TabPointerState>,
    drag: Option<TabDragState<C>>,
    names: HashMap<K, ScalarAnimation<C>>,
}

impl<K, C> Default for TabStripState<K, C> {
    fn default() -> Self {
        Self {
            pointer: None,
            drag: None,
            names: HashMap::new(),
        }
    }
}

impl<K: Eq + Hash, C: AnimationClock> TabStripState<K, C> {
    pub fn begin_pointer(&mut self, index: usize, start_x: f32, item_count: usize) -> bool {
        if index >= item_count {
            return false;
        }
        self.pointer = Some(TabPointerState { index, start_x });
        true
    }

    pub fn cancel_pointer(&mut self) {
        self.pointer = None;
        self.drag = None;
    }

    pub fn clear(&mut self) {
        self.cancel_pointer();
        self.names.clear();
    }

    pub fn remove_name(&mut self, key: &K) {
        self.names.remove(key);
    }

    pub fn update_drag(&mut self, index: usize, current_x: f32, widths: &[f32], now: C) -> bool {
        let Some(pointer) = self.pointer else {
            return false;
        };
        if pointer.index != index || index >= widths.len() {
            return false;
        }
        let travelled = current_x - pointer.start_x;
        if self.drag.is_none() {
            if travelled.abs() < TAB_DRAG_SLOP || widths.len() < 2 {
                return false;
            }
            self.drag = Some(TabDragState {
                from: index,
                to: index,
                travelled,
                widths: widths.to_vec(),
                origins: tab_origins(widths, 0.0),
                shift: None,
            });
        }

        let Some(drag) = self.drag.as_mut() else {
            return false;
        };
        drag.travelled = travelled;
        let middle = drag.origins[drag.from] + travelled + drag.widths[drag.from] / 2.0;
        let landing = tab_landing_index(&drag.widths, 0.0, drag.from, middle);
        if landing != drag.to {
            let from = drag.offsets(now);
            drag.to = landing;
            let positions = reordered_tab_origins(&drag.widths, 0.0, drag.from, landing);
            let to = positions
                .iter()
                .zip(&drag.origins)
                .map(|(position, origin)| position - origin)
                .collect();
            drag.shift = Some(OffsetAnimation {
                from,
                to,
                started: now,
            });
        }
        true
    }

    #[must_use]
    pub fn finish(&mut self, fallback_index: usize) -> TabRelease {
        self.pointer = None;
        let Some(drag) = self.drag.take() else {
            return TabRelease::Click(fallback_index);
        };
        TabRelease::Reorder {
            from: drag.from,
            to: drag.to,
        }
    }

    #[must_use]
    pub fn offsets(&self, now: C, count: usize) -> Vec<f32> {
        self.drag
            .as_ref()
            .map_or_else(|| vec![0.0; count], |drag| drag.offsets(now))
    }

    #[must_use]
    pub fn is_dragging(&self, index: usize) -> bool {
        self.drag.as_ref().is_some_and(|drag| drag.from == index)
    }

    #[must_use]
    pub fn dragged_travel(&self) -> f32 {
        self.drag.as_ref().map_or(0.0, |drag| drag.travelled)
    }

    #[must_use]
    pub fn shift_is_running(&self, now: C) -> bool {
        self.drag
            .as_ref()
            .is_some_and(|drag| drag.shift_is_running(now))
    }

    pub fn set_name_hover(&mut self, key: K, hovered: bool, travel: f32, now: C) -> bool {
        if travel <= 0.0 {
            return false;
        }
        let current = self
            .names
            .get(&key)
            .map_or(0.0, |animation| animation.value(now));
        let target = if hovered { -travel } else { 0.0 };
        let distance = (target - current).abs();
        if distance <= f32::EPSILON {
            return false;
        }
        self.names.insert(
            key,
            ScalarAnimation {
                from: current,
                to: target,
                started: now,
                duration: marquee_duration(distance),
            },
        );
        true
    }

    #[must_use]
    pub fn name_offset(&self, key: &K, now: C) -> f32 {
        self.names
            .get(key)
            .map_or(0.0, |animation| animation.value(now))
    }

    pub fn retain_name_animations(&mut self, now: C) {
        self.names
            .retain(|_, animation| animation.is_running(now) || animation.to.abs() > f32::EPSILON);
    }

    #[must_use]
    pub fn name_animation_is_running(&self, now: C) -> bool {
        self.names
            .values()
            .any(|animation| animation.is_running(now))
    }
}

impl<C: AnimationClock> TabDragState<C> {
    #[must_use]
    pub fn offsets(&self, now: C) -> Vec<f32> {
        let Some(shift) = &self.shift else {
            return vec![0.0; self.widths.len()];
        };
        let progress = animation_progress(now.elapsed(shift.started), TAB_REORDER_DURATION);
        interpolated_offsets(&shift.from, &shift.to, progress)
    }

    #[must_use]
    pub fn shift_is_running(&self, now: C) -> bool {
        self.shift
            .as_ref()
            .is_some_and(|shift| now.elapsed(shift.started) < TAB_REORDER_DURATION)
    }
}

#[derive(Clone, Copy)]
struct TabChrome {
    pub width: f32,
    pub active: bool,
    pub divider: bool,
    pub effect_opacity: [f32; 3],
    pub tone: ChannelTabTone,
}

#[derive(IntoElement)]
struct TabVisual {
    label: String,
    measured: f32,
    marquee_offset: f32,
    tint: Hsla,
    chrome: TabChrome,
    assets: UiAssets,
}

impl TabVisual {
    #[must_use]
    fn new(
        label: String,
        measured: f32,
        marquee_offset: f32,
        tint: impl Into<Hsla>,
        chrome: TabChrome,
        assets: UiAssets,
    ) -> Self {
        Self {
            label,
            measured,
            marquee_offset,
            tint: tint.into(),
            chrome,
            assets,
        }
    }
}

impl RenderOnce for TabVisual {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .child(tab_chrome(self.chrome, &self.assets))
            .child(tab_label(
                self.label,
                self.measured,
                self.chrome.width,
                self.marquee_offset,
                self.tint,
            ))
    }
}

#[must_use]
pub fn tab_width(title: &str, unread: bool) -> f32 {
    let characters = title.to_uppercase().chars().count() + usize::from(unread) * 3;
    let characters = u16::try_from(characters).unwrap_or(u16::MAX);
    f32::from(characters).mul_add(8.1, 98.0).clamp(182.0, 258.0)
}

#[must_use]
fn measure_tab_title(title: &str, window: &Window) -> f32 {
    let run = TextRun {
        len: title.len(),
        font: font(FONT_NAVIGATION),
        color: rgb(0xffffff).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    f32::from(
        window
            .text_system()
            .layout_line(title, px(12.5), &[run], None)
            .width,
    )
}

#[must_use]
fn tab_chrome(spec: TabChrome, assets: &UiAssets) -> Div {
    let mut chrome = div().absolute().inset_0().overflow_hidden();
    if spec.active {
        let (selected, line, glow) = match spec.tone {
            ChannelTabTone::Group => (
                assets.top_navigation_selected_orange.clone(),
                assets.top_navigation_selected_line_orange.clone(),
                assets.top_navigation_selected_glow_orange.clone(),
            ),
            ChannelTabTone::Party => (
                assets.top_navigation_selected_pink.clone(),
                assets.top_navigation_selected_line_pink.clone(),
                assets.top_navigation_selected_glow_pink.clone(),
            ),
            ChannelTabTone::Standard => (
                assets.top_navigation_selected.clone(),
                assets.top_navigation_selected_line.clone(),
                assets.top_navigation_selected_glow.clone(),
            ),
        };
        chrome = chrome
            .child(
                img(selected)
                    .absolute()
                    .top(px(1.0))
                    .left(px(5.0))
                    .w(px((spec.width - 10.0).max(0.0)))
                    .h(px(35.0))
                    .opacity(spec.effect_opacity[0])
                    .object_fit(ObjectFit::Fill),
            )
            .child(
                img(line)
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .h(px(5.0))
                    .opacity(spec.effect_opacity[1])
                    .object_fit(ObjectFit::Fill),
            )
            .child(
                img(glow)
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .h(px(5.0))
                    .opacity(spec.effect_opacity[2])
                    .object_fit(ObjectFit::Fill),
            );
    }
    if spec.divider {
        chrome = chrome.child(
            img(assets.top_navigation_divider.clone())
                .absolute()
                .top(px(2.0))
                .left_0()
                .w(px(1.0))
                .h(px(32.0))
                .object_fit(ObjectFit::Fill),
        );
    }
    chrome
}

fn tab_text_tint(tone: ChannelTabTone, active: bool, hovered: bool, unread: bool) -> Hsla {
    if unread && !active {
        return match tone {
            ChannelTabTone::Party => rgb(0xf092c4).into(),
            ChannelTabTone::Group => rgb(0xf0aa64).into(),
            ChannelTabTone::Standard => rgb(0x6bc2f2).into(),
        };
    }
    if active {
        return match tone {
            ChannelTabTone::Party => rgb(0xffe8f6).into(),
            ChannelTabTone::Group => rgb(0xffedd7).into(),
            ChannelTabTone::Standard => rgb(0xe6f9ff).into(),
        };
    }
    if hovered {
        return match tone {
            ChannelTabTone::Party => rgb(0xb9789d).into(),
            ChannelTabTone::Group => rgb(0xb78358).into(),
            ChannelTabTone::Standard => rgb(0x7394b4).into(),
        };
    }
    match tone {
        ChannelTabTone::Party => rgb(0x76516a).into(),
        ChannelTabTone::Group => rgb(0x745b45).into(),
        ChannelTabTone::Standard => rgb(0x415d7d).into(),
    }
}

#[must_use]
fn centered_tab_label(label: String, width: f32, tint: Hsla) -> Div {
    div()
        .absolute()
        .left(px(TAB_NAME_INSET))
        .top_0()
        .w(px((width - TAB_NAME_INSET * 2.0).max(1.0)))
        .h(px(TAB_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .whitespace_nowrap()
        .font_family(FONT_NAVIGATION)
        .text_size(px(12.5))
        .text_color(tint)
        .child(label)
}

#[must_use]
fn tab_name_layout(measured: f32, slot_width: f32) -> TabNameLayout {
    let centered_width = (slot_width - TAB_NAME_INSET * 2.0).max(1.0);
    let viewport_width = (slot_width - TAB_NAME_LEAD - TAB_NAME_INSET).max(1.0);
    let uses_marquee = measured > centered_width;
    let travel = uses_marquee
        .then(|| (measured - viewport_width).max(0.0))
        .unwrap_or(0.0);
    TabNameLayout {
        uses_marquee,
        viewport_width,
        travel,
    }
}

#[must_use]
fn tab_label(
    label: String,
    measured: f32,
    slot_width: f32,
    marquee_offset: f32,
    tint: impl Into<Hsla>,
) -> Div {
    let tint = tint.into();
    let layout = tab_name_layout(measured, slot_width);
    if !layout.uses_marquee {
        return centered_tab_label(label, slot_width, tint);
    }
    let label_left = if layout.travel > 0.0 {
        marquee_offset
    } else {
        ((layout.viewport_width - measured) / 2.0).max(0.0)
    };
    div()
        .absolute()
        .top_0()
        .left(px(TAB_NAME_LEAD))
        .w(px(layout.viewport_width))
        .h(px(TAB_HEIGHT))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left(px(label_left))
                .top_0()
                .w(px(measured + 24.0))
                .h(px(TAB_HEIGHT))
                .flex()
                .items_center()
                .whitespace_nowrap()
                .font_family(FONT_NAVIGATION)
                .text_size(px(12.5))
                .text_color(tint)
                .child(label),
        )
}

#[must_use]
fn marquee_duration(distance: f32) -> Duration {
    Duration::from_secs_f32((distance.abs() / TAB_NAME_SPEED).max(0.12))
}

#[must_use]
fn tab_slot(left: f32, width: f32, opacity: f32) -> Div {
    div()
        .absolute()
        .left(px(left))
        .top(px(crate::theme::TAB_TOP))
        .w(px(width))
        .h(px(TAB_HEIGHT))
        .opacity(opacity)
        .overflow_hidden()
}

#[must_use]
fn tab_strip() -> Div {
    div().absolute().inset_0().overflow_hidden()
}

#[must_use]
fn tab_origins(widths: &[f32], start: f32) -> Vec<f32> {
    let mut x = start;
    widths
        .iter()
        .map(|width| {
            let origin = x;
            x += width;
            origin
        })
        .collect()
}

#[must_use]
pub fn reordered_tab_origins(widths: &[f32], start: f32, from: usize, to: usize) -> Vec<f32> {
    let mut order: Vec<usize> = (0..widths.len()).collect();
    let carried = order.remove(from);
    order.insert(to.min(order.len()), carried);
    let mut origins = vec![0.0; widths.len()];
    let mut x = start;
    for index in order {
        origins[index] = x;
        x += widths[index];
    }
    origins
}

#[must_use]
pub fn tab_landing_index(widths: &[f32], start: f32, from: usize, middle: f32) -> usize {
    let mut x = start;
    for (index, width) in widths.iter().enumerate() {
        if index == from {
            continue;
        }
        if middle < x + width / 2.0 {
            return if index > from { index - 1 } else { index };
        }
        x += width;
    }
    widths.len().saturating_sub(1)
}

#[must_use]
pub fn bar(background: Option<ImageSource>) -> Div {
    div()
        .relative()
        .h(px(TAB_BAR_HEIGHT))
        .flex_shrink_0()
        .overflow_hidden()
        .bg(rgb(0x0008_101a))
        .children(
            background.map(|source| img(source).absolute().inset_0().object_fit(ObjectFit::Fill)),
        )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        animation::{animation_progress, interpolated_offsets},
        theme::TAB_NAME_INSET,
    };

    use super::{
        TabRelease, TabStripState, marquee_duration, reordered_tab_origins, tab_landing_index,
        tab_name_layout, tab_origins, tab_width,
    };

    #[test]
    fn tab_width_matches_the_native_contract() {
        assert!((tab_width("General", false) - 182.0).abs() < f32::EPSILON);
        assert!((tab_width(&"x".repeat(40), false) - 258.0).abs() < f32::EPSILON);
        let expected = 12.0_f32.mul_add(8.1, 98.0);
        assert!((tab_width("aaaaaaaaaaß", false) - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn unread_marker_reserves_label_space() {
        let title = "abcdefghijkl";
        assert!(tab_width(title, true) > tab_width(title, false));
    }

    #[test]
    fn tab_reordering_uses_the_dragged_tab_midpoint() {
        let widths = [100.0, 120.0, 80.0];
        assert_eq!(tab_origins(&widths, 10.0), vec![10.0, 110.0, 230.0]);
        assert_eq!(
            reordered_tab_origins(&widths, 10.0, 0, 2),
            vec![210.0, 10.0, 130.0]
        );
        assert_eq!(tab_landing_index(&widths, 10.0, 0, 240.0), 2);
    }

    #[test]
    fn marquee_layout_uses_the_shared_clip_window() {
        let layout = tab_name_layout(170.0, 182.0);
        assert!(layout.uses_marquee);
        assert!(layout.viewport_width > 182.0 - TAB_NAME_INSET * 2.0);
        assert_eq!(layout.travel, 40.0);
        assert_eq!(marquee_duration(55.0), Duration::from_secs(1));
    }

    #[test]
    fn reorder_offsets_share_the_same_easing_output() {
        let progress = animation_progress(Duration::from_millis(80), Duration::from_millis(160));
        assert_eq!(
            interpolated_offsets(&[0.0, 10.0], &[20.0, -10.0], progress),
            vec![10.0, 0.0]
        );
    }

    #[test]
    fn shared_tab_state_distinguishes_clicks_and_reorders() {
        let mut state = TabStripState::<String, f64>::default();
        assert!(state.begin_pointer(1, 150.0, 3));
        assert_eq!(state.finish(1), TabRelease::Click(1));

        assert!(state.begin_pointer(0, 50.0, 3));
        assert!(state.update_drag(0, 280.0, &[100.0, 100.0, 100.0], 0.0));
        assert_eq!(state.finish(0), TabRelease::Reorder { from: 0, to: 2 });
    }

    #[test]
    fn shared_tab_state_animates_marquee_in_both_clock_domains() {
        let mut state = TabStripState::<String, f64>::default();
        let key = "long-tab".to_owned();
        assert!(state.set_name_hover(key.clone(), true, 55.0, 0.0));
        assert!((state.name_offset(&key, 500.0) + 27.5).abs() < f32::EPSILON);
        assert!(state.name_animation_is_running(500.0));
        assert!(!state.name_animation_is_running(1_000.0));
    }
}
