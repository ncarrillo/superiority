use std::{any::Any, time::Duration};

use gpui::{
    Along, Anchor, App, AppContext as _, Axis, BorderStyle, Bounds, ContentMask, Context, Corners,
    CursorStyle, DispatchPhase, Div, Edges, Element, ElementId, Entity, Global, GlobalElementId,
    Hitbox, HitboxBehavior, Hsla, InteractiveElement as _, IntoElement, LayoutId, ListState,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point,
    Position, Render, ScrollHandle, Size, Stateful, StatefulInteractiveElement as _, Style, Task,
    UniformListScrollHandle, Window, prelude::FluentBuilder as _, px, quad, relative, size,
};

const HIDE_DELAY: Duration = Duration::from_secs(1);
const MINIMUM_THUMB_LENGTH: Pixels = px(25.0);
const REGULAR_PADDING: Pixels = px(4.0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShowScrollbar {
    #[default]
    Auto,
    System,
    Always,
    Never,
}

#[derive(Default)]
pub struct ScrollbarAutoHide(pub bool);

impl Global for ScrollbarAutoHide {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollbarAxes {
    Horizontal,
    Vertical,
    Both,
}

impl ScrollbarAxes {
    fn includes(self, axis: Axis) -> bool {
        matches!(
            (self, axis),
            (Self::Horizontal | Self::Both, Axis::Horizontal)
                | (Self::Vertical | Self::Both, Axis::Vertical)
        )
    }

    fn union(self, other: Self) -> Self {
        if self == other { self } else { Self::Both }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollbarStyle {
    #[default]
    Regular,
    Editor,
}

impl ScrollbarStyle {
    #[must_use]
    pub const fn width(self) -> Pixels {
        match self {
            Self::Regular => px(6.0),
            Self::Editor => px(15.0),
        }
    }

    const fn padding(self) -> Pixels {
        match self {
            Self::Regular => REGULAR_PADDING,
            Self::Editor => Pixels::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollbarRevealPolicy {
    #[default]
    ScrollOrContentChange,
    ScrollOnly,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarColors {
    pub thumb: Hsla,
    pub thumb_hover: Hsla,
    pub thumb_active: Hsla,
    pub track: Option<Hsla>,
    pub border: Option<Hsla>,
}

impl Default for ScrollbarColors {
    fn default() -> Self {
        Self {
            thumb: gpui::rgba(0x2a78_a6b3).into(),
            thumb_hover: gpui::rgba(0x3ea9_e6e6).into(),
            thumb_active: gpui::rgba(0x66c5_ffff).into(),
            track: None,
            border: None,
        }
    }
}

#[derive(Clone)]
enum Handle<T: ScrollableHandle> {
    Owned(T),
    Tracked(T),
}

impl<T: ScrollableHandle> Handle<T> {
    fn value(&self) -> &T {
        match self {
            Self::Owned(handle) | Self::Tracked(handle) => handle,
        }
    }

    fn is_owned(&self) -> bool {
        matches!(self, Self::Owned(_))
    }
}

#[derive(Clone)]
pub struct Scrollbars<T: ScrollableHandle = ScrollHandle> {
    id: Option<ElementId>,
    handle: Handle<T>,
    axes: ScrollbarAxes,
    track_axes: Option<ScrollbarAxes>,
    stable_track_axes: Option<ScrollbarAxes>,
    visibility: ShowScrollbar,
    style: ScrollbarStyle,
    reveal_policy: ScrollbarRevealPolicy,
    colors: ScrollbarColors,
}

impl Scrollbars {
    #[must_use]
    pub fn new(axes: ScrollbarAxes) -> Self {
        Self {
            id: None,
            handle: Handle::Owned(ScrollHandle::new()),
            axes,
            track_axes: None,
            stable_track_axes: None,
            visibility: ShowScrollbar::Auto,
            style: ScrollbarStyle::Regular,
            reveal_policy: ScrollbarRevealPolicy::ScrollOrContentChange,
            colors: ScrollbarColors::default(),
        }
    }

    #[must_use]
    pub fn always_visible(axes: ScrollbarAxes) -> Self {
        Self::new(axes).visibility(ShowScrollbar::Always)
    }
}

impl<T: ScrollableHandle> Scrollbars<T> {
    #[must_use]
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    #[must_use]
    pub fn tracked_scroll_handle<U: ScrollableHandle>(self, handle: &U) -> Scrollbars<U> {
        Scrollbars {
            id: self.id,
            handle: Handle::Tracked(handle.clone()),
            axes: self.axes,
            track_axes: self.track_axes,
            stable_track_axes: self.stable_track_axes,
            visibility: self.visibility,
            style: self.style,
            reveal_policy: self.reveal_policy,
            colors: self.colors,
        }
    }

    #[must_use]
    pub fn show_along(mut self, axes: ScrollbarAxes) -> Self {
        self.axes = self.axes.union(axes);
        self
    }

    #[must_use]
    pub fn visibility(mut self, visibility: ShowScrollbar) -> Self {
        self.visibility = visibility;
        self
    }

    #[must_use]
    pub fn style(mut self, style: ScrollbarStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn reveal_policy(mut self, reveal_policy: ScrollbarRevealPolicy) -> Self {
        self.reveal_policy = reveal_policy;
        self
    }

    #[must_use]
    pub fn colors(mut self, colors: ScrollbarColors) -> Self {
        self.colors = colors;
        self
    }

    #[must_use]
    pub fn with_track_along(mut self, axes: ScrollbarAxes, color: Hsla) -> Self {
        self.axes = self.axes.union(axes);
        self.track_axes = Some(self.track_axes.map_or(axes, |current| current.union(axes)));
        self.colors.track = Some(color);
        self
    }

    #[must_use]
    pub fn with_stable_track_along(mut self, axes: ScrollbarAxes, color: Hsla) -> Self {
        self = self.with_track_along(axes, color);
        self.stable_track_axes = Some(
            self.stable_track_axes
                .map_or(axes, |current| current.union(axes)),
        );
        self
    }

    fn ensure_id(mut self, id: impl Into<ElementId>) -> Self {
        if self.id.is_none() {
            self.id = Some(id.into());
        }
        self
    }
}

pub trait WithScrollbar: Sized {
    type Output;

    fn custom_scrollbars<T: ScrollableHandle>(
        self,
        scrollbars: Scrollbars<T>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Output;

    /// a vertical scrollbar in Superiority's own chrome — for surfaces that
    /// belong to no realm (the picker, the tool windows). A product surface
    /// uses [`WithScrollbar::vertical_scrollbar_in`] with its realm's colours.
    #[track_caller]
    fn vertical_scrollbar_for<T: ScrollableHandle>(
        self,
        handle: &T,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Output {
        self.custom_scrollbars(
            Scrollbars::new(ScrollbarAxes::Vertical)
                .tracked_scroll_handle(handle)
                .ensure_id(std::panic::Location::caller()),
            window,
            cx,
        )
    }

    /// a vertical scrollbar in the caller's colours: every realm's surface
    /// passes its own, so a scrollbar never wears another realm's chrome.
    #[track_caller]
    fn vertical_scrollbar_in<T: ScrollableHandle>(
        self,
        handle: &T,
        colors: ScrollbarColors,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Output {
        self.custom_scrollbars(
            Scrollbars::new(ScrollbarAxes::Vertical)
                .tracked_scroll_handle(handle)
                .colors(colors)
                .ensure_id(std::panic::Location::caller()),
            window,
            cx,
        )
    }
}

impl WithScrollbar for Stateful<Div> {
    type Output = Self;

    #[track_caller]
    fn custom_scrollbars<T: ScrollableHandle>(
        self,
        scrollbars: Scrollbars<T>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Output {
        let scrollbars = scrollbars.ensure_id(std::panic::Location::caller());
        render_scrollbars(self, &scrollbars, window, cx)
    }
}

impl WithScrollbar for Div {
    type Output = Stateful<Div>;

    #[track_caller]
    fn custom_scrollbars<T: ScrollableHandle>(
        self,
        scrollbars: Scrollbars<T>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Output {
        let scrollbars = scrollbars.ensure_id(std::panic::Location::caller());
        let id = scrollbars
            .id
            .clone()
            .unwrap_or_else(|| std::panic::Location::caller().into());
        render_scrollbars(self.id((id, "scrollable")), &scrollbars, window, cx)
    }
}

struct ScrollbarStateHolder<T: ScrollableHandle>(Entity<ScrollbarState<T>>);

fn render_scrollbars<T: ScrollableHandle>(
    mut container: Stateful<Div>,
    scrollbars: &Scrollbars<T>,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let element_id = scrollbars
        .id
        .clone()
        .unwrap_or_else(|| "anonymous-scrollbar".into());
    let state_holder = window.use_keyed_state(element_id, cx, |_, cx| {
        ScrollbarStateHolder(cx.new(|cx| ScrollbarState::new(scrollbars, cx)))
    });
    let state = state_holder.read(cx).0.clone();
    state.update(cx, |state, cx| state.update_config(scrollbars, cx));

    let state_handle = state.read(cx).handle.clone();
    if scrollbars.handle.is_owned()
        && let Some(handle) = (&state_handle as &dyn Any).downcast_ref::<ScrollHandle>()
    {
        container = match scrollbars.axes {
            ScrollbarAxes::Horizontal => container.overflow_x_scroll(),
            ScrollbarAxes::Vertical => container.overflow_y_scroll(),
            ScrollbarAxes::Both => container.overflow_scroll(),
        }
        .track_scroll(handle);
    }

    let reserve_horizontal = state.read(cx).reserve_space(Axis::Horizontal);
    let reserve_vertical = state.read(cx).reserve_space(Axis::Vertical);
    container
        .when_some(reserve_horizontal, gpui::Styled::pb)
        .when_some(reserve_vertical, gpui::Styled::pr)
        .child(state)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShowBehavior {
    Auto,
    Always,
    Never,
}

impl ShowBehavior {
    fn resolve(visibility: ShowScrollbar, cx: &mut App) -> Self {
        match visibility {
            ShowScrollbar::Auto => Self::Auto,
            ShowScrollbar::System => {
                if cx.default_global::<ScrollbarAutoHide>().0 {
                    Self::Auto
                } else {
                    Self::Always
                }
            }
            ShowScrollbar::Always => Self::Always,
            ShowScrollbar::Never => Self::Never,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum ThumbInteraction {
    #[default]
    Idle,
    Hover(Axis),
    Dragging {
        axis: Axis,
        grab_offset: Pixels,
    },
}

struct ScrollbarState<T: ScrollableHandle> {
    handle: T,
    axes: ScrollbarAxes,
    track_axes: Option<ScrollbarAxes>,
    stable_track_axes: Option<ScrollbarAxes>,
    style: ScrollbarStyle,
    reveal_policy: ScrollbarRevealPolicy,
    colors: ScrollbarColors,
    show_behavior: ShowBehavior,
    visible: bool,
    parent_hovered: bool,
    interaction: ThumbInteraction,
    last_layout: Option<ScrollbarLayoutState>,
    hide_task: Option<Task<()>>,
}

impl<T: ScrollableHandle> ScrollbarState<T> {
    fn new(config: &Scrollbars<T>, cx: &mut Context<Self>) -> Self {
        let show_behavior = ShowBehavior::resolve(config.visibility, cx);
        Self {
            handle: config.handle.value().clone(),
            axes: config.axes,
            track_axes: config.track_axes,
            stable_track_axes: config.stable_track_axes,
            style: config.style,
            reveal_policy: config.reveal_policy,
            colors: config.colors,
            show_behavior,
            visible: show_behavior != ShowBehavior::Never,
            parent_hovered: false,
            interaction: ThumbInteraction::Idle,
            last_layout: None,
            hide_task: None,
        }
    }

    fn update_config(&mut self, config: &Scrollbars<T>, cx: &mut Context<Self>) {
        if let Handle::Tracked(handle) = &config.handle {
            self.handle = handle.clone();
        }
        self.axes = config.axes;
        self.track_axes = config.track_axes;
        self.stable_track_axes = config.stable_track_axes;
        self.style = config.style;
        self.reveal_policy = config.reveal_policy;
        self.colors = config.colors;
        let show_behavior = ShowBehavior::resolve(config.visibility, cx);
        if self.show_behavior != show_behavior {
            self.show_behavior = show_behavior;
            self.visible = show_behavior != ShowBehavior::Never;
            self.hide_task = None;
            cx.notify();
        }
    }

    fn reserve_space(&self, axis: Axis) -> Option<Pixels> {
        let stable = self
            .stable_track_axes
            .is_some_and(|axes| axes.includes(axis));
        let track = self.track_axes.is_some_and(|axes| axes.includes(axis));
        let scrollable = self.handle.max_offset().along(axis) > Pixels::ZERO;
        (self.show_behavior != ShowBehavior::Never && (stable || track && scrollable))
            .then(|| self.style.width() + self.style.padding() * 2.0)
    }

    fn shown(&self) -> bool {
        match self.show_behavior {
            ShowBehavior::Always => true,
            ShowBehavior::Never => false,
            ShowBehavior::Auto => self.visible,
        }
    }

    fn reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.show_behavior != ShowBehavior::Auto {
            return;
        }
        self.visible = true;
        self.hide_task = None;
        self.schedule_hide(window, cx);
        cx.notify();
    }

    fn schedule_hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.show_behavior != ShowBehavior::Auto || self.interaction != ThumbInteraction::Idle {
            return;
        }
        self.hide_task = Some(cx.spawn_in(window, async move |state, cx| {
            cx.background_executor().timer(HIDE_DELAY).await;
            let _ = state.update(cx, |state, cx| {
                if state.interaction == ThumbInteraction::Idle {
                    state.visible = false;
                    state.hide_task = None;
                    cx.notify();
                }
            });
        }));
    }

    fn set_interaction(
        &mut self,
        interaction: ThumbInteraction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.interaction == interaction {
            return;
        }
        self.interaction = interaction;
        if interaction == ThumbInteraction::Idle {
            self.schedule_hide(window, cx);
        } else {
            self.visible = true;
            self.hide_task = None;
        }
        cx.notify();
    }

    fn set_offset(&self, axis: Axis, offset: Pixels, window: &mut Window) {
        self.handle
            .set_offset(self.handle.offset().apply_along(axis, |_| offset));
        window.refresh();
    }
}

impl<T: ScrollableHandle> Render for ScrollbarState<T> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        ScrollbarElement { state: cx.entity() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ScrollPosition {
    offset: Point<Pixels>,
    max_offset: Point<Pixels>,
}

impl ScrollPosition {
    fn changed_independently_of_content(self, previous: Self, axis: Axis) -> bool {
        let offset_change = self.offset.along(axis) - previous.offset.along(axis);
        if offset_change == Pixels::ZERO {
            return false;
        }
        let content_change = self.max_offset.along(axis) - previous.max_offset.along(axis);
        offset_change != -content_change
    }
}

struct ScrollbarLayoutState {
    parent_hitbox: Hitbox,
    thumbs: Vec<ThumbLayout>,
    position: ScrollPosition,
}

impl ScrollbarLayoutState {
    fn thumb_at(&self, point: Point<Pixels>) -> Option<&ThumbLayout> {
        self.thumbs
            .iter()
            .find(|thumb| thumb.thumb_bounds.contains(&point))
    }

    fn hit_at(&self, point: Point<Pixels>) -> Option<&ThumbLayout> {
        self.thumbs.iter().find(|thumb| {
            if thumb.track_clickable {
                thumb.track_bounds.contains(&point)
            } else {
                thumb.thumb_bounds.contains(&point)
            }
        })
    }

    fn thumb_for_axis(&self, axis: Axis) -> Option<&ThumbLayout> {
        self.thumbs.iter().find(|thumb| thumb.axis == axis)
    }

    fn should_reveal(&self, previous: Option<&Self>, policy: ScrollbarRevealPolicy) -> bool {
        let Some(previous) = previous else {
            return true;
        };
        match policy {
            ScrollbarRevealPolicy::ScrollOrContentChange => {
                self.position != previous.position
                    || self
                        .thumbs
                        .iter()
                        .zip(&previous.thumbs)
                        .any(|(current, previous)| current.geometry() != previous.geometry())
                    || self.thumbs.len() != previous.thumbs.len()
            }
            ScrollbarRevealPolicy::ScrollOnly => self.thumbs.iter().any(|thumb| {
                self.position
                    .changed_independently_of_content(previous.position, thumb.axis)
            }),
        }
    }
}

struct ThumbLayout {
    axis: Axis,
    thumb_bounds: Bounds<Pixels>,
    track_bounds: Bounds<Pixels>,
    track_paint_bounds: Bounds<Pixels>,
    hitbox: Hitbox,
    track_clickable: bool,
}

impl ThumbLayout {
    fn geometry(&self) -> (Axis, Bounds<Pixels>, Bounds<Pixels>) {
        (self.axis, self.thumb_bounds, self.track_bounds)
    }

    fn offset_for_pointer(
        &self,
        point: Point<Pixels>,
        max_offset: Point<Pixels>,
        grab_offset: Pixels,
    ) -> Pixels {
        pointer_offset_to_scroll_offset(
            point.along(self.axis) - self.track_bounds.origin.along(self.axis),
            grab_offset,
            self.track_bounds.size.along(self.axis),
            self.thumb_bounds.size.along(self.axis),
            max_offset.along(self.axis),
        )
    }
}

struct ScrollbarElement<T: ScrollableHandle> {
    state: Entity<ScrollbarState<T>>,
}

impl<T: ScrollableHandle> Element for ScrollbarElement<T> {
    type RequestLayoutState = ();
    type PrepaintState = Option<ScrollbarLayoutState>;

    fn id(&self) -> Option<ElementId> {
        Some(("superiority-scrollbar", self.state.entity_id()).into())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style {
            position: Position::Absolute,
            inset: Edges::default(),
            size: size(relative(1.0), relative(1.0)).map(Into::into),
            ..Default::default()
        };
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let state = self.state.read(cx);
        if state.show_behavior == ShowBehavior::Never {
            return None;
        }

        let position = ScrollPosition {
            offset: state.handle.offset(),
            max_offset: state.handle.max_offset(),
        };
        let viewport = state.handle.viewport().size;
        let width = state.style.width();
        let padding = state.style.padding();
        let axes = state.axes;
        let track_axes = state.track_axes;
        let shown = state.shown();
        let mut thumbs = Vec::with_capacity(2);

        for axis in [Axis::Horizontal, Axis::Vertical] {
            if !axes.includes(axis) {
                continue;
            }
            let max_offset = position.max_offset.along(axis);
            let viewport_length = viewport.along(axis);
            let cross_axis_is_scrollable = axes.includes(axis.invert())
                && position.max_offset.along(axis.invert()) > Pixels::ZERO;
            let corner_space = if cross_axis_is_scrollable {
                width + padding * 2.0
            } else {
                Pixels::ZERO
            };
            let outer_length = bounds.size.along(axis) - corner_space;
            let available_length = outer_length - padding * 2.0;
            let Some((thumb_start, thumb_length)) = thumb_metrics(
                viewport_length,
                max_offset,
                position.offset.along(axis),
                available_length,
            ) else {
                continue;
            };

            let anchor = match axis {
                Axis::Horizontal => Anchor::BottomLeft,
                Axis::Vertical => Anchor::TopRight,
            };
            let track_paint_bounds = Bounds::from_anchor_and_size(
                anchor,
                bounds.corner(anchor),
                bounds
                    .size
                    .apply_along(axis, |_| outer_length)
                    .apply_along(axis.invert(), |_| width + padding * 2.0),
            );
            let track_bounds = track_paint_bounds.dilate(-padding);
            let thumb_bounds = Bounds::new(
                track_bounds
                    .origin
                    .apply_along(axis, |origin| origin + thumb_start),
                track_bounds.size.apply_along(axis, |_| thumb_length),
            );
            let track_clickable = track_axes.is_some_and(|axes| axes.includes(axis));
            let hit_bounds = if track_clickable {
                track_bounds
            } else {
                thumb_bounds
            };
            let hitbox = window.insert_hitbox(
                if shown { hit_bounds } else { Bounds::default() },
                HitboxBehavior::BlockMouseExceptScroll,
            );
            thumbs.push(ThumbLayout {
                axis,
                thumb_bounds,
                track_bounds,
                track_paint_bounds,
                hitbox,
                track_clickable,
            });
        }
        let layout = ScrollbarLayoutState {
            parent_hitbox: window.insert_hitbox(bounds, HitboxBehavior::Normal),
            thumbs,
            position,
        };
        let should_reveal = {
            let state = self.state.read(cx);
            layout.should_reveal(state.last_layout.as_ref(), state.reveal_policy)
        };
        if should_reveal {
            self.state.update(cx, |state, cx| state.reveal(window, cx));
        }
        Some(layout)
    }

    #[allow(clippy::too_many_lines)]
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(layout) = prepaint.take() else {
            return;
        };

        let shown = self.state.read(cx).shown();
        let interaction = self.state.read(cx).interaction;
        let colors = self.state.read(cx).colors;
        let style = self.state.read(cx).style;
        let phase = if matches!(interaction, ThumbInteraction::Dragging { .. }) {
            DispatchPhase::Capture
        } else {
            DispatchPhase::Bubble
        };

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if shown {
                for thumb in &layout.thumbs {
                    if let Some(track) = colors.track
                        && thumb.track_clickable
                    {
                        let border = colors.border.unwrap_or_else(Hsla::transparent_black);
                        let border_width = colors.border.map_or(Pixels::ZERO, |_| px(1.0));
                        window.paint_quad(quad(
                            thumb.track_paint_bounds,
                            Corners::default(),
                            track,
                            Edges::all(border_width),
                            border,
                            BorderStyle::Solid,
                        ));
                    }

                    let color = match interaction {
                        ThumbInteraction::Dragging { axis, .. } if axis == thumb.axis => {
                            colors.thumb_active
                        }
                        ThumbInteraction::Hover(axis) if axis == thumb.axis => colors.thumb_hover,
                        _ => colors.thumb,
                    };
                    let corners = match style {
                        ScrollbarStyle::Regular => Corners::all(Pixels::MAX)
                            .clamp_radii_for_quad_size(thumb.thumb_bounds.size),
                        ScrollbarStyle::Editor => Corners::default(),
                    };
                    window.paint_quad(quad(
                        thumb.thumb_bounds,
                        corners,
                        color,
                        Edges::default(),
                        Hsla::transparent_black(),
                        BorderStyle::Solid,
                    ));
                    if matches!(interaction, ThumbInteraction::Dragging { .. }) {
                        window.set_window_cursor_style(CursorStyle::Arrow);
                    } else {
                        window.set_cursor_style(CursorStyle::Arrow, &thumb.hitbox);
                    }
                }
            }

            self.state.update(cx, |state, _| {
                state.last_layout = Some(layout);
            });

            window.on_mouse_event({
                let state = self.state.clone();
                move |event: &MouseDownEvent, event_phase, window, cx| {
                    if event_phase != phase || event.button != MouseButton::Left {
                        return;
                    }
                    let hit = {
                        let state = state.read(cx);
                        state.last_layout.as_ref().and_then(|layout| {
                            layout.hit_at(event.position).map(|thumb| {
                                (
                                    thumb.axis,
                                    thumb.thumb_bounds,
                                    thumb.track_bounds,
                                    thumb.track_clickable,
                                )
                            })
                        })
                    };
                    let Some((axis, thumb_bounds, track_bounds, track_clickable)) = hit else {
                        return;
                    };
                    if thumb_bounds.contains(&event.position) {
                        let grab_offset =
                            event.position.along(axis) - thumb_bounds.origin.along(axis);
                        state.update(cx, |state, cx| {
                            state.handle.drag_started();
                            state.set_interaction(
                                ThumbInteraction::Dragging { axis, grab_offset },
                                window,
                                cx,
                            );
                        });
                    } else if track_clickable {
                        let offset = pointer_offset_to_scroll_offset(
                            event.position.along(axis) - track_bounds.origin.along(axis),
                            thumb_bounds.size.along(axis) / 2.0,
                            track_bounds.size.along(axis),
                            thumb_bounds.size.along(axis),
                            state.read(cx).handle.max_offset().along(axis),
                        );
                        state.update(cx, |state, _| state.set_offset(axis, offset, window));
                    }
                    cx.stop_propagation();
                }
            });

            window.on_mouse_event({
                let state = self.state.clone();
                move |event: &MouseMoveEvent, event_phase, window, cx| {
                    if event_phase != phase {
                        return;
                    }
                    let interaction = state.read(cx).interaction;
                    if let ThumbInteraction::Dragging { axis, grab_offset } = interaction
                        && event.dragging()
                    {
                        let offset = {
                            let state = state.read(cx);
                            state
                                .last_layout
                                .as_ref()
                                .and_then(|layout| layout.thumb_for_axis(axis))
                                .map(|thumb| {
                                    thumb.offset_for_pointer(
                                        event.position,
                                        state.handle.max_offset(),
                                        grab_offset,
                                    )
                                })
                        };
                        if let Some(offset) = offset {
                            state.update(cx, |state, _| state.set_offset(axis, offset, window));
                            cx.stop_propagation();
                        }
                        return;
                    }

                    let (parent_hovered, hovered_axis) = {
                        let state = state.read(cx);
                        let parent_hovered = state
                            .last_layout
                            .as_ref()
                            .is_some_and(|layout| layout.parent_hitbox.is_hovered(window));
                        let hovered_axis = state.last_layout.as_ref().and_then(|layout| {
                            layout
                                .thumb_at(event.position)
                                .filter(|thumb| thumb.hitbox.is_hovered(window))
                                .map(|thumb| thumb.axis)
                        });
                        (parent_hovered, hovered_axis)
                    };
                    state.update(cx, |state, cx| {
                        let entered = parent_hovered && !state.parent_hovered;
                        state.parent_hovered = parent_hovered;
                        if entered {
                            state.reveal(window, cx);
                        }
                        state.set_interaction(
                            hovered_axis.map_or(ThumbInteraction::Idle, ThumbInteraction::Hover),
                            window,
                            cx,
                        );
                    });
                }
            });

            window.on_mouse_event({
                let state = self.state.clone();
                move |event: &MouseUpEvent, event_phase, window, cx| {
                    if event_phase != phase || event.button != MouseButton::Left {
                        return;
                    }
                    let was_dragging = matches!(
                        state.read(cx).interaction,
                        ThumbInteraction::Dragging { .. }
                    );
                    if !was_dragging {
                        return;
                    }
                    let hovered_axis = {
                        let state = state.read(cx);
                        state
                            .last_layout
                            .as_ref()
                            .and_then(|layout| layout.thumb_at(event.position))
                            .map(|thumb| thumb.axis)
                    };
                    state.update(cx, |state, cx| {
                        state.handle.drag_ended();
                        state.set_interaction(
                            hovered_axis.map_or(ThumbInteraction::Idle, ThumbInteraction::Hover),
                            window,
                            cx,
                        );
                    });
                    cx.stop_propagation();
                }
            });
        });
    }
}

impl<T: ScrollableHandle> IntoElement for ScrollbarElement<T> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

pub trait ScrollableHandle: 'static + Any + Clone {
    fn max_offset(&self) -> Point<Pixels>;
    fn set_offset(&self, offset: Point<Pixels>);
    fn offset(&self) -> Point<Pixels>;
    fn viewport(&self) -> Bounds<Pixels>;
    fn drag_started(&self) {}
    fn drag_ended(&self) {}

    fn content_size(&self) -> Size<Pixels> {
        self.viewport().size + self.max_offset().into()
    }
}

impl ScrollableHandle for ScrollHandle {
    fn max_offset(&self) -> Point<Pixels> {
        self.max_offset()
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        self.set_offset(offset);
    }

    fn offset(&self) -> Point<Pixels> {
        self.offset()
    }

    fn viewport(&self) -> Bounds<Pixels> {
        self.bounds()
    }
}

impl ScrollableHandle for UniformListScrollHandle {
    fn max_offset(&self) -> Point<Pixels> {
        self.0.borrow().base_handle.max_offset()
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        self.0.borrow().base_handle.set_offset(offset);
    }

    fn offset(&self) -> Point<Pixels> {
        self.0.borrow().base_handle.offset()
    }

    fn viewport(&self) -> Bounds<Pixels> {
        self.0.borrow().base_handle.bounds()
    }
}

impl ScrollableHandle for ListState {
    fn max_offset(&self) -> Point<Pixels> {
        self.max_offset_for_scrollbar()
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        self.set_offset_from_scrollbar(offset);
    }

    fn offset(&self) -> Point<Pixels> {
        self.scroll_px_offset_for_scrollbar()
    }

    fn viewport(&self) -> Bounds<Pixels> {
        self.viewport_bounds()
    }

    fn drag_started(&self) {
        self.scrollbar_drag_started();
    }

    fn drag_ended(&self) {
        self.scrollbar_drag_ended();
    }
}

fn thumb_metrics(
    viewport_length: Pixels,
    max_offset: Pixels,
    current_offset: Pixels,
    track_length: Pixels,
) -> Option<(Pixels, Pixels)> {
    if viewport_length <= Pixels::ZERO || max_offset <= Pixels::ZERO || track_length <= Pixels::ZERO
    {
        return None;
    }
    let content_length = viewport_length + max_offset;
    let thumb_length = (track_length * (viewport_length / content_length))
        .max(MINIMUM_THUMB_LENGTH)
        .min(track_length);
    if thumb_length >= track_length {
        return None;
    }
    let progress = ((-current_offset).clamp(Pixels::ZERO, max_offset)) / max_offset;
    Some(((track_length - thumb_length) * progress, thumb_length))
}

fn pointer_offset_to_scroll_offset(
    pointer_offset: Pixels,
    grab_offset: Pixels,
    track_length: Pixels,
    thumb_length: Pixels,
    max_offset: Pixels,
) -> Pixels {
    let travel = track_length - thumb_length;
    if travel <= Pixels::ZERO || max_offset <= Pixels::ZERO {
        return Pixels::ZERO;
    }
    let thumb_start = (pointer_offset - grab_offset).clamp(Pixels::ZERO, travel);
    -max_offset * (thumb_start / travel)
}

#[cfg(test)]
mod tests {
    use gpui::{Axis, point, px};

    use super::{
        ScrollPosition, ScrollbarRevealPolicy, pointer_offset_to_scroll_offset, thumb_metrics,
    };

    #[test]
    fn thumb_tracks_the_scroll_offset() {
        let (start, length) = thumb_metrics(px(100.0), px(300.0), px(-150.0), px(100.0))
            .expect("scrollable content should have a thumb");
        assert_eq!(length, px(25.0));
        assert_eq!(start, px(37.5));
    }

    #[test]
    fn dragging_maps_the_track_to_the_full_scroll_range() {
        assert_eq!(
            pointer_offset_to_scroll_offset(px(87.5), px(12.5), px(100.0), px(25.0), px(300.0),),
            px(-300.0)
        );
    }

    #[test]
    fn content_growth_at_the_bottom_is_not_user_scrolling() {
        let previous = ScrollPosition {
            offset: point(px(0.0), px(-300.0)),
            max_offset: point(px(0.0), px(300.0)),
        };
        let current = ScrollPosition {
            offset: point(px(0.0), px(-340.0)),
            max_offset: point(px(0.0), px(340.0)),
        };
        assert!(!current.changed_independently_of_content(previous, Axis::Vertical));
    }

    #[test]
    fn scroll_only_policy_is_distinct_from_content_change_policy() {
        assert_ne!(
            ScrollbarRevealPolicy::ScrollOnly,
            ScrollbarRevealPolicy::ScrollOrContentChange
        );
    }
}
