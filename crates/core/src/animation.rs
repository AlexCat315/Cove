/// Analytic damped spring; evaluation does not depend on frame rate.
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub mass: f64,
    pub stiffness: f64,
    pub damping: f64,
}

impl Default for Spring {
    fn default() -> Self {
        Self {
            mass: 1.0,
            stiffness: 170.0,
            damping: 26.0,
        }
    }
}

impl Spring {
    pub fn sample(self, displacement: f64, velocity: f64, seconds: f64) -> Option<(f64, f64)> {
        if !self.mass.is_finite()
            || !self.stiffness.is_finite()
            || !self.damping.is_finite()
            || self.mass <= 0.0
            || self.stiffness <= 0.0
            || self.damping < 0.0
            || !displacement.is_finite()
            || !velocity.is_finite()
            || !seconds.is_finite()
            || seconds < 0.0
        {
            return None;
        }
        let w = (self.stiffness / self.mass).sqrt();
        let a = self.damping / (2.0 * self.mass);
        let discriminant = a * a - w * w;
        if !w.is_finite() || !a.is_finite() || !discriminant.is_finite() {
            return None;
        }
        let t = seconds;
        let (x, v) = if discriminant.abs() < 1e-10 * w * w {
            let b = velocity + a * displacement;
            let e = (-a * t).exp();
            (
                e * (displacement + b * t),
                e * (b - a * (displacement + b * t)),
            )
        } else if discriminant < 0.0 {
            let d = (-discriminant).sqrt();
            let b = (velocity + a * displacement) / d;
            let (s, c) = (d * t).sin_cos();
            let e = (-a * t).exp();
            let x = e * (displacement * c + b * s);
            (x, -a * x + e * d * (-displacement * s + b * c))
        } else {
            let d = discriminant.sqrt();
            let r1 = -w * w / (a + d);
            let r2 = -a - d;
            let c1 = (velocity - r2 * displacement) / (r1 - r2);
            let c2 = displacement - c1;
            let e1 = (r1 * t).exp();
            let e2 = (r2 * t).exp();
            (c1 * e1 + c2 * e2, c1 * r1 * e1 + c2 * r2 * e2)
        };
        (x.is_finite() && v.is_finite()).then_some((x, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spring_is_interruptible_and_independent_of_frame_rate() {
        for damping in [5.0, 20.0, 40.0] {
            let spring = Spring {
                mass: 1.0,
                stiffness: 100.0,
                damping,
            };
            let whole = spring.sample(10.0, -3.0, 0.5).unwrap();
            let half = spring.sample(10.0, -3.0, 0.25).unwrap();
            let two = spring.sample(half.0, half.1, 0.25).unwrap();
            assert!((whole.0 - two.0).abs() < 1e-8);
            assert!((whole.1 - two.1).abs() < 1e-8);
            assert!(spring.sample(10.0, 0.0, 100.0).unwrap().0.abs() < 1e-8);
        }
    }
}
