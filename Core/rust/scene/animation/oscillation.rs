//! Translation of Animation/Curves/Oscillation.swift.
//!
//! The curve values are also inlined in `AnimationCurve::evaluate`; these entry
//! points exist so callers (and tests) can sample an oscillation without
//! constructing a curve.
use super::AnimationCurve;

impl AnimationCurve {
    pub fn evaluate_bounce_out(time: f32) -> f32 {
        if time == 0.0 || time == 1.0 {
            return time;
        }
        let scale: f32 = 2.75;
        let (shifted, floor) = if time < 1.0 / scale {
            (time, 0.0)
        } else if time < 2.0 / scale {
            (time - 1.5 / scale, 0.75)
        } else if time < 2.5 / scale {
            (time - 2.25 / scale, 0.9375)
        } else {
            (time - 2.625 / scale, 0.984375)
        };
        (7.5625 * shifted * shifted + floor).clamp(0.0, 1.0)
    }

    pub fn evaluate_elastic_out(time: f32) -> f32 {
        if time == 0.0 || time == 1.0 {
            return time;
        }
        let period: f32 = 0.4;
        1.0 + 2f32.powf(-10.0 * time) * ((time - period / 4.0) * (2.0 * std::f32::consts::PI) / period).sin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounce_out_pins_its_endpoints() {
        assert_eq!(AnimationCurve::evaluate_bounce_out(0.0), 0.0);
        assert_eq!(AnimationCurve::evaluate_bounce_out(1.0), 1.0);
        assert!((AnimationCurve::evaluate_bounce_out(0.5) - 0.765625).abs() < 1e-5);
    }

    #[test]
    fn elastic_out_settles_on_one() {
        assert_eq!(AnimationCurve::evaluate_elastic_out(0.0), 0.0);
        assert!((AnimationCurve::evaluate_elastic_out(1.0) - 1.0).abs() < 1e-5);
        assert!(AnimationCurve::evaluate_elastic_out(0.6) > 0.9);
    }

    #[test]
    fn curve_evaluation_matches_the_helpers() {
        for step in 0..=10 {
            let time = step as f32 / 10.0;
            assert_eq!(
                AnimationCurve::BounceOut.evaluate(time),
                AnimationCurve::evaluate_bounce_out(time)
            );
        }
    }
}
