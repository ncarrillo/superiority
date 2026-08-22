//! Product-neutral member-list filtering, placement, and transition mechanics.

use std::{cmp::Ordering, collections::HashSet, hash::Hash, ops::Range, time::Duration};

use gpui::{AnyElement, Div, IntoElement, div, ease_in_out, prelude::*, px};

use crate::foundation::animation::AnimationClock;

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
    slot_height: f32,
    opacity: f32,
    x: f32,
    y: f32,
}

pub struct Transition<T, H = u32> {
    previous: Vec<T>,
    inserted: HashSet<H>,
    removed: HashSet<H>,
    full_reveal: bool,
}

pub struct TimedTransition<S, T, C, H = u32> {
    pub scope: S,
    pub transition: Transition<T, H>,
    pub started: C,
}

pub struct PlacedRows<T> {
    pub rows: Vec<(T, RowMotion, f32)>,
    pub reveal_opacity: f32,
}

impl<S, T: Clone, C: AnimationClock, H: Copy + Eq + Hash> TimedTransition<S, T, C, H> {
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

impl<T: Clone, H: Copy + Eq + Hash> Transition<T, H> {
    #[must_use]
    pub fn new(previous: Vec<T>, next: &[T], handle: impl Fn(&T) -> H) -> Option<Self> {
        let previous_handles = previous.iter().map(&handle).collect::<Vec<_>>();
        let next_handles = next.iter().map(&handle).collect::<Vec<_>>();
        if previous_handles == next_handles {
            return None;
        }
        let Some((removed, inserted)) = roster_diff(&previous_handles, &next_handles) else {
            return Some(Self::full_reveal());
        };
        if removed.len() + inserted.len() > MAX_ANIMATED_CHANGES {
            return Some(Self::full_reveal());
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

    fn full_reveal() -> Self {
        Self {
            previous: Vec::new(),
            inserted: HashSet::new(),
            removed: HashSet::new(),
            full_reveal: true,
        }
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
    pub fn rows(&self, next: &[T], handle: impl Fn(&T) -> H) -> Vec<(T, RowMotion)> {
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
pub fn placed_rows<S, T: Clone, C: AnimationClock, H: Copy + Eq + Hash>(
    items: Vec<T>,
    animation: Option<&TimedTransition<S, T, C, H>>,
    now: C,
    handle: impl Fn(&T) -> H,
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
pub fn animated_rows<S, T: Clone, C: AnimationClock, H: Copy + Eq + Hash>(
    items: Vec<T>,
    animation: Option<&TimedTransition<S, T, C, H>>,
    now: C,
    handle: impl Fn(&T) -> H,
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
            animated_row_slot(
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
pub fn virtual_row_slot(row: impl IntoElement, row_height: f32, row_gap: f32) -> Div {
    div()
        .relative()
        .w_full()
        .h(px(row_height + row_gap))
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

/// Shared member-list ordering inside a relationship band: people who are
/// present read first, absent people collect at the bottom, and both groups
/// remain alphabetical. Product surfaces decide which of their protocol
/// presence states count as absent.
#[must_use]
pub fn present_before_absent(
    left_absent: bool,
    left_name: &str,
    right_absent: bool,
    right_name: &str,
) -> Ordering {
    left_absent.cmp(&right_absent).then_with(|| {
        left_name
            .to_lowercase()
            .cmp(&right_name.to_lowercase())
            .then_with(|| left_name.cmp(right_name))
    })
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

fn roster_diff<H: Copy + Eq + Hash>(
    previous: &[H],
    next: &[H],
) -> Option<(Vec<usize>, Vec<usize>)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorder_falls_back_to_a_full_reveal() {
        let transition = Transition::new(vec![1, 2, 3], &[2, 1, 3], |value| *value)
            .expect("a changed list has a transition");
        assert!(transition.is_full_reveal());
    }

    #[test]
    fn filtering_is_case_insensitive_and_range_aware() {
        let values = vec!["Alpha", "beta", "alphabet"];
        assert_eq!(
            filtered_range(&values, "ALP", 1..2, |value, filter| {
                filter_matches(value, filter)
            }),
            vec!["alphabet"]
        );
    }

    #[test]
    fn absent_members_sink_below_present_members_without_losing_name_order() {
        let mut members = vec![("Zulu", true), ("beta", false), ("Alpha", false)];
        members.sort_by(|left, right| present_before_absent(left.1, left.0, right.1, right.0));
        assert_eq!(
            members,
            vec![("Alpha", false), ("beta", false), ("Zulu", true)]
        );
    }

    #[test]
    fn a_membership_delta_keeps_departures_long_enough_to_animate_them_out() {
        let transition =
            Transition::new(vec![1, 2], &[2, 3], |value| *value).expect("membership changed");
        assert_eq!(
            transition.rows(&[2, 3], |value| *value),
            vec![
                (1, RowMotion::Removed),
                (2, RowMotion::Stable),
                (3, RowMotion::Inserted),
            ]
        );
    }
}
