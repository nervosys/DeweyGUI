//! Animation system for Dewey.
//!
//! Easing functions, tweens, spring physics and timelines, for an application
//! to drive.
//!
//! **Nothing in this crate drives it.** No host advances an animation: an
//! application holds its own `Tween` and calls [`Animation::tick`] from
//! `update`, and if it does not, nothing moves.
//!
//! There is a gap in the way of doing that well. [`Animation::tick`] takes a
//! delta, and `Event::Tick` — the event the runtime schedules, whose own
//! documentation says it is "for animations" — carries none, so an application
//! has to keep an `Instant` of its own to work out how much time passed. The
//! event cannot gain a field without breaking every `match` on `Event`, so it
//! is recorded here and in ROADMAP.md rather than quietly changed.

use std::time::{Duration, Instant};

/// An animation that interpolates a value over time.
pub trait Animation {
    /// Advance the animation by the given delta time.
    fn tick(&mut self, dt: Duration);

    /// The current interpolated value in `[0.0, 1.0]`.
    fn value(&self) -> f32;

    /// Whether the animation has completed.
    fn is_finished(&self) -> bool;

    /// Reset to the initial state.
    fn reset(&mut self);
}

/// Standard easing functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,
    EaseInQuint,
    EaseOutQuint,
    EaseInOutQuint,
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,
    EaseInCirc,
    EaseOutCirc,
    EaseInOutCirc,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
    EaseInElastic,
    EaseOutElastic,
    EaseInOutElastic,
    EaseInBounce,
    EaseOutBounce,
    EaseInOutBounce,
}

impl Easing {
    /// Apply this easing function to a progress value `t` in `[0.0, 1.0]`.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn | Self::EaseInCubic => t * t * t,
            Self::EaseOut | Self::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Self::EaseInOut | Self::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Self::EaseInQuad => t * t,
            Self::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Self::EaseInQuart => t * t * t * t,
            Self::EaseOutQuart => 1.0 - (1.0 - t).powi(4),
            Self::EaseInOutQuart => {
                if t < 0.5 {
                    8.0 * t * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
                }
            }
            Self::EaseInQuint => t * t * t * t * t,
            Self::EaseOutQuint => 1.0 - (1.0 - t).powi(5),
            Self::EaseInOutQuint => {
                if t < 0.5 {
                    16.0 * t * t * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(5) / 2.0
                }
            }
            Self::EaseInSine => 1.0 - (t * std::f32::consts::FRAC_PI_2).cos(),
            Self::EaseOutSine => (t * std::f32::consts::FRAC_PI_2).sin(),
            Self::EaseInOutSine => -(((t * std::f32::consts::PI).cos() - 1.0) / 2.0),
            Self::EaseInExpo => {
                if t == 0.0 {
                    0.0
                } else {
                    (2.0f32).powf(10.0 * t - 10.0)
                }
            }
            Self::EaseOutExpo => {
                if t == 1.0 {
                    1.0
                } else {
                    1.0 - (2.0f32).powf(-10.0 * t)
                }
            }
            Self::EaseInOutExpo => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else if t < 0.5 {
                    (2.0f32).powf(20.0 * t - 10.0) / 2.0
                } else {
                    (2.0 - (2.0f32).powf(-20.0 * t + 10.0)) / 2.0
                }
            }
            Self::EaseInCirc => 1.0 - (1.0 - t * t).sqrt(),
            Self::EaseOutCirc => (1.0 - (t - 1.0).powi(2)).sqrt(),
            Self::EaseInOutCirc => {
                if t < 0.5 {
                    (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
                } else {
                    ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0
                }
            }
            Self::EaseInBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                c3 * t * t * t - c1 * t * t
            }
            Self::EaseOutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Self::EaseInOutBack => {
                let c1 = 1.70158;
                let c2 = c1 * 1.525;
                if t < 0.5 {
                    ((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0
                } else {
                    ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (t * 2.0 - 2.0) + c2) + 2.0) / 2.0
                }
            }
            Self::EaseInElastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                    -(2.0f32).powf(10.0 * t - 10.0) * ((10.0 * t - 10.75) * c4).sin()
                }
            }
            Self::EaseOutElastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                    (2.0f32).powf(-10.0 * t) * ((10.0 * t - 0.75) * c4).sin() + 1.0
                }
            }
            Self::EaseInOutElastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c5 = (2.0 * std::f32::consts::PI) / 4.5;
                    if t < 0.5 {
                        -((2.0f32).powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0
                    } else {
                        ((2.0f32).powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0
                            + 1.0
                    }
                }
            }
            Self::EaseInBounce => 1.0 - Self::EaseOutBounce.apply(1.0 - t),
            Self::EaseOutBounce => {
                let n1 = 7.5625;
                let d1 = 2.75;
                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    let t = t - 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    let t = t - 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d1;
                    n1 * t * t + 0.984375
                }
            }
            Self::EaseInOutBounce => {
                if t < 0.5 {
                    (1.0 - Self::EaseOutBounce.apply(1.0 - 2.0 * t)) / 2.0
                } else {
                    (1.0 + Self::EaseOutBounce.apply(2.0 * t - 1.0)) / 2.0
                }
            }
        }
    }
}

/// A tween that interpolates between two values over a duration.
pub struct Tween {
    from: f32,
    to: f32,
    duration: Duration,
    elapsed: Duration,
    easing: Easing,
}

impl Tween {
    /// Create a new tween.
    pub fn new(from: f32, to: f32, duration: Duration, easing: Easing) -> Self {
        Self {
            from,
            to,
            duration,
            elapsed: Duration::ZERO,
            easing,
        }
    }

    /// The current interpolated value.
    pub fn current(&self) -> f32 {
        let t = if self.duration.is_zero() {
            1.0
        } else {
            (self.elapsed.as_secs_f32() / self.duration.as_secs_f32()).min(1.0)
        };
        let eased = self.easing.apply(t);
        self.from + (self.to - self.from) * eased
    }
}

impl Animation for Tween {
    fn tick(&mut self, dt: Duration) {
        self.elapsed = self.elapsed.saturating_add(dt);
    }

    fn value(&self) -> f32 {
        let range = self.to - self.from;
        if range.abs() < f32::EPSILON {
            return 1.0;
        }
        ((self.current() - self.from) / range).clamp(0.0, 1.0)
    }

    fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    fn reset(&mut self) {
        self.elapsed = Duration::ZERO;
    }
}

/// A spring-physics animation.
pub struct Spring {
    position: f32,
    velocity: f32,
    target: f32,
    stiffness: f32,
    damping: f32,
    mass: f32,
    rest_threshold: f32,
}

impl Spring {
    /// Create a new spring.
    pub fn new(initial: f32, target: f32) -> Self {
        Self {
            position: initial,
            velocity: 0.0,
            target,
            stiffness: 170.0,
            damping: 26.0,
            mass: 1.0,
            rest_threshold: 0.001,
        }
    }

    /// Set the stiffness (spring constant).
    pub fn with_stiffness(mut self, stiffness: f32) -> Self {
        self.stiffness = stiffness;
        self
    }

    /// Set the damping coefficient.
    pub fn with_damping(mut self, damping: f32) -> Self {
        self.damping = damping;
        self
    }

    /// Set the mass.
    pub fn with_mass(mut self, mass: f32) -> Self {
        self.mass = mass.max(0.001);
        self
    }

    /// Set a new target position.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Current position.
    pub fn position(&self) -> f32 {
        self.position
    }
}

impl Animation for Spring {
    fn tick(&mut self, dt: Duration) {
        let dt = dt.as_secs_f32();
        let displacement = self.position - self.target;
        let spring_force = -self.stiffness * displacement;
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.mass;
        self.velocity += acceleration * dt;
        self.position += self.velocity * dt;
    }

    fn value(&self) -> f32 {
        self.position
    }

    fn is_finished(&self) -> bool {
        let displacement = (self.position - self.target).abs();
        displacement < self.rest_threshold && self.velocity.abs() < self.rest_threshold
    }

    fn reset(&mut self) {
        self.velocity = 0.0;
    }
}

/// A timeline that manages multiple named animations.
pub struct Timeline {
    entries: Vec<(String, Box<dyn Animation>)>,
    started: Option<Instant>,
}

impl Timeline {
    /// Create an empty timeline.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            started: None,
        }
    }

    /// Add a named animation.
    pub fn add(&mut self, name: impl Into<String>, anim: impl Animation + 'static) {
        self.entries.push((name.into(), Box::new(anim)));
    }

    /// Start the timeline (resets the clock).
    pub fn start(&mut self) {
        self.started = Some(Instant::now());
    }

    /// Advance all animations by the given delta.
    pub fn tick(&mut self, dt: Duration) {
        if self.started.is_none() {
            self.started = Some(Instant::now());
        }
        for (_, anim) in &mut self.entries {
            anim.tick(dt);
        }
    }

    /// Get the current value of a named animation.
    pub fn get(&self, name: &str) -> Option<f32> {
        self.entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, a)| a.value())
    }

    /// Whether all animations are finished.
    pub fn is_finished(&self) -> bool {
        self.entries.iter().all(|(_, a)| a.is_finished())
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

/// A single keyframe in a keyframe sequence.
#[derive(Debug, Clone)]
pub struct Keyframe {
    /// The value at this keyframe.
    pub value: f32,
    /// The time offset from the animation start.
    pub time: Duration,
    /// Easing function to interpolate *from* this keyframe to the next.
    pub easing: Easing,
}

impl Keyframe {
    pub fn new(value: f32, time: Duration) -> Self {
        Self {
            value,
            time,
            easing: Easing::Linear,
        }
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

/// An animation defined by a sequence of keyframes.
///
/// Interpolates between keyframes using each segment's easing function.
/// After the last keyframe, the animation is finished and holds the final value.
///
/// # Example
/// ```
/// use std::time::Duration;
/// use dewey::animation::{Keyframe, KeyframeSequence, Easing, Animation};
///
/// let mut seq = KeyframeSequence::new(vec![
///     Keyframe::new(0.0, Duration::ZERO),
///     Keyframe::new(100.0, Duration::from_millis(500)).with_easing(Easing::EaseOut),
///     Keyframe::new(50.0, Duration::from_millis(1000)).with_easing(Easing::EaseInOut),
/// ]);
///
/// seq.tick(Duration::from_millis(250));
/// let mid = seq.current();
/// assert!(mid > 0.0 && mid < 100.0);
/// ```
pub struct KeyframeSequence {
    keyframes: Vec<Keyframe>,
    elapsed: Duration,
    repeat: bool,
}

impl KeyframeSequence {
    /// Create a new keyframe sequence. Keyframes are sorted by time internally.
    pub fn new(mut keyframes: Vec<Keyframe>) -> Self {
        keyframes.sort_by_key(|a| a.time);
        Self {
            keyframes,
            elapsed: Duration::ZERO,
            repeat: false,
        }
    }

    /// Enable looping: after the last keyframe, restart from the beginning.
    pub fn with_repeat(mut self, repeat: bool) -> Self {
        self.repeat = repeat;
        self
    }

    /// The total duration of the sequence (time of last keyframe).
    pub fn duration(&self) -> Duration {
        self.keyframes
            .last()
            .map(|k| k.time)
            .unwrap_or(Duration::ZERO)
    }

    /// The current interpolated value.
    pub fn current(&self) -> f32 {
        if self.keyframes.is_empty() {
            return 0.0;
        }
        if self.keyframes.len() == 1 {
            return self.keyframes[0].value;
        }

        let total = self.duration();
        let elapsed = if self.repeat && !total.is_zero() {
            Duration::from_secs_f64(self.elapsed.as_secs_f64() % total.as_secs_f64())
        } else {
            self.elapsed
        };

        // Find the segment we're in
        for i in 0..self.keyframes.len() - 1 {
            let from = &self.keyframes[i];
            let to = &self.keyframes[i + 1];
            if elapsed >= from.time && elapsed <= to.time {
                let segment_dur = to.time.saturating_sub(from.time);
                if segment_dur.is_zero() {
                    return to.value;
                }
                let t =
                    (elapsed.saturating_sub(from.time)).as_secs_f32() / segment_dur.as_secs_f32();
                let eased = from.easing.apply(t);
                return from.value + (to.value - from.value) * eased;
            }
        }

        // Past the end
        self.keyframes.last().map(|k| k.value).unwrap_or(0.0)
    }
}

impl Animation for KeyframeSequence {
    fn tick(&mut self, dt: Duration) {
        self.elapsed = self.elapsed.saturating_add(dt);
    }

    fn value(&self) -> f32 {
        let total_dur = self.duration();
        if total_dur.is_zero() {
            return 1.0;
        }
        (self.elapsed.as_secs_f32() / total_dur.as_secs_f32()).min(1.0)
    }

    fn is_finished(&self) -> bool {
        if self.repeat {
            return false;
        }
        self.elapsed >= self.duration()
    }

    fn reset(&mut self) {
        self.elapsed = Duration::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_boundaries() {
        for easing in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::EaseOutBounce,
        ] {
            assert!((easing.apply(0.0)).abs() < 0.01, "{easing:?} at 0");
            assert!((easing.apply(1.0) - 1.0).abs() < 0.01, "{easing:?} at 1");
        }
    }

    #[test]
    fn tween_interpolation() {
        let mut tw = Tween::new(0.0, 100.0, Duration::from_secs(1), Easing::Linear);
        tw.tick(Duration::from_millis(500));
        assert!((tw.current() - 50.0).abs() < 1.0);
        tw.tick(Duration::from_millis(500));
        assert!((tw.current() - 100.0).abs() < 0.01);
        assert!(tw.is_finished());
    }

    #[test]
    fn spring_converges() {
        let mut s = Spring::new(0.0, 1.0);
        for _ in 0..1000 {
            s.tick(Duration::from_millis(16));
        }
        assert!(
            (s.position() - 1.0).abs() < 0.01,
            "Spring did not converge: {}",
            s.position()
        );
    }
}
