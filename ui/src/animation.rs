use std::time::{Duration, Instant};

use gpui::ease_in_out;

pub trait AnimationClock: Copy {
    fn elapsed(self, started: Self) -> Duration;
}

impl AnimationClock for Instant {
    fn elapsed(self, started: Self) -> Duration {
        self.saturating_duration_since(started)
    }
}

impl AnimationClock for f64 {
    fn elapsed(self, started: Self) -> Duration {
        let milliseconds = self - started;
        if !milliseconds.is_finite() || milliseconds <= 0.0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(milliseconds / 1_000.0)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScalarAnimation<C> {
    pub from: f32,
    pub to: f32,
    pub started: C,
    pub duration: Duration,
}

impl<C: AnimationClock> ScalarAnimation<C> {
    #[must_use]
    pub fn value(&self, now: C) -> f32 {
        interpolated_value(
            self.from,
            self.to,
            animation_progress(now.elapsed(self.started), self.duration),
        )
    }

    #[must_use]
    pub fn is_running(&self, now: C) -> bool {
        now.elapsed(self.started) < self.duration
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OffsetAnimation<C> {
    pub from: Vec<f32>,
    pub to: Vec<f32>,
    pub started: C,
}

#[must_use]
pub(crate) fn animation_progress(elapsed: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }
    ease_in_out((elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0))
}

#[must_use]
fn interpolated_value(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

#[must_use]
pub(crate) fn interpolated_offsets(from: &[f32], to: &[f32], progress: f32) -> Vec<f32> {
    from.iter()
        .zip(to)
        .map(|(from, to)| interpolated_value(*from, *to, progress))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{AnimationClock, ScalarAnimation};
    use std::time::Duration;

    #[test]
    fn browser_clock_rejects_invalid_or_reversed_time() {
        assert_eq!(100.0_f64.elapsed(200.0), Duration::ZERO);
        assert_eq!(f64::NAN.elapsed(200.0), Duration::ZERO);
        assert_eq!(1_250.0_f64.elapsed(250.0), Duration::from_secs(1));
    }

    #[test]
    fn scalar_animation_is_clock_agnostic() {
        let animation = ScalarAnimation {
            from: 0.0,
            to: 10.0,
            started: 100.0_f64,
            duration: Duration::from_secs(1),
        };
        assert!((animation.value(600.0) - 5.0).abs() < f32::EPSILON);
        assert!(!animation.is_running(1_100.0));
    }
}
