//! Translation of Animation/SpringSolution.swift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringSolution {
    pub response: f32,
    pub damping_fraction: f32,
    omega0: f32,
    zeta: f32,
    omega_d: f32,
    lambda1: f32,
    lambda2: f32,
}
impl SpringSolution {
    pub fn normalized_response(value: f64) -> f64 {
        if value.is_finite() {
            value.clamp(0.001, 30.0)
        } else {
            0.4
        }
    }
    pub fn normalized_damping(value: f64) -> f64 {
        if value.is_finite() {
            value.clamp(0.01, 20.0)
        } else {
            1.0
        }
    }
    pub fn new(response: f32, damping_fraction: f32) -> Self {
        let r = Self::normalized_response(response as f64) as f32;
        let z = Self::normalized_damping(damping_fraction as f64) as f32;
        let w0 = 2.0 * std::f32::consts::PI / r;
        let (omega_d, lambda1, lambda2) = if z < 1.0 {
            (w0 * (1.0 - z * z).max(0.0).sqrt(), 0.0, 0.0)
        } else if (z - 1.0).abs() < 0.0001 {
            (w0, -w0, -w0)
        } else {
            let root = (z * z - 1.0).max(0.0).sqrt();
            (0.0, -w0 * (z - root), -w0 * (z + root))
        };
        Self {
            response: r,
            damping_fraction: z,
            omega0: w0,
            zeta: z,
            omega_d,
            lambda1,
            lambda2,
        }
    }
    pub fn seconds(&self, normalized: f32) -> f32 {
        normalized * self.response
    }
    pub fn settling_duration(&self) -> f64 {
        let rate = if self.zeta <= 1.0 {
            self.zeta * self.omega0
        } else {
            self.omega0 / (self.zeta + (self.zeta * self.zeta - 1.0).sqrt())
        };
        (12.0 / rate.max(0.0001) as f64).min(30.0)
    }
    pub fn position(&self, t: f32, start: f32, velocity: f32, target: f32) -> f32 {
        if t <= 0.0 || t.is_nan() {
            return start;
        }
        if t == f32::INFINITY {
            return target;
        }
        let d = start - target;
        let displacement = if self.zeta < 1.0 {
            let envelope = (-self.zeta * self.omega0 * t).exp();
            let b = (velocity + self.zeta * self.omega0 * d) / self.omega_d;
            envelope * (d * (self.omega_d * t).cos() + b * (self.omega_d * t).sin())
        } else if (self.zeta - 1.0).abs() < 0.0001 {
            let c = velocity + self.omega0 * d;
            (-self.omega0 * t).exp() * (d + c * t)
        } else {
            let c1 = (velocity - self.lambda2 * d) / (self.lambda1 - self.lambda2);
            let c2 = (self.lambda1 * d - velocity) / (self.lambda1 - self.lambda2);
            c1 * (self.lambda1 * t).exp() + c2 * (self.lambda2 * t).exp()
        };
        target + displacement
    }
    pub fn velocity(&self, t: f32, start: f32, velocity: f32, target: f32) -> f32 {
        if t <= 0.0 || t.is_nan() {
            return velocity;
        }
        if t == f32::INFINITY {
            return 0.0;
        }
        let d = start - target;
        if self.zeta < 1.0 {
            let envelope = (-self.zeta * self.omega0 * t).exp();
            let b = (velocity + self.zeta * self.omega0 * d) / self.omega_d;
            envelope
                * ((b * self.omega_d - self.zeta * self.omega0 * d) * (self.omega_d * t).cos()
                    - (d * self.omega_d + self.zeta * self.omega0 * b) * (self.omega_d * t).sin())
        } else if (self.zeta - 1.0).abs() < 0.0001 {
            let c = velocity + self.omega0 * d;
            (-self.omega0 * t).exp() * (velocity - self.omega0 * c * t)
        } else {
            let c1 = (velocity - self.lambda2 * d) / (self.lambda1 - self.lambda2);
            let c2 = (self.lambda1 * d - velocity) / (self.lambda1 - self.lambda2);
            c1 * self.lambda1 * (self.lambda1 * t).exp()
                + c2 * self.lambda2 * (self.lambda2 * t).exp()
        }
    }
}
