//! product-neutral channel transition, roster state, and responsive layout.

use std::{
    borrow::Borrow,
    collections::HashMap,
    hash::Hash,
    ops::{Deref, DerefMut},
    time::Duration,
};

use gpui::UniformListScrollHandle;

use crate::{
    foundation::animation::AnimationClock,
    patterns::roster::{TimedTransition, Transition},
};

const CHANNEL_CROSSFADE_DURATION: Duration = Duration::from_millis(320);

#[derive(Clone, Copy, Debug)]
struct ChannelCrossfade<C> {
    started: Option<C>,
}

impl<C> ChannelCrossfade<C> {
    const fn pending() -> Self {
        Self { started: None }
    }

    const fn started(now: C) -> Self {
        Self { started: Some(now) }
    }

    fn start(&mut self, now: C) {
        self.started.get_or_insert(now);
    }

    const fn is_pending(&self) -> bool {
        self.started.is_none()
    }
}

impl<C: AnimationClock> ChannelCrossfade<C> {
    fn progress(&self, now: C) -> Option<f32> {
        self.started
            .map(|started| channel_crossfade_progress(now.elapsed(started)))
    }

    fn is_running(&self, now: C) -> bool {
        self.started
            .is_some_and(|started| now.elapsed(started) < CHANNEL_CROSSFADE_DURATION)
    }

    fn is_complete(&self, now: C) -> bool {
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

pub struct RosterState<K, T, C, H = u32> {
    pub scroll: UniformListScrollHandle,
    pub focused: bool,
    pub animation: Option<TimedTransition<K, T, C, H>>,
    pub selections: HashMap<K, H>,
}

impl<K, T, C, H> Default for RosterState<K, T, C, H> {
    fn default() -> Self {
        Self {
            scroll: UniformListScrollHandle::new(),
            focused: false,
            animation: None,
            selections: HashMap::new(),
        }
    }
}

impl<K, T, C, H> RosterState<K, T, C, H>
where
    K: Clone + Eq + Hash,
    T: Clone,
    C: AnimationClock,
    H: Copy + Eq + Hash,
{
    #[must_use]
    pub fn selection<Q>(&self, scope: &Q) -> Option<H>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.selections.get(scope).copied()
    }

    pub fn set_selection(&mut self, scope: K, selection: Option<H>) {
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
        handle: impl Fn(&T) -> H,
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

    /// `positions` maps each selectable handle to the row it occupies in the
    /// rendered list, including any non-selectable section headers.
    #[must_use]
    pub fn select_index(
        &mut self,
        scope: K,
        handles: &[H],
        positions: &[usize],
        index: usize,
        strategy: gpui::ScrollStrategy,
    ) -> Option<H> {
        let selected = handles.get(index).copied()?;
        self.selections.insert(scope, selected);
        self.scroll
            .scroll_to_item(positions.get(index).copied().unwrap_or(index), strategy);
        Some(selected)
    }

    #[must_use]
    pub fn move_selection(
        &mut self,
        scope: K,
        handles: &[H],
        positions: &[usize],
        delta: isize,
        strategy: gpui::ScrollStrategy,
    ) -> Option<H> {
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
        self.select_index(scope, handles, positions, next, strategy)
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
    pub const fn wide(margin: f32, roster_width: f32) -> Self {
        Self {
            gap: margin,
            margin,
            top_padding: 19.0,
            stacked: false,
            roster_width: Some(roster_width),
            roster_height: None,
        }
    }

    #[must_use]
    pub fn responsive(width: f32, height: f32, wide: Self) -> Self {
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
        wide
    }
}

fn channel_crossfade_progress(elapsed: Duration) -> f32 {
    gpui::ease_in_out(
        (elapsed.as_secs_f32() / CHANNEL_CROSSFADE_DURATION.as_secs_f32()).clamp(0.0, 1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_transition_starts_when_incoming_data_is_ready() {
        let mut transition = ChannelTransition::<_, f64>::pending("general");
        assert!(transition.is_pending());
        assert_eq!(transition.progress(100.0), None);
        transition.start(100.0);
        assert!(!transition.is_pending());
        assert_eq!(*transition, "general");
        assert!(transition.is_running(200.0));
        assert!(transition.is_complete(500.0));
    }

    #[test]
    fn roster_selection_is_scoped() {
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
    fn roster_selection_accepts_product_native_wide_handles() {
        let mut roster = RosterState::<String, (), f64, u64>::default();
        let handle = u64::from(u32::MAX) + 7;
        roster.set_selection("wc3-general".to_owned(), Some(handle));
        assert_eq!(roster.selection("wc3-general"), Some(handle));
    }

    #[test]
    fn narrow_layout_stacks_the_roster() {
        let layout =
            ChannelChromeLayout::responsive(390.0, 844.0, ChannelChromeLayout::wide(22.0, 300.0));
        assert!(layout.stacked);
        assert_eq!(layout.roster_width, None);
        assert!(layout.roster_height.is_some());
    }
}
