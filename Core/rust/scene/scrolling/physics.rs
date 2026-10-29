//! Translation of CoveUIScene/Scrolling/{ScrollPhysics,RubberBand,ScrollSpring,ScrollVelocityTracker,AxisMotion}.swift.
use crate::{Point, Size};

/// Tuning for the rubber-band stretch applied when a scroll offset travels past
/// one of its bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollRubberBandSettings {
    /// `false` restores hard clamping — the offset stops dead at the bound.
    pub is_enabled: bool,
    /// Fraction of the raw overscroll visible for the first point pushed past
    /// the bound. Lower values feel stiffer.
    pub compliance: f32,
    /// Hard ceiling on the *raw* overscroll, as a multiple of the viewport.
    pub maximum_raw_overscroll: f32,
}
impl Default for ScrollRubberBandSettings {
    fn default() -> Self {
        Self { is_enabled: true, compliance: 0.55, maximum_raw_overscroll: 2.0 }
    }
}

/// Tuning for the spring that returns an overscrolled offset back inside its
/// bounds once input stops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollSettleSettings {
    /// Approximate period, in seconds, of one natural oscillation.
    pub response: f32,
    /// Damping ratio. `1` is critical damping.
    pub damping_fraction: f32,
    /// Distance below which the spring snaps to its target and stops.
    pub position_tolerance: f32,
    /// Velocity (points per second) below which the spring stops.
    pub velocity_tolerance: f32,
}
impl Default for ScrollSettleSettings {
    fn default() -> Self {
        Self { response: 0.34, damping_fraction: 1.0, position_tolerance: 0.05, velocity_tolerance: 1.0 }
    }
}

/// Tuning for inertial scrolling — the glide that carries content along after
/// the input that produced it has stopped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollMomentumSettings {
    pub is_enabled: bool,
    /// Decay rate in nepers per second.
    pub deceleration: f32,
    /// Launch speed below which the gesture counts as placed rather than thrown.
    pub minimum_velocity: f32,
    /// Speed at which an existing glide comes to rest.
    pub rest_velocity: f32,
    /// Ceiling on the launch speed.
    pub maximum_velocity: f32,
    /// Samples a gesture must have produced before it counts as a throw.
    pub minimum_sample_count: usize,
    /// Quiet period after the newest sample before the glide launches.
    pub idle_window: f64,
    /// Age limit of samples contributing to the launch velocity.
    pub tracking_window: f64,
    /// How long the newest sample keeps describing the gesture.
    pub stale_window: f64,
}
impl Default for ScrollMomentumSettings {
    fn default() -> Self {
        Self {
            is_enabled: true,
            deceleration: 4.2,
            minimum_velocity: 50.0,
            rest_velocity: 2.0,
            maximum_velocity: 1_100.0,
            minimum_sample_count: 3,
            idle_window: 0.09,
            tracking_window: 0.1,
            stale_window: 0.25,
        }
    }
}

/// How a scrollable container reacts to input that pushes it past its bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollPhysics {
    pub rubber_band: ScrollRubberBandSettings,
    pub settle: ScrollSettleSettings,
    pub momentum: ScrollMomentumSettings,
}
impl ScrollPhysics {
    pub const DEFAULT: Self = Self { rubber_band: ScrollRubberBandSettings { is_enabled: true, compliance: 0.55, maximum_raw_overscroll: 2.0 },
        settle: ScrollSettleSettings { response: 0.34, damping_fraction: 1.0, position_tolerance: 0.05, velocity_tolerance: 1.0 },
        momentum: ScrollMomentumSettings { is_enabled: true, deceleration: 4.2, minimum_velocity: 50.0, rest_velocity: 2.0,
            maximum_velocity: 1_100.0, minimum_sample_count: 3, idle_window: 0.09, tracking_window: 0.1, stale_window: 0.25 } };
    /// Hard clamping at both bounds: content stops dead instead of stretching.
    pub fn clamped() -> Self {
        Self { rubber_band: ScrollRubberBandSettings { is_enabled: false, ..ScrollRubberBandSettings::default() }, ..Self::default() }
    }
    /// Stretch plus a visible bounce at the bound on release.
    pub fn bouncy() -> Self {
        Self { settle: ScrollSettleSettings { response: 0.42, damping_fraction: 0.62, ..ScrollSettleSettings::default() }, ..Self::default() }
    }
    /// Stiff early curve, slow return, no bounce.
    pub fn firm() -> Self {
        Self {
            rubber_band: ScrollRubberBandSettings { compliance: 0.25, ..ScrollRubberBandSettings::default() },
            settle: ScrollSettleSettings { response: 0.5, ..ScrollSettleSettings::default() },
            ..Self::default()
        }
    }
    /// Nothing that was not asked for: no stretching, no coasting.
    pub fn precise() -> Self {
        Self {
            rubber_band: ScrollRubberBandSettings { is_enabled: false, ..ScrollRubberBandSettings::default() },
            momentum: ScrollMomentumSettings { is_enabled: false, ..ScrollMomentumSettings::default() },
            ..Self::default()
        }
    }
}

/// Maps a raw overscroll past a scroll bound onto the smaller, progressively
/// stiffening distance that is actually drawn — the rubber band.
///
/// ```text
/// projected(x) = (c · |x| · d) / (d + c · |x|)
/// ```
pub struct RubberBand;
impl RubberBand {
    /// Project signed `overscroll` onto the visible displacement.
    pub fn project(overscroll: f32, extent: f32, compliance: f32) -> f32 {
        if overscroll == 0.0 || extent <= 0.0 || compliance <= 0.0 {
            return 0.0;
        }
        let distance = overscroll.abs();
        let resilience = compliance * distance * extent;
        let projected = resilience / (extent + compliance * distance);
        if overscroll < 0.0 { -projected } else { projected }
    }
    /// Signed overscroll of `value` against the closed range `0...maximum`.
    pub fn excess(value: f32, maximum: f32) -> f32 {
        if value < 0.0 {
            value
        } else if value > maximum {
            value - maximum
        } else {
            0.0
        }
    }
    /// Inverse of `project`.
    pub fn unproject(visible_overscroll: f32, extent: f32, compliance: f32) -> f32 {
        if visible_overscroll == 0.0 || extent <= 0.0 || compliance <= 0.0 {
            return 0.0;
        }
        let remaining = extent - visible_overscroll.abs();
        if remaining <= 0.0 {
            return 0.0;
        }
        (visible_overscroll * extent) / (compliance * remaining)
    }
}

/// Analytic damped spring in visible logical coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollSpring {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
    pub settings: ScrollSettleSettings,
}
impl ScrollSpring {
    pub fn new(value: f32, velocity: f32, target: f32, settings: ScrollSettleSettings) -> Self {
        Self { value, velocity, target, settings }
    }
    pub fn is_at_rest(&self) -> bool {
        (self.value - self.target).abs() <= self.settings.position_tolerance.max(0.0)
            && self.velocity.abs() <= self.settings.velocity_tolerance.max(0.0)
    }
    pub fn advance(&mut self, delta_time: f32) {
        if !delta_time.is_finite() || delta_time <= 0.0 {
            return;
        }
        if self.is_at_rest() {
            self.snap_to_target();
            return;
        }
        let omega = 2.0 * std::f64::consts::PI / f64::from(self.settings.response.max(0.0001));
        let ratio = f64::from(self.settings.damping_fraction.max(0.0));
        let time = f64::from(delta_time);
        let displacement = f64::from(self.value - self.target);
        let speed = f64::from(self.velocity);
        let (next, next_speed) = if (ratio - 1.0).abs() < 0.00001 {
            let b = speed + omega * displacement;
            let decay = (-omega * time).exp();
            ((displacement + b * time) * decay, (speed - omega * b * time) * decay)
        } else if ratio < 1.0 {
            let damped = omega * (1.0 - ratio * ratio).sqrt();
            let decay = (-ratio * omega * time).exp();
            let b = (speed + ratio * omega * displacement) / damped;
            let phase = damped * time;
            let next = decay * (displacement * phase.cos() + b * phase.sin());
            (next, -ratio * omega * next + decay * damped * (-displacement * phase.sin() + b * phase.cos()))
        } else {
            let root = (ratio * ratio - 1.0).sqrt();
            let slow = -omega / (ratio + root);
            let fast = -omega * (ratio + root);
            let a = (speed - fast * displacement) / (slow - fast);
            let b = displacement - a;
            (a * (slow * time).exp() + b * (fast * time).exp(), slow * a * (slow * time).exp() + fast * b * (fast * time).exp())
        };
        self.value = self.target + next as f32;
        self.velocity = next_speed as f32;
        if self.is_at_rest() {
            self.snap_to_target();
        }
    }
    pub fn snap_to_target(&mut self) {
        self.value = self.target;
        self.velocity = 0.0;
    }
}

/// Estimates how fast, along one axis, the user is scrolling from a stream of
/// position samples, using a least-squares fit over a short window.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollVelocityTracker {
    pub window: f64,
    samples: Vec<(f64, f32)>,
}
impl ScrollVelocityTracker {
    const MAXIMUM_SAMPLE_COUNT: usize = 16;
    pub fn new(window: f64) -> Self {
        Self { window, samples: Vec::new() }
    }
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
    pub fn reset(&mut self) {
        self.samples.clear();
    }
    pub fn add(&mut self, time: f64, value: f32) {
        if !time.is_finite() || !value.is_finite() {
            return;
        }
        if let Some((last_time, _)) = self.samples.last()
            && time < *last_time
        {
            self.reset();
        }
        self.samples.push((time, value));
        self.trim(time);
    }
    /// Least-squares slope of the retained samples, in points per second.
    pub fn velocity(&mut self, time: Option<f64>, stale_after: Option<f64>) -> f32 {
        let Some((newest, _)) = self.samples.last().copied() else { return 0.0 };
        if let (Some(stale_after), Some(now)) = (stale_after, time)
            && now - newest > stale_after
        {
            return 0.0;
        }
        self.trim(newest);
        if self.samples.len() < 2 {
            return 0.0;
        }
        let count = self.samples.len() as f64;
        let mean_time = self.samples.iter().map(|(time, _)| *time).sum::<f64>() / count;
        let mean_value = self.samples.iter().map(|(_, value)| f64::from(*value)).sum::<f64>() / count;
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        for (time, value) in &self.samples {
            let elapsed = time - mean_time;
            numerator += elapsed * (f64::from(*value) - mean_value);
            denominator += elapsed * elapsed;
        }
        if denominator <= 0.0 {
            return 0.0;
        }
        let slope = numerator / denominator;
        if slope.is_finite() { slope as f32 } else { 0.0 }
    }
    fn trim(&mut self, time: f64) {
        if !time.is_finite() {
            return;
        }
        let cutoff = time - self.window.max(0.0);
        self.samples.retain(|(sample_time, _)| *sample_time >= cutoff);
        if self.samples.len() > Self::MAXIMUM_SAMPLE_COUNT {
            self.samples.drain(..self.samples.len() - Self::MAXIMUM_SAMPLE_COUNT);
        }
    }
}
impl Default for ScrollVelocityTracker {
    fn default() -> Self {
        Self::new(0.1)
    }
}

/// An axis changes from glide to spring at the time it reaches its edge,
/// independently of the other axis. Positions are raw; springs are visible.
#[derive(Clone, Debug, Default)]
pub(crate) struct ScrollAxisMotion {
    pub glide: Option<f32>,
    pub spring: Option<ScrollSpring>,
}
impl ScrollAxisMotion {
    pub fn stop(&mut self) {
        self.glide = None;
        self.spring = None;
    }
    /// Returns `(raw position, visible offset)`.
    pub fn advance(&mut self, position: f32, maximum: f32, extent: f32, physics: ScrollPhysics, duration: f32) -> (f32, f32) {
        if let Some(spring) = self.spring.as_mut() {
            spring.advance(duration);
            let visible = spring.value;
            if spring.is_at_rest() {
                self.spring = None;
            }
            let excess = RubberBand::excess(spring.value, maximum);
            let raw = if excess == 0.0 {
                visible
            } else {
                (if excess < 0.0 { 0.0 } else { maximum })
                    + RubberBand::unproject(excess, extent, physics.rubber_band.compliance)
            };
            return (raw, visible);
        }
        let Some(speed) = self.glide else { return (position, position) };
        let rate = physics.momentum.deceleration.max(0.1);
        let floor = physics.momentum.rest_velocity.max(0.01);
        let rest_time = ((floor.max(speed.abs()) / floor).ln() / rate).max(0.0);
        let step = duration.min(rest_time);
        let travel = -(-rate * step).exp_m1() / rate;
        let next = position + speed * travel;
        let edge = if speed < 0.0 { 0.0 } else { maximum };
        if (speed < 0.0 && next <= 0.0) || (speed > 0.0 && next >= maximum) {
            let fraction = (edge - position) * rate / speed;
            let hit_time = -(1.0 - fraction).max(0.000001).ln() / rate;
            self.glide = None;
            if !physics.rubber_band.is_enabled {
                return (edge, edge);
            }
            self.spring = Some(ScrollSpring::new(edge, speed * (-rate * hit_time).exp(), edge, physics.settle));
            return self.advance(edge, maximum, extent, physics, (duration - hit_time).max(0.0));
        }
        self.glide = if duration >= rest_time { None } else { Some(speed * (-rate * duration).exp()) };
        (next, next)
    }
}

/// Window-space rectangle helpers shared by the animator.
pub(crate) fn constrain(value: Point, maximum: Point) -> Point {
    Point { x: value.x.clamp(0.0, maximum.x.max(0.0)), y: value.y.clamp(0.0, maximum.y.max(0.0)) }
}
pub(crate) fn size_of(point: Point) -> Size {
    Size { width: point.x, height: point.y }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rubber_band_is_monotonic_and_bounded() {
        assert_eq!(RubberBand::project(0.0, 100.0, 0.55), 0.0);
        assert_eq!(RubberBand::project(10.0, 0.0, 0.55), 0.0);
        let first = RubberBand::project(10.0, 100.0, 0.55);
        let second = RubberBand::project(40.0, 100.0, 0.55);
        assert!(second > first);
        assert!(RubberBand::project(10_000.0, 100.0, 0.55) < 100.0);
        assert_eq!(RubberBand::project(-10.0, 100.0, 0.55), -first);
    }

    #[test]
    fn unproject_inverts_project() {
        let projected = RubberBand::project(25.0, 300.0, 0.55);
        let raw = RubberBand::unproject(projected, 300.0, 0.55);
        assert!((raw - 25.0).abs() < 1e-3);
        assert_eq!(RubberBand::unproject(300.0, 300.0, 0.55), 0.0);
    }

    #[test]
    fn spring_settles_on_its_target() {
        let mut spring = ScrollSpring::new(50.0, 0.0, 0.0, ScrollSettleSettings::default());
        for _ in 0..240 {
            spring.advance(1.0 / 60.0);
        }
        assert!(spring.is_at_rest());
        assert_eq!(spring.value, 0.0);
    }

    #[test]
    fn velocity_fit_tracks_a_steady_drag() {
        let mut tracker = ScrollVelocityTracker::new(0.1);
        for step in 0..8 {
            tracker.add(step as f64 * 0.01, step as f32 * 2.0);
        }
        let velocity = tracker.velocity(Some(0.08), Some(0.25));
        assert!((velocity - 200.0).abs() < 1.0);
        assert_eq!(tracker.velocity(Some(5.0), Some(0.25)), 0.0);
        tracker.reset();
        assert_eq!(tracker.sample_count(), 0);
    }

    #[test]
    fn glide_stops_at_the_edge_and_hands_over_to_the_spring() {
        let mut motion = ScrollAxisMotion { glide: Some(1_000.0), spring: None };
        let (raw, visible) = motion.advance(90.0, 100.0, 50.0, ScrollPhysics::DEFAULT, 1.0 / 60.0);
        assert!(raw >= 100.0 || visible >= 100.0);
        assert!(motion.glide.is_none() || motion.spring.is_some());
    }
}
