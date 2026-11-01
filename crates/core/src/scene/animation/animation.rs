//! Translation of Animation/Animation.swift, Curves/Steps.swift and Curves/Oscillation.swift.
use super::{AnimationRepetition, LinearTimingCurve, SpringSolution};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationStepPosition {
    Start,
    #[default]
    End,
}
#[derive(Clone, Debug, PartialEq)]
pub enum AnimationCurve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Steps(i64, AnimationStepPosition),
    PiecewiseLinear(LinearTimingCurve),
    BounceOut,
    ElasticOut,
    Spring {
        response: f32,
        damping_fraction: f32,
    },
    CubicBezier(f32, f32, f32, f32),
}
impl AnimationCurve {
    pub fn evaluate(&self, t: f32) -> f32 {
        let x = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        match self {
            Self::Linear => x,
            Self::EaseIn => x * x,
            Self::EaseOut => 1.0 - (1.0 - x).powi(2),
            Self::EaseInOut => {
                if x < 0.5 {
                    2.0 * x * x
                } else {
                    1.0 - 2.0 * (1.0 - x).powi(2)
                }
            }
            Self::Steps(count, position) => {
                let intervals = (*count).max(1) as f32;
                (((x * intervals).floor()
                    + if *position == AnimationStepPosition::Start {
                        1.0
                    } else {
                        0.0
                    })
                    / intervals)
                    .clamp(0.0, 1.0)
            }
            Self::PiecewiseLinear(curve) => curve.evaluate(x),
            Self::BounceOut => {
                if x == 0.0 || x == 1.0 {
                    return x;
                }
                let (shifted, floor) = if x < 1.0 / 2.75 {
                    (x, 0.0)
                } else if x < 2.0 / 2.75 {
                    (x - 1.5 / 2.75, 0.75)
                } else if x < 2.5 / 2.75 {
                    (x - 2.25 / 2.75, 0.9375)
                } else {
                    (x - 2.625 / 2.75, 0.984375)
                };
                (7.5625 * shifted * shifted + floor).clamp(0.0, 1.0)
            }
            Self::ElasticOut => {
                if x == 0.0 || x == 1.0 {
                    x
                } else {
                    1.0 + 2.0_f32.powf(-10.0 * x)
                        * ((x - 0.1) * 2.0 * std::f32::consts::PI / 0.4).sin()
                }
            }
            Self::Spring {
                response,
                damping_fraction,
            } => {
                let spring = SpringSolution::new(*response, *damping_fraction);
                if x == 0.0 || x == 1.0 {
                    return x;
                }
                let end = spring.position(spring.response, 0.0, 0.0, 1.0);
                let value = spring.position(spring.seconds(x), 0.0, 0.0, 1.0);
                if end.abs() > 0.00001 {
                    value / end
                } else {
                    value
                }
            }
            Self::CubicBezier(c1x, c1y, c2x, c2y) => {
                let cx = 3.0 * c1x;
                let bx = 3.0 * (c2x - c1x) - cx;
                let ax = 1.0 - cx - bx;
                let cy = 3.0 * c1y;
                let by = 3.0 * (c2y - c1y) - cy;
                let ay = 1.0 - cy - by;
                let sample_x = |t| ((ax * t + bx) * t + cx) * t;
                let sample_y = |t| ((ay * t + by) * t + cy) * t;
                let derivative = |t| (3.0 * ax * t + 2.0 * bx) * t + cx;
                let mut t = x;
                for _ in 0..8 {
                    let error = sample_x(t) - x;
                    if error.abs() < 0.00001 {
                        return sample_y(t);
                    }
                    let dx = derivative(t);
                    if dx.abs() < 0.000001 {
                        break;
                    }
                    t -= error / dx;
                }
                let mut lo = 0.0;
                let mut hi = 1.0;
                t = x;
                for _ in 0..32 {
                    let value = sample_x(t);
                    if (value - x).abs() < 0.00001 {
                        return sample_y(t);
                    }
                    if value < x {
                        lo = t;
                    } else {
                        hi = t;
                    }
                    t = (lo + hi) * 0.5;
                }
                sample_y(t)
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub duration: f64,
    pub curve: AnimationCurve,
    pub delay: f64,
    pub allows_overshoot: bool,
    pub initial_velocity: f32,
    pub repetition: AnimationRepetition,
}
impl Default for Animation {
    fn default() -> Self {
        Self::new(0.25, AnimationCurve::EaseInOut)
    }
}
impl Animation {
    pub fn new(duration: f64, curve: AnimationCurve) -> Self {
        Self {
            duration: if duration.is_finite() {
                duration.max(0.0)
            } else {
                0.0
            },
            curve,
            delay: 0.0,
            allows_overshoot: false,
            initial_velocity: 0.0,
            repetition: AnimationRepetition::Once,
        }
    }
    pub fn with_delay(mut self, delay: f64) -> Self {
        self.delay = if delay.is_finite() {
            delay.max(0.0)
        } else {
            0.0
        };
        self
    }
    pub fn with_overshoot(mut self, allowed: bool) -> Self {
        self.allows_overshoot = allowed;
        self
    }
    pub fn with_initial_velocity(mut self, velocity: f32) -> Self {
        self.initial_velocity = if velocity.is_finite() { velocity } else { 0.0 };
        self
    }
    pub fn repeat_count(mut self, count: usize, autoreverses: bool) -> Self {
        self.repetition = AnimationRepetition::Count(count.max(1), autoreverses);
        self
    }
    pub fn repeat_forever(mut self, autoreverses: bool) -> Self {
        self.repetition = AnimationRepetition::Forever(autoreverses);
        self
    }
    pub fn linear() -> Self {
        Self::new(0.25, AnimationCurve::Linear)
    }
    pub fn ease_in() -> Self {
        Self::new(0.25, AnimationCurve::EaseIn)
    }
    pub fn ease_out() -> Self {
        Self::new(0.25, AnimationCurve::EaseOut)
    }
    pub fn ease_in_out(duration: f64) -> Self {
        Self::new(duration, AnimationCurve::EaseInOut)
    }
    pub fn spring(response: f64, damping_fraction: f64) -> Self {
        let duration = SpringSolution::normalized_response(response);
        let spring = SpringSolution::new(
            duration as f32,
            SpringSolution::normalized_damping(damping_fraction) as f32,
        );
        Self::new(
            duration,
            AnimationCurve::Spring {
                response: spring.response,
                damping_fraction: spring.damping_fraction,
            },
        )
    }
    pub fn snappy() -> Self {
        Self::spring(0.30, 0.90)
    }
    pub fn bouncy() -> Self {
        Self::spring(0.45, 0.65)
    }
    pub fn smooth() -> Self {
        Self::spring(0.40, 1.0)
    }
}
