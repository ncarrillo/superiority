//! Product-neutral tab pointer, reorder, and marquee state.

use std::{collections::HashMap, hash::Hash, time::Duration};

use crate::foundation::animation::{
    AnimationClock, OffsetAnimation, ScalarAnimation, animation_progress, interpolated_offsets,
};

const TAB_DRAG_SLOP: f32 = 4.0;
const TAB_NAME_SPEED: f32 = 55.0;
const TAB_REORDER_DURATION: Duration = Duration::from_millis(160);

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
    fn offsets(&self, now: C) -> Vec<f32> {
        let Some(shift) = &self.shift else {
            return vec![0.0; self.widths.len()];
        };
        let progress = animation_progress(now.elapsed(shift.started), TAB_REORDER_DURATION);
        interpolated_offsets(&shift.from, &shift.to, progress)
    }

    fn shift_is_running(&self, now: C) -> bool {
        self.shift
            .as_ref()
            .is_some_and(|shift| now.elapsed(shift.started) < TAB_REORDER_DURATION)
    }
}

#[must_use]
pub fn marquee_duration(distance: f32) -> Duration {
    Duration::from_secs_f32((distance.abs() / TAB_NAME_SPEED).max(0.12))
}

#[must_use]
pub fn tab_origins(widths: &[f32], start: f32) -> Vec<f32> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_distinguishes_clicks_and_reorders() {
        let mut state = TabStripState::<String, f64>::default();
        assert!(state.begin_pointer(1, 150.0, 3));
        assert_eq!(state.finish(1), TabRelease::Click(1));

        assert!(state.begin_pointer(0, 50.0, 3));
        assert!(state.update_drag(0, 280.0, &[100.0, 100.0, 100.0], 0.0));
        assert_eq!(state.finish(0), TabRelease::Reorder { from: 0, to: 2 });
    }

    #[test]
    fn marquee_runs_in_the_browser_clock_domain() {
        let mut state = TabStripState::<String, f64>::default();
        let key = "long-tab".to_owned();
        assert!(state.set_name_hover(key.clone(), true, 55.0, 0.0));
        assert!((state.name_offset(&key, 500.0) + 27.5).abs() < f32::EPSILON);
        assert!(!state.name_animation_is_running(1_000.0));
    }
}
