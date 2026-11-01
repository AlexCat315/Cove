//! Minimum and maximum editor height normalization.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorHeightLimits {
    pub minimum: f32,
    pub maximum: f32,
}

impl EditorHeightLimits {
    pub fn new(minimum: f32, maximum: f32) -> Self {
        let minimum = if minimum.is_finite() {
            minimum.max(0.0)
        } else {
            96.0
        };
        let maximum = if maximum.is_finite() {
            maximum.max(minimum)
        } else {
            minimum.max(520.0)
        };
        Self { minimum, maximum }
    }

    pub fn clamp(self, value: f32) -> f32 {
        self.minimum
            .max(if value.is_finite() {
                value
            } else {
                self.minimum
            })
            .min(self.maximum)
    }
}
