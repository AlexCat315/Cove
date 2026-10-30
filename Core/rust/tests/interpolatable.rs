//! Translation of `Tests/CoveUIRuntimeTests/InterpolatableTests.swift`.
//!
//! Behaviour checklist:
//! - f32 / f64 interpolation hits both ends and the midpoint
//! - colour interpolation lerps each channel independently, including a
//!   fractional t
//! - colour endpoints are returned exactly
//! - colour interpolation does **not** premultiply alpha (straight-alpha lerp)
//! - a rect interpolates origin and size component-wise
//! - point and size interpolate component-wise

use cove_core::scene::Interpolatable;
use cove_core::{Color, Point, Rect, Size};

fn close(actual: f32, expected: f32, context: &str) {
    assert!(
        (actual - expected).abs() < 1e-6,
        "{context}: expected {expected}, got {actual}"
    );
}

#[test]
fn float_interp() {
    assert_eq!(f32::interpolate(&0.0, &10.0, 0.0), 0.0);
    assert_eq!(f32::interpolate(&0.0, &10.0, 1.0), 10.0);
    assert_eq!(f32::interpolate(&0.0, &10.0, 0.5), 5.0);
    assert_eq!(f32::interpolate(&-4.0, &4.0, 0.25), -2.0);
}

#[test]
fn double_interp() {
    assert_eq!(f64::interpolate(&0.0, &1.0, 0.5), 0.5);
    assert_eq!(f64::interpolate(&10.0, &20.0, 0.0), 10.0);
    assert_eq!(f64::interpolate(&10.0, &20.0, 1.0), 20.0);
}

#[test]
fn color_interp() {
    let black = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    let white = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    let mid = Color::interpolate(&black, &white, 0.5);
    assert_eq!(mid.r, 0.5);
    assert_eq!(mid.g, 0.5);
    assert_eq!(mid.b, 0.5);
    assert_eq!(mid.a, 0.5);
}

#[test]
fn color_endpoints() {
    let a = Color { r: 0.2, g: 0.3, b: 0.4, a: 0.5 };
    let b = Color { r: 0.8, g: 0.7, b: 0.6, a: 1.0 };
    assert_eq!(Color::interpolate(&a, &b, 0.0), a);
    assert_eq!(Color::interpolate(&a, &b, 1.0), b);
}

#[test]
fn color_straight_alpha() {
    // Straight-alpha semantics: RGB lerps independently of alpha. Premultiplying
    // would pull the red toward zero here, which is what this pins down.
    let opaque = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    let transparent = Color { r: 0.0, g: 0.0, b: 1.0, a: 0.0 };
    let mid = Color::interpolate(&opaque, &transparent, 0.5);
    assert_eq!(mid.r, 0.5);
    assert_eq!(mid.b, 0.5);
    assert_eq!(mid.a, 0.5);
}

#[test]
fn rect_interp() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    let b = Rect::new(100.0, 50.0, 20.0, 40.0);
    let mid = Rect::interpolate(&a, &b, 0.5);
    close(mid.x, 50.0, "origin x");
    close(mid.y, 25.0, "origin y");
    close(mid.width, 15.0, "size width");
    close(mid.height, 25.0, "size height");
}

#[test]
fn point_and_size_interp() {
    let point = Point::interpolate(&Point { x: 0.0, y: 0.0 }, &Point { x: 4.0, y: 8.0 }, 0.25);
    close(point.x, 1.0, "point x");
    close(point.y, 2.0, "point y");

    let size = Size::interpolate(
        &Size { width: 10.0, height: 20.0 },
        &Size { width: 20.0, height: 40.0 },
        0.5,
    );
    close(size.width, 15.0, "size width");
    close(size.height, 30.0, "size height");
}
