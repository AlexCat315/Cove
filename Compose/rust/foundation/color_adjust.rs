//! Color derivation helpers used by widget styles.
use cove_core::Color;

pub trait ColorAdjustExt {
    fn darker(self, amount: f32) -> Color;
    fn lighter(self, amount: f32) -> Color;
    fn mixed(self, other: Color, amount: f32) -> Color;
}

impl ColorAdjustExt for Color {
    fn darker(self, amount: f32) -> Color {
        let amount = amount.clamp(0.0, 1.0);
        Color {
            r: self.r * (1.0 - amount),
            g: self.g * (1.0 - amount),
            b: self.b * (1.0 - amount),
            a: self.a,
        }
    }

    fn lighter(self, amount: f32) -> Color {
        let amount = amount.clamp(0.0, 1.0);
        Color {
            r: self.r + (1.0 - self.r) * amount,
            g: self.g + (1.0 - self.g) * amount,
            b: self.b + (1.0 - self.b) * amount,
            a: self.a,
        }
    }

    fn mixed(self, other: Color, amount: f32) -> Color {
        let amount = amount.clamp(0.0, 1.0);
        Color {
            r: self.r + (other.r - self.r) * amount,
            g: self.g + (other.g - self.g) * amount,
            b: self.b + (other.b - self.b) * amount,
            a: self.a + (other.a - self.a) * amount,
        }
    }
}
