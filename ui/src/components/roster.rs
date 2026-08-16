use std::{collections::HashSet, ops::Range, time::Duration};

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, Hsla, ImageSource, IntoElement,
    ObjectFit, Render, RenderOnce, SharedString, Stateful, StyledImage as _, Window, div,
    ease_in_out, img, prelude::*, px, rgb, rgba,
};

use crate::{
    Portrait, RosterUser, RosterUserTone, UiAssets,
    animation::AnimationClock,
    components::controls,
    theme::{
        FONT_INTERFACE, FONT_INTERNATIONAL, MUTED, PANEL_BACKGROUND, PANEL_BORDER, PANEL_HEADER,
        PANEL_SHELL, ROSTER_ROW_HEIGHT, ROSTER_WIDTH, TEXT,
    },
};

const TRANSITION_DURATION: Duration = Duration::from_millis(240);
const FULL_REVEAL_DURATION: Duration = Duration::from_millis(180);
const MAX_ANIMATED_CHANGES: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowMotion {
    Stable,
    Inserted,
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RowFrame {
    pub slot_height: f32,
    pub opacity: f32,
    pub x: f32,
    pub y: f32,
}

pub struct Transition<T> {
    previous: Vec<T>,
    inserted: HashSet<u32>,
    removed: HashSet<u32>,
    full_reveal: bool,
}

pub struct TimedTransition<S, T, C> {
    pub scope: S,
    pub transition: Transition<T>,
    pub started: C,
}

pub struct PlacedRows<T> {
    pub rows: Vec<(T, RowMotion, f32)>,
    pub reveal_opacity: f32,
}

impl<S, T: Clone, C: AnimationClock> TimedTransition<S, T, C> {
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.transition.duration()
    }

    #[must_use]
    pub fn progress(&self, now: C) -> f32 {
        transition_progress(now.elapsed(self.started), self.duration())
    }

    #[must_use]
    pub fn is_running(&self, now: C) -> bool {
        now.elapsed(self.started) < self.duration()
    }
}

impl<T: Clone> Transition<T> {
    #[must_use]
    pub fn new(previous: Vec<T>, next: &[T], handle: impl Fn(&T) -> u32) -> Option<Self> {
        let previous_handles = previous.iter().map(&handle).collect::<Vec<_>>();
        let next_handles = next.iter().map(&handle).collect::<Vec<_>>();
        if previous_handles == next_handles {
            return None;
        }
        let Some((removed, inserted)) = roster_diff(&previous_handles, &next_handles) else {
            return Some(Self {
                previous: Vec::new(),
                inserted: HashSet::new(),
                removed: HashSet::new(),
                full_reveal: true,
            });
        };
        if removed.len() + inserted.len() > MAX_ANIMATED_CHANGES {
            return Some(Self {
                previous: Vec::new(),
                inserted: HashSet::new(),
                removed: HashSet::new(),
                full_reveal: true,
            });
        }
        Some(Self {
            inserted: inserted
                .iter()
                .filter_map(|index| next_handles.get(*index).copied())
                .collect(),
            removed: removed
                .iter()
                .filter_map(|index| previous_handles.get(*index).copied())
                .collect(),
            previous,
            full_reveal: false,
        })
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        if self.full_reveal {
            FULL_REVEAL_DURATION
        } else {
            TRANSITION_DURATION
        }
    }

    #[must_use]
    pub fn is_full_reveal(&self) -> bool {
        self.full_reveal
    }

    #[must_use]
    pub fn rows(&self, next: &[T], handle: impl Fn(&T) -> u32) -> Vec<(T, RowMotion)> {
        if self.full_reveal {
            return next
                .iter()
                .cloned()
                .map(|item| (item, RowMotion::Stable))
                .collect();
        }
        let mut rows = self
            .previous
            .iter()
            .filter_map(|previous| {
                let previous_handle = handle(previous);
                if self.removed.contains(&previous_handle) {
                    Some((previous.clone(), RowMotion::Removed))
                } else {
                    next.iter()
                        .find(|item| handle(item) == previous_handle)
                        .cloned()
                        .map(|item| (item, RowMotion::Stable))
                }
            })
            .collect::<Vec<_>>();
        for (index, item) in next.iter().enumerate() {
            let item_handle = handle(item);
            if !self.inserted.contains(&item_handle) {
                continue;
            }
            let next_anchor = next[index + 1..]
                .iter()
                .find(|candidate| !self.inserted.contains(&handle(candidate)))
                .map(&handle);
            let insertion = next_anchor
                .and_then(|anchor| {
                    rows.iter()
                        .position(|(candidate, _)| handle(candidate) == anchor)
                })
                .unwrap_or(rows.len());
            rows.insert(insertion, (item.clone(), RowMotion::Inserted));
        }
        rows
    }
}

#[must_use]
pub fn placed_rows<S, T: Clone, C: AnimationClock>(
    items: Vec<T>,
    animation: Option<&TimedTransition<S, T, C>>,
    now: C,
    handle: impl Fn(&T) -> u32,
) -> PlacedRows<T> {
    let Some(animation) = animation.filter(|animation| animation.is_running(now)) else {
        return PlacedRows {
            rows: items
                .into_iter()
                .map(|item| (item, RowMotion::Stable, 1.0))
                .collect(),
            reveal_opacity: 1.0,
        };
    };
    let progress = animation.progress(now);
    let rows = if animation.transition.is_full_reveal() {
        items
            .into_iter()
            .map(|item| (item, RowMotion::Stable, 1.0))
            .collect()
    } else {
        animation
            .transition
            .rows(&items, handle)
            .into_iter()
            .map(|(item, motion)| (item, motion, progress))
            .collect()
    };
    PlacedRows {
        rows,
        reveal_opacity: 1.0,
    }
}

#[must_use]
fn transition_progress(elapsed: Duration, duration: Duration) -> f32 {
    ease_in_out((elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0))
}

#[must_use]
fn row_frame(motion: RowMotion, progress: f32, row_height: f32, row_gap: f32) -> RowFrame {
    let full_height = row_height + row_gap;
    match motion {
        RowMotion::Stable => RowFrame {
            slot_height: full_height,
            opacity: 1.0,
            x: 0.0,
            y: 0.0,
        },
        RowMotion::Inserted => RowFrame {
            slot_height: full_height * progress,
            opacity: progress,
            x: 0.0,
            y: -10.0 * (1.0 - progress),
        },
        RowMotion::Removed => RowFrame {
            slot_height: full_height * (1.0 - progress),
            opacity: 1.0 - progress,
            x: -18.0 * progress,
            y: 0.0,
        },
    }
}

#[must_use]
pub fn animated_row_slot(
    row: impl IntoElement,
    motion: RowMotion,
    progress: f32,
    reveal_opacity: f32,
    row_gap: f32,
) -> Div {
    animated_row_slot_with_height(
        row,
        motion,
        progress,
        reveal_opacity,
        ROSTER_ROW_HEIGHT,
        row_gap,
    )
}

#[must_use]
pub fn animated_row_slot_with_height(
    row: impl IntoElement,
    motion: RowMotion,
    progress: f32,
    reveal_opacity: f32,
    row_height: f32,
    row_gap: f32,
) -> Div {
    let frame = row_frame(motion, progress, row_height, row_gap);
    div()
        .relative()
        .w_full()
        .h(px(frame.slot_height))
        .flex_shrink_0()
        .overflow_hidden()
        .opacity(reveal_opacity)
        .child(
            div()
                .relative()
                .left(px(frame.x))
                .top(px(frame.y))
                .h(px(row_height))
                .w_full()
                .opacity(frame.opacity)
                .child(row),
        )
}

#[must_use]
pub fn animated_rows<S, T: Clone, C: AnimationClock>(
    items: Vec<T>,
    animation: Option<&TimedTransition<S, T, C>>,
    now: C,
    handle: impl Fn(&T) -> u32,
    row_gap: f32,
    mut render: impl FnMut(&T, RowMotion) -> AnyElement,
) -> Vec<AnyElement> {
    let placement = placed_rows(items, animation, now, handle);
    let reveal_opacity = placement.reveal_opacity;
    placement
        .rows
        .into_iter()
        .map(|(item, motion, progress)| {
            animated_row_slot(
                render(&item, motion),
                motion,
                progress,
                reveal_opacity,
                row_gap,
            )
            .into_any_element()
        })
        .collect()
}

#[must_use]
pub fn animated_rows_with_height<S, T: Clone, C: AnimationClock>(
    items: Vec<T>,
    animation: Option<&TimedTransition<S, T, C>>,
    now: C,
    handle: impl Fn(&T) -> u32,
    row_height: f32,
    row_gap: f32,
    mut render: impl FnMut(&T, RowMotion) -> AnyElement,
) -> Vec<AnyElement> {
    let placement = placed_rows(items, animation, now, handle);
    let reveal_opacity = placement.reveal_opacity;
    placement
        .rows
        .into_iter()
        .map(|(item, motion, progress)| {
            animated_row_slot_with_height(
                render(&item, motion),
                motion,
                progress,
                reveal_opacity,
                row_height,
                row_gap,
            )
            .into_any_element()
        })
        .collect()
}

#[must_use]
pub fn virtual_row_slot(row: impl IntoElement, row_gap: f32) -> Div {
    div()
        .relative()
        .w_full()
        .h(px(ROSTER_ROW_HEIGHT + row_gap))
        .flex_shrink_0()
        .overflow_hidden()
        .child(row)
}

#[must_use]
pub fn filter_matches(candidate: &str, normalized_filter: &str) -> bool {
    normalized_filter.is_empty() || candidate.to_lowercase().contains(normalized_filter)
}

#[must_use]
pub fn filtered_refs<'a, T>(
    items: &'a [T],
    filter: &str,
    mut matches: impl FnMut(&T, &str) -> bool,
) -> Vec<&'a T> {
    let filter = filter.trim().to_lowercase();
    items.iter().filter(|item| matches(item, &filter)).collect()
}

#[must_use]
pub fn filtered_count<T>(
    items: &[T],
    filter: &str,
    mut matches: impl FnMut(&T, &str) -> bool,
) -> usize {
    let filter = filter.trim().to_lowercase();
    items.iter().filter(|item| matches(item, &filter)).count()
}

#[must_use]
pub fn filtered_range<T: Clone>(
    items: &[T],
    filter: &str,
    range: Range<usize>,
    mut matches: impl FnMut(&T, &str) -> bool,
) -> Vec<T> {
    if range.is_empty() {
        return Vec::new();
    }
    let filter = filter.trim().to_lowercase();
    items
        .iter()
        .filter(|item| matches(item, &filter))
        .skip(range.start)
        .take(range.len())
        .cloned()
        .collect()
}

fn roster_diff(previous: &[u32], next: &[u32]) -> Option<(Vec<usize>, Vec<usize>)> {
    let previous_set = previous.iter().copied().collect::<HashSet<_>>();
    let next_set = next.iter().copied().collect::<HashSet<_>>();
    let common_previous = previous
        .iter()
        .filter(|handle| next_set.contains(handle))
        .copied()
        .collect::<Vec<_>>();
    let common_next = next
        .iter()
        .filter(|handle| previous_set.contains(handle))
        .copied()
        .collect::<Vec<_>>();
    (common_previous == common_next).then(|| {
        (
            previous
                .iter()
                .enumerate()
                .filter_map(|(index, handle)| (!next_set.contains(handle)).then_some(index))
                .collect(),
            next.iter()
                .enumerate()
                .filter_map(|(index, handle)| (!previous_set.contains(handle)).then_some(index))
                .collect(),
        )
    })
}

fn portrait(user: &RosterUser, assets: &UiAssets) -> AnyElement {
    portrait_at(user, assets, 12.0, 7.0, 38.0)
}

fn portrait_at(user: &RosterUser, assets: &UiAssets, left: f32, top: f32, size: f32) -> AnyElement {
    match &user.portrait {
        Some(Portrait::Image(source)) => img(source.clone())
            .absolute()
            .left(px(left))
            .top(px(top))
            .size(px(size))
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        Some(Portrait::Atlas {
            image,
            cell,
            columns,
            ..
        }) => {
            let column = f32::from(*cell % *columns);
            let row = f32::from(*cell / *columns);
            let atlas_size = size * f32::from(*columns);
            div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .size(px(size))
                .overflow_hidden()
                .child(
                    img(image.clone())
                        .absolute()
                        .left(px(-column * size))
                        .top(px(-row * size))
                        .size(px(atlas_size))
                        .object_fit(ObjectFit::Fill),
                )
                .into_any_element()
        }
        None => img(assets.portrait_placeholder.clone())
            .absolute()
            .left(px(left))
            .top(px(top))
            .size(px(size))
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
    }
}

pub struct RosterTooltip {
    user: RosterUser,
    channel: String,
    assets: UiAssets,
}

impl RosterTooltip {
    #[must_use]
    pub fn new(user: RosterUser, channel: String, assets: UiAssets) -> Self {
        Self {
            user,
            channel,
            assets,
        }
    }
}

impl Render for RosterTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let presence = self.user.presence_id.map_or_else(
            || self.user.presence_label.clone(),
            |id| format!("{} · #{id}", self.user.presence_label),
        );
        let tooltip = controls::tooltip_shell(286.0, 152.0, self.assets.tooltip_fill.clone())
            .font_family(FONT_INTERFACE)
            .child(portrait_at(&self.user, &self.assets, 16.0, 20.0, 48.0))
            .child(
                img(self.assets.portrait_frame.clone())
                    .absolute()
                    .left(px(12.0))
                    .top(px(16.0))
                    .size(px(56.0))
                    .object_fit(ObjectFit::Fill),
            )
            .child(
                div()
                    .absolute()
                    .left(px(80.0))
                    .right(px(16.0))
                    .top(px(22.0))
                    .h(px(22.0))
                    .font_family(FONT_INTERNATIONAL)
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_size(px(14.0))
                    .text_color(rgb(TEXT))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(self.user.name.clone()),
            )
            .child(
                presence_line(
                    self.user.presence_icon.clone(),
                    self.user.presence_label.clone(),
                    13.0,
                    5.0,
                    11.5,
                    rgb(MUTED).into(),
                )
                .absolute()
                .left(px(80.0))
                .right(px(16.0))
                .top(px(51.0))
                .h(px(18.0)),
            )
            .child(tooltip_detail("CHANNEL", &self.channel, 86.0))
            .child(tooltip_detail("PRESENCE", &presence, 114.0));
        controls::animated_tooltip(tooltip, "roster-tooltip-open", 0.0, 0.0, -8.0)
    }
}

fn tooltip_detail(key: &'static str, value: &str, top: f32) -> Div {
    div()
        .absolute()
        .left(px(16.0))
        .right(px(16.0))
        .top(px(top))
        .h(px(18.0))
        .flex()
        .items_center()
        .child(
            div()
                .w(px(76.0))
                .font_weight(gpui::FontWeight::BOLD)
                .text_size(px(10.0))
                .text_color(rgb(MUTED))
                .child(key),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(11.0))
                .text_color(rgb(TEXT))
                .child(value.to_owned()),
        )
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type HoverHandler = Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RosterAvailability {
    Online,
    Loading,
    Offline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RosterHeaderModel {
    pub heading: String,
    pub count: String,
    pub filter_active: bool,
}

impl RosterHeaderModel {
    #[must_use]
    pub fn new(
        title: impl Into<String>,
        total: usize,
        filtered: usize,
        filter: &str,
        focused: bool,
        availability: RosterAvailability,
    ) -> Self {
        let filter_active = !filter.is_empty();
        let title = title.into();
        let heading = if filter_active {
            format!("{title}  /  {filter}")
        } else {
            title
        };
        let count = match availability {
            RosterAvailability::Offline => format!("{total} last seen"),
            _ if filter_active => format!("{filtered} / {total}"),
            _ if focused => "type to filter".to_owned(),
            RosterAvailability::Online => format!("{total} online"),
            RosterAvailability::Loading => format!("{total} syncing"),
        };
        Self {
            heading,
            count,
            filter_active,
        }
    }

    #[must_use]
    pub fn heading_color(&self, focused: bool) -> Hsla {
        if focused || self.filter_active {
            rgb(0x39aee8).into()
        } else {
            rgb(TEXT).into()
        }
    }
}

#[derive(IntoElement)]
pub struct RosterHeader {
    id: String,
    heading: String,
    count: String,
    heading_color: Hsla,
    on_focus: Option<ClickHandler>,
    on_clear: Option<ClickHandler>,
    tooltip: Option<(ChannelTooltipModel, UiAssets)>,
}

impl RosterHeader {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        heading: impl Into<String>,
        count: impl Into<String>,
        heading_color: impl Into<Hsla>,
    ) -> Self {
        Self {
            id: id.into(),
            heading: heading.into(),
            count: count.into(),
            heading_color: heading_color.into(),
            on_focus: None,
            on_clear: None,
            tooltip: None,
        }
    }

    #[must_use]
    pub fn on_focus(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_focus = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_clear(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_clear = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn channel_tooltip(mut self, model: ChannelTooltipModel, assets: UiAssets) -> Self {
        self.tooltip = Some((model, assets));
        self
    }
}

impl RenderOnce for RosterHeader {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let filter_active = self.on_clear.is_some();
        let mut header = div()
            .id(self.id)
            .absolute()
            .inset_0()
            .font_family(FONT_INTERFACE);
        if let Some((model, assets)) = self.tooltip {
            header = header.tooltip(move |_, cx| {
                cx.new(|_| ChannelTooltip::new(model.clone(), assets.clone()))
                    .into()
            });
        }
        if let Some(on_focus) = self.on_focus {
            header = header
                .cursor_pointer()
                .on_click(move |event, window, cx| on_focus(event, window, cx));
        }
        header = header.child(filter_header(
            self.heading,
            self.count,
            self.heading_color,
            filter_active,
        ));
        if let Some(on_clear) = self.on_clear {
            header = header.child(
                div()
                    .id("roster-filter-clear")
                    .absolute()
                    .right(px(12.0))
                    .top(px(10.0))
                    .size(px(22.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .font_family(FONT_INTERFACE)
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_size(px(14.0))
                    .text_color(rgb(0x39aee8))
                    .hover(|style| style.text_color(rgb(0xffffff)))
                    .active(|style| style.opacity(0.64))
                    .on_click(move |event, window, cx| on_clear(event, window, cx))
                    .child("×"),
            );
        }
        header
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChannelTooltipModel {
    pub name: String,
    pub channel_type: String,
    pub shard: Option<u16>,
    pub identity: String,
}

impl ChannelTooltipModel {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        channel_type: impl Into<String>,
        shard: Option<u16>,
        identity: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            channel_type: channel_type.into(),
            shard,
            identity: identity.into(),
        }
    }
}

pub struct ChannelTooltip {
    model: ChannelTooltipModel,
    assets: UiAssets,
}

impl ChannelTooltip {
    #[must_use]
    pub fn new(model: ChannelTooltipModel, assets: UiAssets) -> Self {
        Self { model, assets }
    }
}

impl Render for ChannelTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let shard = self
            .model
            .shard
            .map_or_else(|| "Not reported".to_owned(), |shard| shard.to_string());
        let tooltip = controls::tooltip_shell(300.0, 146.0, self.assets.tooltip_fill.clone())
            .font_family(FONT_INTERFACE)
            .child(
                div()
                    .absolute()
                    .left(px(16.0))
                    .right(px(16.0))
                    .top(px(17.0))
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_family(FONT_INTERNATIONAL)
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_size(px(15.0))
                    .text_color(rgb(TEXT))
                    .child(self.model.name.clone()),
            )
            .child(
                div()
                    .absolute()
                    .left(px(16.0))
                    .right(px(16.0))
                    .top(px(49.0))
                    .h(px(1.0))
                    .bg(rgba(0x1853_78a6)),
            )
            .child(tooltip_detail("TYPE", &self.model.channel_type, 62.0))
            .child(tooltip_detail("SHARD", &shard, 88.0))
            .child(tooltip_detail("IDENTITY", &self.model.identity, 114.0));
        controls::animated_tooltip(tooltip, "channel-tooltip-open", 0.0, 0.0, -8.0)
    }
}

#[derive(IntoElement)]
pub struct RosterRow {
    id: String,
    group: String,
    user: RosterUser,
    channel: String,
    selected: bool,
    assets: UiAssets,
    on_click: Option<ClickHandler>,
}

impl RosterRow {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        group: impl Into<String>,
        user: RosterUser,
        channel: impl Into<String>,
        selected: bool,
        assets: UiAssets,
    ) -> Self {
        Self {
            id: id.into(),
            group: group.into(),
            user,
            channel: channel.into(),
            selected,
            assets,
            on_click: None,
        }
    }

    #[must_use]
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for RosterRow {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let tooltip_user = self.user.clone();
        let tooltip_channel = self.channel;
        let tooltip_assets = self.assets.clone();
        let mut row = div()
            .id(self.id)
            .group(self.group.clone())
            .relative()
            .h(px(ROSTER_ROW_HEIGHT))
            .w_full()
            .flex_shrink_0()
            .cursor_pointer()
            .tooltip(move |_, cx| {
                cx.new(|_| {
                    RosterTooltip::new(
                        tooltip_user.clone(),
                        tooltip_channel.clone(),
                        tooltip_assets.clone(),
                    )
                })
                .into()
            });
        if let Some(on_click) = self.on_click {
            row = row.on_click(move |event, window, cx| on_click(event, window, cx));
        }
        row.child(segment_divider(&self.user))
            .child(
                selection(self.selected, self.user.tone)
                    .group_hover(self.group, |style| style.opacity(1.0)),
            )
            .child(row_body(&self.user, &self.assets))
    }
}

#[must_use]
fn row_body(user: &RosterUser, assets: &UiAssets) -> Div {
    div()
        .relative()
        .size_full()
        .child(portrait(user, assets))
        .child(
            img(assets.portrait_frame.clone())
                .absolute()
                .left(px(8.0))
                .top(px(3.0))
                .size(px(46.0))
                .object_fit(ObjectFit::Fill),
        )
        .child(
            div()
                .absolute()
                .left(px(62.0))
                .right(px(10.0))
                .top(px(10.0))
                .h(px(16.0))
                .flex()
                .items_center()
                .overflow_hidden()
                .whitespace_nowrap()
                .font_family(FONT_INTERNATIONAL)
                .text_size(px(13.0))
                .text_color(username_color(user.tone))
                .opacity(if user.dimmed { 0.54 } else { 1.0 })
                .child(user.name.clone()),
        )
        .child(
            presence_line(
                user.presence_icon.clone(),
                user.presence_label.clone(),
                14.0,
                4.0,
                11.5,
                rgb(MUTED).into(),
            )
            .absolute()
            .left(px(62.0))
            .right(px(10.0))
            .top(px(29.0))
            .h(px(18.0)),
        )
}

#[must_use]
pub fn presence_line(
    icon: ImageSource,
    label: impl Into<SharedString>,
    icon_size: f32,
    gap: f32,
    text_size: f32,
    text_color: Hsla,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(gap))
        .overflow_hidden()
        .font_family(FONT_INTERFACE)
        .child(
            img(icon)
                .size(px(icon_size))
                .flex_shrink_0()
                .object_fit(ObjectFit::Contain),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(text_size))
                .text_color(text_color)
                .child(label.into()),
        )
}

#[must_use]
fn selection(selected: bool, tone: RosterUserTone) -> Div {
    let (background, border) = match tone {
        RosterUserTone::Clan => (rgba(0x3b24_10eb), rgba(0xd68b_43e0)),
        RosterUserTone::Party => (rgba(0x2816_3ceb), rgba(0x993d_dbd9)),
        RosterUserTone::Normal => (rgba(0x1231_5ef5), rgba(0x003d_9be6)),
    };
    div()
        .absolute()
        .left(px(4.0))
        .right(px(4.0))
        .top(px(1.0))
        .bottom(px(1.0))
        .opacity(if selected { 1.0 } else { 0.0 })
        .bg(background)
        .border_1()
        .border_color(border)
}

#[must_use]
fn username_color(tone: RosterUserTone) -> Hsla {
    match tone {
        RosterUserTone::Clan => rgb(0xf0aa64).into(),
        RosterUserTone::Party => rgb(0xf092c4).into(),
        RosterUserTone::Normal => rgb(TEXT).into(),
    }
}

#[must_use]
fn segment_divider(user: &RosterUser) -> Div {
    let color = match user.tone {
        RosterUserTone::Clan => rgba(0xf0aa_648f),
        RosterUserTone::Party => rgba(0xf092_c48f),
        RosterUserTone::Normal => rgba(0x6bc2_f266),
    };
    div()
        .absolute()
        .left(px(8.0))
        .right(px(8.0))
        .top_0()
        .h(px(1.0))
        .bg(color)
        .opacity(if user.segment_start { 1.0 } else { 0.0 })
}

#[must_use]
pub fn static_row(user: &RosterUser, assets: &UiAssets, selected: bool) -> Div {
    div()
        .relative()
        .h(px(ROSTER_ROW_HEIGHT))
        .w_full()
        .flex_shrink_0()
        .child(segment_divider(user))
        .child(selection(selected, user.tone))
        .child(row_body(user, assets))
}

#[must_use]
fn filter_header(title: String, count: String, title_color: Hsla, has_clear_control: bool) -> Div {
    div()
        .absolute()
        .inset_0()
        .font_family(FONT_INTERFACE)
        .child(
            div()
                .absolute()
                .left(px(14.0))
                .top(px(14.0))
                .right(px(128.0))
                .h(px(20.0))
                .flex()
                .items_center()
                .overflow_hidden()
                .whitespace_nowrap()
                .font_weight(gpui::FontWeight::BOLD)
                .text_size(px(13.0))
                .text_color(title_color)
                .child(title),
        )
        .child(
            div()
                .absolute()
                .right(px(if has_clear_control { 38.0 } else { 14.0 }))
                .top(px(16.0))
                .w(px(110.0))
                .h(px(16.0))
                .flex()
                .items_center()
                .justify_end()
                .text_size(px(12.0))
                .text_color(rgb(MUTED))
                .child(count),
        )
}

#[must_use]
pub(crate) fn header_layer() -> Div {
    div().absolute().top_0().left_0().w_full().h(px(42.0))
}

#[must_use]
fn panel_with_width(width: Option<f32>) -> Div {
    let panel = div()
        .relative()
        .h_full()
        .flex_shrink_0()
        .border_1()
        .border_color(rgb(PANEL_BORDER))
        .bg(rgb(PANEL_SHELL))
        .child(
            div()
                .absolute()
                .left(px(5.0))
                .right(px(5.0))
                .top(px(5.0))
                .h(px(37.0))
                .bg(rgb(PANEL_HEADER)),
        )
        .child(
            div()
                .absolute()
                .left(px(5.0))
                .right(px(5.0))
                .top(px(41.0))
                .h(px(1.0))
                .bg(rgba(0x133e_5ba6)),
        );
    if let Some(width) = width {
        panel.w(px(width))
    } else {
        panel.w_full()
    }
}

#[must_use]
pub(crate) fn rows() -> Div {
    div()
        .absolute()
        .left(px(6.0))
        .right(px(6.0))
        .top(px(48.0))
        .bottom(px(6.0))
        .overflow_hidden()
        .bg(rgb(PANEL_BACKGROUND))
}

#[must_use]
pub fn list_layer(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .overflow_hidden()
}

#[derive(IntoElement)]
pub(crate) struct RosterPanel {
    header: AnyElement,
    rows: AnyElement,
    width: Option<f32>,
    overlays: Vec<AnyElement>,
    focused: bool,
    on_hover: Option<HoverHandler>,
}

impl RosterPanel {
    #[must_use]
    pub(crate) fn new(header: impl IntoElement, rows: impl IntoElement) -> Self {
        Self {
            header: header.into_any_element(),
            rows: rows.into_any_element(),
            width: Some(ROSTER_WIDTH),
            overlays: Vec::new(),
            focused: false,
            on_hover: None,
        }
    }

    #[must_use]
    pub(crate) const fn width(mut self, width: Option<f32>) -> Self {
        self.width = width;
        self
    }

    #[must_use]
    pub(crate) fn overlay(mut self, overlay: impl IntoElement) -> Self {
        self.overlays.push(overlay.into_any_element());
        self
    }

    #[must_use]
    pub(crate) const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    #[must_use]
    pub(crate) fn on_hover(
        mut self,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_hover = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for RosterPanel {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut panel = panel_with_width(self.width)
            .id("channel-roster")
            .child(self.header)
            .child(self.rows)
            .children(self.overlays)
            .when(self.focused, |panel| {
                panel.child(
                    div()
                        .absolute()
                        .inset_0()
                        .border_1()
                        .border_color(rgba(0x39ba_ffb8)),
                )
            });
        if let Some(on_hover) = self.on_hover {
            panel = panel.on_hover(on_hover);
        }
        panel
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RosterAvailability, RosterHeaderModel, RowMotion, TimedTransition, Transition,
        filter_matches, filtered_count, filtered_range, placed_rows,
    };

    #[derive(Clone)]
    struct Item(u32);

    #[test]
    fn transition_keeps_removed_and_inserted_rows_in_place() {
        let next = [Item(10), Item(30), Item(40)];
        let transition = Transition::new(vec![Item(10), Item(20), Item(40)], &next, |item| item.0)
            .expect("roster changed");
        let rows = transition.rows(&next, |item| item.0);
        assert_eq!(
            rows.iter().map(|(item, _)| item.0).collect::<Vec<_>>(),
            vec![10, 20, 30, 40]
        );
        assert_eq!(rows[1].1, RowMotion::Removed);
        assert_eq!(rows[2].1, RowMotion::Inserted);
    }

    #[test]
    fn transition_uses_full_reveal_for_reorders() {
        let transition = Transition::new(vec![Item(10), Item(20)], &[Item(20), Item(10)], |item| {
            item.0
        })
        .expect("roster changed");
        assert!(transition.is_full_reveal());
    }

    #[test]
    fn full_reveal_does_not_dim_unchanged_rows() {
        let next = vec![Item(20), Item(10)];
        let transition = Transition::new(vec![Item(10), Item(20)], &next, |item| item.0)
            .expect("roster changed");
        let animation = TimedTransition {
            scope: "general",
            transition,
            started: 0.0_f64,
        };
        let placement = placed_rows(next, Some(&animation), 90.0, |item| item.0);
        assert!((placement.reveal_opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn filtering_is_case_insensitive_and_virtualization_safe() {
        let items = ["Alpha", "Bravo", "Charlie", "Delta"];
        let matches = |item: &&str, filter: &str| filter_matches(item, filter);
        assert_eq!(filtered_count(&items, "A", matches), 4);
        assert_eq!(
            filtered_range(&items, "a", 1..3, matches),
            vec!["Bravo", "Charlie"]
        );
        assert_eq!(
            filtered_range(&items, "", 2..8, matches),
            vec!["Charlie", "Delta"]
        );
    }

    #[test]
    fn roster_header_model_centralizes_filter_and_availability_copy() {
        let filtered = RosterHeaderModel::new(
            "General",
            180,
            7,
            "raynor",
            true,
            RosterAvailability::Online,
        );
        assert_eq!(filtered.heading, "General  /  raynor");
        assert_eq!(filtered.count, "7 / 180");
        assert!(filtered.filter_active);

        let loading =
            RosterHeaderModel::new("Arcade", 12, 12, "", false, RosterAvailability::Loading);
        assert_eq!(loading.count, "12 syncing");

        let offline =
            RosterHeaderModel::new("General", 0, 0, "", false, RosterAvailability::Offline);
        assert_eq!(offline.count, "0 last seen");
    }

    #[test]
    fn shared_placement_preserves_transition_rows_and_progress() {
        let next = vec![Item(10), Item(30), Item(40)];
        let transition = Transition::new(vec![Item(10), Item(20), Item(40)], &next, |item| item.0)
            .expect("roster changed");
        let animation = TimedTransition {
            scope: "general",
            transition,
            started: 0.0_f64,
        };
        let placement = placed_rows(next, Some(&animation), 120.0, |item| item.0);
        assert_eq!(
            placement
                .rows
                .iter()
                .map(|(item, motion, _)| (item.0, *motion))
                .collect::<Vec<_>>(),
            vec![
                (10, RowMotion::Stable),
                (20, RowMotion::Removed),
                (30, RowMotion::Inserted),
                (40, RowMotion::Stable),
            ]
        );
        assert!((placement.rows[1].2 - 0.5).abs() < f32::EPSILON);
        assert!((placement.reveal_opacity - 1.0).abs() < f32::EPSILON);
    }
}
