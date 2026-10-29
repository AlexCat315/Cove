//! Translation of Animation/Interpolatable.swift.
use crate::{Color, Point, Rect, Size};
pub trait Interpolatable: Clone + 'static { fn interpolate(a: &Self, b: &Self, t: f32) -> Self; }
impl Interpolatable for f32 { fn interpolate(a: &Self, b: &Self, t: f32) -> Self { a + (b - a) * t } }
impl Interpolatable for f64 { fn interpolate(a: &Self, b: &Self, t: f32) -> Self { a + (b - a) * t as f64 } }
impl Interpolatable for Color { fn interpolate(a: &Self, b: &Self, t: f32) -> Self { Self { r: f32::interpolate(&a.r, &b.r, t), g: f32::interpolate(&a.g, &b.g, t), b: f32::interpolate(&a.b, &b.b, t), a: f32::interpolate(&a.a, &b.a, t) } } }
impl Interpolatable for Point { fn interpolate(a: &Self, b: &Self, t: f32) -> Self { Self { x: f32::interpolate(&a.x, &b.x, t), y: f32::interpolate(&a.y, &b.y, t) } } }
impl Interpolatable for Size { fn interpolate(a: &Self, b: &Self, t: f32) -> Self { Self { width: f32::interpolate(&a.width, &b.width, t), height: f32::interpolate(&a.height, &b.height, t) } } }
impl Interpolatable for Rect { fn interpolate(a: &Self, b: &Self, t: f32) -> Self { Self { x: f32::interpolate(&a.x, &b.x, t), y: f32::interpolate(&a.y, &b.y, t), width: f32::interpolate(&a.width, &b.width, t), height: f32::interpolate(&a.height, &b.height, t) } } }
