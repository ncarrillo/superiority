use std::{
    borrow::Borrow,
    collections::HashMap,
    hash::Hash,
    ops::{Deref, DerefMut},
    time::Duration,
};

use gpui::{
    AnyElement, App, Div, ImageSource, IntoElement, RenderOnce, ScrollHandle,
    UniformListScrollHandle, Window, div, ease_in_out, prelude::*, px,
};

use crate::{
    animation::AnimationClock,
    components::{
        chat::{self, TranscriptSelection},
        navigation::TabStripState,
        roster::{self, TimedTransition, Transition},
        shell,
    },
    theme::MARGIN,
};

const CHANNEL_CROSSFADE_DURATION: Duration = Duration::from_millis(320);

#[derive(Clone, Copy, Debug)]
struct ChannelCrossfade<C> {
    started: Option<C>,
}

impl<C> ChannelCrossfade<C> {
    #[must_use]
    pub const fn pending() -> Self {
        Self { started: None }
    }

    #[must_use]
    pub const fn started(now: C) -> Self {
        Self { started: Some(now) }
    }

    pub fn start(&mut self, now: C) {
        self.started.get_or_insert(now);
    }

    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.started.is_none()
    }
}

impl<C: AnimationClock> ChannelCrossfade<C> {
    #[must_use]
    pub fn progress(&self, now: C) -> Option<f32> {
        self.started
            .map(|started| channel_crossfade_progress(now.elapsed(started)))
    }

    #[must_use]
    pub fn is_running(&self, now: C) -> bool {
        self.started
            .is_some_and(|started| now.elapsed(started) < CHANNEL_CROSSFADE_DURATION)
    }

    #[must_use]
    pub fn is_complete(&self, now: C) -> bool {
        self.started
            .is_some_and(|started| now.elapsed(started) >= CHANNEL_CROSSFADE_DURATION)
    }
}

pub struct ChannelTransition<S, C> {
    snapshot: S,
    crossfade: ChannelCrossfade<C>,
}

impl<S, C> ChannelTransition<S, C> {
    #[must_use]
    pub const fn pending(snapshot: S) -> Self {
        Self {
            snapshot,
            crossfade: ChannelCrossfade::pending(),
        }
    }

    #[must_use]
    pub const fn started(snapshot: S, now: C) -> Self {
        Self {
            snapshot,
            crossfade: ChannelCrossfade::started(now),
        }
    }

    pub fn start(&mut self, now: C) {
        self.crossfade.start(now);
    }

    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.crossfade.is_pending()
    }
}

impl<S, C: AnimationClock> ChannelTransition<S, C> {
    #[must_use]
    pub fn progress(&self, now: C) -> Option<f32> {
        self.crossfade.progress(now)
    }

    #[must_use]
    pub fn is_running(&self, now: C) -> bool {
        self.crossfade.is_running(now)
    }

    #[must_use]
    pub fn is_complete(&self, now: C) -> bool {
        self.crossfade.is_complete(now)
    }
}

impl<S, C> Deref for ChannelTransition<S, C> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

impl<S, C> DerefMut for ChannelTransition<S, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.snapshot
    }
}

pub struct NavigationState<K, C> {
    pub tabs: TabStripState<K, C>,
    pub scroll: ScrollHandle,
}

impl<K, C> Default for NavigationState<K, C> {
    fn default() -> Self {
        Self {
            tabs: TabStripState::default(),
            scroll: ScrollHandle::new(),
        }
    }
}

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

pub struct RosterState<K, T, C> {
    pub scroll: UniformListScrollHandle,
    pub focused: bool,
    pub animation: Option<TimedTransition<K, T, C>>,
    pub selections: HashMap<K, u32>,
}

impl<K, T, C> Default for RosterState<K, T, C> {
    fn default() -> Self {
        Self {
            scroll: UniformListScrollHandle::new(),
            focused: false,
            animation: None,
            selections: HashMap::new(),
        }
    }
}

impl<K, T, C> RosterState<K, T, C>
where
    K: Clone + Eq + Hash,
    T: Clone,
    C: AnimationClock,
{
    #[must_use]
    pub fn selection<Q>(&self, scope: &Q) -> Option<u32>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.selections.get(scope).copied()
    }

    pub fn set_selection(&mut self, scope: K, selection: Option<u32>) {
        if let Some(selection) = selection {
            self.selections.insert(scope, selection);
        } else {
            self.selections.remove(&scope);
        }
    }

    pub fn begin_transition(
        &mut self,
        scope: K,
        previous: Vec<T>,
        next: &[T],
        now: C,
        handle: impl Fn(&T) -> u32,
    ) -> bool {
        let Some(transition) = Transition::new(previous, next, handle) else {
            return false;
        };
        self.animation = Some(TimedTransition {
            scope,
            transition,
            started: now,
        });
        true
    }

    pub fn finish_transition(&mut self, now: C) -> bool {
        if self
            .animation
            .as_ref()
            .is_some_and(|animation| !animation.is_running(now))
        {
            self.animation = None;
            return true;
        }
        false
    }

    pub fn clear_interaction(&mut self) {
        self.focused = false;
        self.animation = None;
    }

    pub fn retain_selections(&mut self, mut retain: impl FnMut(&K) -> bool) {
        self.selections.retain(|scope, _| retain(scope));
    }

    #[must_use]
    pub fn select_index(
        &mut self,
        scope: K,
        handles: &[u32],
        index: usize,
        strategy: gpui::ScrollStrategy,
    ) -> Option<u32> {
        let selected = handles.get(index).copied()?;
        self.selections.insert(scope, selected);
        self.scroll.scroll_to_item(index, strategy);
        Some(selected)
    }

    #[must_use]
    pub fn move_selection(
        &mut self,
        scope: K,
        handles: &[u32],
        delta: isize,
        strategy: gpui::ScrollStrategy,
    ) -> Option<u32> {
        if handles.is_empty() {
            return None;
        }
        let current = self
            .selections
            .get(&scope)
            .and_then(|selected| handles.iter().position(|handle| handle == selected));
        let next = match current {
            Some(current) => current
                .saturating_add_signed(delta)
                .min(handles.len().saturating_sub(1)),
            None if delta < 0 => handles.len() - 1,
            None => 0,
        };
        self.select_index(scope, handles, next, strategy)
    }
}

pub struct WorkspaceState<K, T, C> {
    pub navigation: NavigationState<K, C>,
    pub transcript: TranscriptState,
    pub roster: RosterState<K, T, C>,
}

impl<K, T, C> Default for WorkspaceState<K, T, C> {
    fn default() -> Self {
        Self {
            navigation: NavigationState::default(),
            transcript: TranscriptState::default(),
            roster: RosterState::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelChromeLayout {
    pub gap: f32,
    pub margin: f32,
    pub top_padding: f32,
    pub stacked: bool,
    pub roster_width: Option<f32>,
    pub roster_height: Option<f32>,
}

impl ChannelChromeLayout {
    #[must_use]
    pub fn for_viewport(width: f32, height: f32) -> Self {
        if width < 720.0 {
            return Self {
                gap: 12.0,
                margin: 12.0,
                top_padding: 12.0,
                stacked: true,
                roster_width: None,
                roster_height: Some((height * 0.38).clamp(190.0, 320.0)),
            };
        }
        if width < 1_024.0 {
            return Self {
                gap: 16.0,
                margin: 16.0,
                top_padding: 14.0,
                stacked: false,
                roster_width: Some(260.0),
                roster_height: None,
            };
        }
        Self::default()
    }
}

impl Default for ChannelChromeLayout {
    fn default() -> Self {
        Self {
            gap: MARGIN,
            margin: MARGIN,
            top_padding: 19.0,
            stacked: false,
            roster_width: Some(crate::theme::ROSTER_WIDTH),
            roster_height: None,
        }
    }
}

#[derive(IntoElement)]
pub struct ChannelWorkspace {
    navigation: AnyElement,
    chat: AnyElement,
    roster: AnyElement,
    footer: Option<AnyElement>,
    layout: ChannelChromeLayout,
}

impl ChannelWorkspace {
    #[must_use]
    pub fn new(
        navigation: impl IntoElement,
        chat: impl IntoElement,
        roster: impl IntoElement,
    ) -> Self {
        Self {
            navigation: navigation.into_any_element(),
            chat: chat.into_any_element(),
            roster: roster.into_any_element(),
            footer: None,
            layout: ChannelChromeLayout::default(),
        }
    }

    #[must_use]
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    #[must_use]
    pub const fn layout(mut self, layout: ChannelChromeLayout) -> Self {
        self.layout = layout;
        self
    }
}

impl RenderOnce for ChannelWorkspace {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        channel_chrome_with_layout(
            self.navigation,
            self.chat,
            self.roster,
            self.footer,
            self.layout,
        )
    }
}

#[derive(IntoElement)]
pub struct ChannelRoster {
    current_header: AnyElement,
    current_rows: AnyElement,
    outgoing_header: Option<AnyElement>,
    outgoing_rows: Option<AnyElement>,
    progress: Option<f32>,
    width: Option<f32>,
    overlays: Vec<AnyElement>,
    focused: bool,
}

impl ChannelRoster {
    #[must_use]
    pub fn new(current_header: impl IntoElement, current_rows: impl IntoElement) -> Self {
        Self {
            current_header: current_header.into_any_element(),
            current_rows: current_rows.into_any_element(),
            outgoing_header: None,
            outgoing_rows: None,
            progress: None,
            width: Some(crate::theme::ROSTER_WIDTH),
            overlays: Vec::new(),
            focused: false,
        }
    }

    #[must_use]
    pub fn outgoing(
        mut self,
        header: impl IntoElement,
        rows: impl IntoElement,
        progress: Option<f32>,
    ) -> Self {
        self.outgoing_header = Some(header.into_any_element());
        self.outgoing_rows = Some(rows.into_any_element());
        self.progress = progress;
        self
    }

    #[must_use]
    pub const fn width(mut self, width: Option<f32>) -> Self {
        self.width = width;
        self
    }

    #[must_use]
    pub fn overlay(mut self, overlay: impl IntoElement) -> Self {
        self.overlays.push(overlay.into_any_element());
        self
    }

    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl RenderOnce for ChannelRoster {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let header = roster::header_layer().child(transition_layers(
            self.current_header,
            self.outgoing_header,
            self.progress,
        ));
        let rows = roster::rows().child(transition_layers(
            self.current_rows,
            self.outgoing_rows,
            self.progress,
        ));
        let mut panel = roster::RosterPanel::new(header, rows)
            .width(self.width)
            .focused(self.focused);
        for overlay in self.overlays {
            panel = panel.overlay(overlay);
        }
        panel
    }
}

#[derive(IntoElement)]
pub struct ChannelChat {
    background: ImageSource,
    current: AnyElement,
    outgoing: Option<AnyElement>,
    progress: Option<f32>,
    overlays: Vec<AnyElement>,
}

impl ChannelChat {
    #[must_use]
    pub fn new(background: impl Into<ImageSource>, current: impl IntoElement) -> Self {
        Self {
            background: background.into(),
            current: current.into_any_element(),
            outgoing: None,
            progress: None,
            overlays: Vec::new(),
        }
    }

    #[must_use]
    pub fn outgoing(mut self, outgoing: impl IntoElement, progress: Option<f32>) -> Self {
        self.outgoing = Some(outgoing.into_any_element());
        self.progress = progress;
        self
    }

    #[must_use]
    pub fn overlay(mut self, overlay: impl IntoElement) -> Self {
        self.overlays.push(overlay.into_any_element());
        self
    }
}

impl RenderOnce for ChannelChat {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut panel = chat::ChatPanel::new(self.background).child(transition_layers(
            self.current,
            self.outgoing,
            self.progress,
        ));
        for overlay in self.overlays {
            panel = panel.child(overlay);
        }
        panel
    }
}

#[must_use]
fn channel_crossfade_progress(elapsed: Duration) -> f32 {
    ease_in_out((elapsed.as_secs_f32() / CHANNEL_CROSSFADE_DURATION.as_secs_f32()).clamp(0.0, 1.0))
}

#[must_use]
pub fn root() -> Div {
    shell::root().relative()
}

#[must_use]
fn crossfade_layers(incoming: impl IntoElement, outgoing: impl IntoElement, progress: f32) -> Div {
    let progress = normalized_crossfade_progress(progress);
    div()
        .absolute()
        .inset_0()
        .child(div().absolute().inset_0().opacity(progress).child(incoming))
        .child(
            div()
                .absolute()
                .inset_0()
                .opacity(1.0 - progress)
                .child(outgoing),
        )
}

#[must_use]
fn transition_layers(
    current: impl IntoElement,
    outgoing: Option<AnyElement>,
    progress: Option<f32>,
) -> AnyElement {
    let current = current.into_any_element();
    let Some(outgoing) = outgoing else {
        return current;
    };
    match progress {
        None => outgoing,
        Some(progress) if progress < 1.0 => {
            crossfade_layers(current, outgoing, progress).into_any_element()
        }
        Some(_) => current,
    }
}

fn normalized_crossfade_progress(progress: f32) -> f32 {
    progress.clamp(0.0, 1.0)
}

fn channel_chrome_with_layout(
    navigation: impl IntoElement,
    chat: impl IntoElement,
    roster: impl IntoElement,
    footer: Option<AnyElement>,
    layout: ChannelChromeLayout,
) -> Div {
    let mut panels = div()
        .flex()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .gap(px(layout.gap));
    if layout.stacked {
        panels = panels.flex_col().child(chat).child(
            div()
                .relative()
                .w_full()
                .h(px(layout.roster_height.unwrap_or(240.0)))
                .flex_shrink_0()
                .child(roster),
        );
    } else {
        panels = panels.child(chat).child(roster);
    }

    div()
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .child(navigation)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .gap(px(layout.gap))
                .pt(px(layout.top_padding))
                .px(px(layout.margin))
                .pb(px(layout.margin))
                .child(panels)
                .children(footer),
        )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        CHANNEL_CROSSFADE_DURATION, ChannelChromeLayout, ChannelCrossfade, ChannelTransition,
        RosterState, channel_crossfade_progress, normalized_crossfade_progress,
    };

    #[test]
    fn crossfade_progress_is_bounded() {
        assert!(normalized_crossfade_progress(-1.0).abs() < f32::EPSILON);
        assert!((normalized_crossfade_progress(0.35) - 0.35).abs() < f32::EPSILON);
        assert!((normalized_crossfade_progress(2.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn channel_crossfade_uses_shared_duration() {
        assert!(channel_crossfade_progress(Duration::ZERO).abs() < f32::EPSILON);
        assert!(
            (channel_crossfade_progress(CHANNEL_CROSSFADE_DURATION) - 1.0).abs() < f32::EPSILON
        );
        assert!(
            (channel_crossfade_progress(CHANNEL_CROSSFADE_DURATION + Duration::from_millis(1))
                - 1.0)
                .abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn channel_crossfade_can_be_armed_before_the_incoming_data_arrives() {
        let mut crossfade = ChannelCrossfade::<f64>::pending();
        assert!(crossfade.is_pending());
        assert_eq!(crossfade.progress(100.0), None);
        crossfade.start(100.0);
        assert!(!crossfade.is_pending());
        assert!(crossfade.is_running(200.0));
        assert!(crossfade.is_complete(500.0));
    }

    #[test]
    fn pending_channel_transition_keeps_the_outgoing_layer_visible() {
        let mut transition = ChannelTransition::<_, f64>::pending("general");
        assert!(transition.is_pending());
        assert_eq!(transition.progress(100.0), None);
        transition.start(100.0);
        assert!(!transition.is_pending());
        assert_eq!(*transition, "general");
    }

    #[test]
    fn roster_selection_is_scoped_to_each_channel() {
        let mut roster = RosterState::<String, (), f64>::default();
        roster.set_selection("general".to_owned(), Some(7));
        roster.set_selection("arcade".to_owned(), Some(11));
        assert_eq!(roster.selection("general"), Some(7));
        assert_eq!(roster.selection("arcade"), Some(11));
        roster.set_selection("general".to_owned(), None);
        assert_eq!(roster.selection("general"), None);
        assert_eq!(roster.selection("arcade"), Some(11));
    }

    #[test]
    fn channel_layout_stacks_on_phone_sized_viewports() {
        let layout = ChannelChromeLayout::for_viewport(390.0, 844.0);
        assert!(layout.stacked);
        assert_eq!(layout.roster_width, None);
        assert_eq!(layout.roster_height, Some(320.0));
        assert_eq!(layout.margin, 12.0);
    }

    #[test]
    fn channel_layout_uses_a_narrower_tablet_roster() {
        let layout = ChannelChromeLayout::for_viewport(820.0, 1_180.0);
        assert!(!layout.stacked);
        assert_eq!(layout.roster_width, Some(260.0));
        assert_eq!(layout.margin, 16.0);
    }
}
