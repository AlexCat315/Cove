//! HSV conversion and colour-wheel geometry.
//!
//! The wheel maps screen angle to hue (red at the top, clockwise) and the
//! inscribed square to saturation and value. Keeping the mapping here lets the
//! pointer handler, the draw pass and the tests share one definition.
//!
//! Behaviour checklist:
//! - a grayscale has saturation 0 and hue 0
//! - hue wraps in `0…1`; the RGB round trip is stable within a channel step
//! - angle 0 is screen-up and grows clockwise, matching `atan2(dy, dx)` with a
//!   downward y axis
//! - the square maps left→right to saturation 0→1 and top→bottom to value 1→0

use cove_core::Color;
use std::f32::consts::{FRAC_PI_2, TAU};

/// Hue, saturation and value, each in `0…1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

impl Hsv {
    pub fn new(h: f32, s: f32, v: f32) -> Self {
        Self {
            h: h.rem_euclid(1.0),
            s: s.clamp(0.0, 1.0),
            v: v.clamp(0.0, 1.0),
        }
    }

    /// The RGB colour, keeping `alpha`.
    pub fn color(self, alpha: f32) -> Color {
        color_from_hsv(self, alpha)
    }
}

/// Splits an RGB colour into HSV.
pub fn hsv_from_color(color: Color) -> Hsv {
    let max = color.r.max(color.g).max(color.b);
    let min = color.r.min(color.g).min(color.b);
    let delta = max - min;
    let s = if max <= 0.0 { 0.0 } else { delta / max };
    let h = if delta <= 0.0 {
        0.0
    } else if max == color.r {
        ((color.g - color.b) / delta).rem_euclid(6.0) / 6.0
    } else if max == color.g {
        (((color.b - color.r) / delta) + 2.0) / 6.0
    } else {
        (((color.r - color.g) / delta) + 4.0) / 6.0
    };
    Hsv {
        h: h.rem_euclid(1.0),
        s: s.clamp(0.0, 1.0),
        v: max.clamp(0.0, 1.0),
    }
}

/// Builds an RGB colour from HSV, keeping `alpha`.
pub fn color_from_hsv(hsv: Hsv, alpha: f32) -> Color {
    let h = hsv.h.rem_euclid(1.0) * 6.0;
    let s = hsv.s.clamp(0.0, 1.0);
    let v = hsv.v.clamp(0.0, 1.0);
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    Color {
        r: r + m,
        g: g + m,
        b: b + m,
        a: alpha.clamp(0.0, 1.0),
    }
}

/// The hue at a screen angle, red at the top and clockwise.
pub fn hue_for_angle(angle: f32) -> f32 {
    ((angle + FRAC_PI_2) / TAU).rem_euclid(1.0)
}

/// The screen angle for a hue, the inverse of [`hue_for_angle`].
pub fn angle_for_hue(hue: f32) -> f32 {
    hue.rem_euclid(1.0) * TAU - FRAC_PI_2
}

/// Saturation and value for a point inside the square, measured from its
/// top-left corner.
pub fn sv_for_point(x: f32, y: f32, side: f32) -> (f32, f32) {
    if !side.is_finite() || side <= 0.0 {
        return (0.0, 1.0);
    }
    ((x / side).clamp(0.0, 1.0), 1.0 - (y / side).clamp(0.0, 1.0))
}

/// The point for a saturation and value, relative to the square's top-left.
pub fn point_for_sv(s: f32, v: f32, side: f32) -> (f32, f32) {
    if !side.is_finite() || side <= 0.0 {
        return (0.0, 0.0);
    }
    (s.clamp(0.0, 1.0) * side, (1.0 - v.clamp(0.0, 1.0)) * side)
}

/// Saturation for a distance from the disk centre.
pub fn disk_saturation_for_radius(distance: f32, radius: f32) -> f32 {
    if !radius.is_finite() || radius <= 0.0 {
        return 0.0;
    }
    (distance / radius).clamp(0.0, 1.0)
}

/// The distance from the centre for a saturation on the disk.
pub fn disk_radius_for_saturation(s: f32, radius: f32) -> f32 {
    if !radius.is_finite() || radius <= 0.0 {
        return 0.0;
    }
    s.clamp(0.0, 1.0) * radius
}
