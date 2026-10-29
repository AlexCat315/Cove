use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn contains(self, point: Point) -> bool {
        self.width > 0.0
            && self.height > 0.0
            && point.x >= self.x
            && point.y >= self.y
            && point.x < self.x + self.width
            && point.y < self.y + self.height
    }
    pub fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let width = (self.x + self.width).min(other.x + other.width) - x;
        let height = (self.y + self.height).min(other.y + other.height) - y;
        (width > 0.0 && height > 0.0).then_some(Self::new(x, y, width, height))
    }
    pub fn translated(self, offset: Point) -> Self {
        Self {
            x: self.x + offset.x,
            y: self.y + offset.y,
            ..self
        }
    }
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self::new(
            x,
            y,
            (self.x + self.width).max(other.x + other.width) - x,
            (self.y + self.height).max(other.y + other.height) - y,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const CLEAR: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }
    /// Straight-alpha RGBA, R in the low byte. Identical to the native vertex ABI.
    pub fn rgba8(self) -> u32 {
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        byte(self.r) | byte(self.g) << 8 | byte(self.b) << 16 | byte(self.a) << 24
    }
    pub fn multiplied_alpha(self, opacity: f32) -> Self {
        Self {
            a: self.a * opacity.clamp(0.0, 1.0),
            ..self
        }
    }
    pub fn composited(self, overlay: Self) -> Self {
        let oa = overlay.a.clamp(0.0, 1.0);
        let a = oa + self.a * (1.0 - oa);
        if a <= 0.0 {
            return Self::CLEAR;
        }
        let blend = |base, top| (top * oa + base * self.a * (1.0 - oa)) / a;
        Self {
            r: blend(self.r, overlay.r),
            g: blend(self.g, overlay.g),
            b: blend(self.b, overlay.b),
            a,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgba_and_source_over() {
        assert_eq!(
            Color::from_rgba8(0x12, 0x34, 0x56, 0x78).rgba8(),
            0x78563412
        );
        let result = Color::BLACK.composited(Color {
            a: 0.5,
            ..Color::WHITE
        });
        assert_eq!(
            result,
            Color {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 1.0
            }
        );
        assert_eq!(Color::CLEAR.composited(Color::CLEAR), Color::CLEAR);
    }
}
