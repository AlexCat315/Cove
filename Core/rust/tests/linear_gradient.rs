//! Translation of `Tests/CoveUIRuntimeTests/LinearGradientTests.swift`.
//!
//! Behaviour checklist:
//! - CSS angles put the start colour on the edge the ramp leaves from
//!   (90° = left→right, 180° = top→bottom, 135° = top-left→bottom-right)
//! - the midpoint of a diagonal ramp is a 50/50 mix
//! - a rounded gradient fill keeps its coverage fringe and colours every vertex
//!   by its position
//! - `multiplied_alpha` scales both stops

use cove_core::{Color, DrawList, LinearGradient, Point, Rect};

fn red() -> Color {
    Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    }
}

fn blue() -> Color {
    Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    }
}

fn rect() -> Rect {
    Rect::new(10.0, 20.0, 100.0, 40.0)
}

fn same(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 0.001
        && (a.g - b.g).abs() < 0.001
        && (a.b - b.b).abs() < 0.001
        && (a.a - b.a).abs() < 0.001
}

#[test]
fn angles() {
    // 90° ramps left to right.
    let across = LinearGradient {
        start: red(),
        end: blue(),
        angle: 90.0,
    };
    assert!(same(
        across.color_at(Point { x: 10.0, y: 40.0 }, rect()),
        red()
    ));
    assert!(same(
        across.color_at(Point { x: 110.0, y: 40.0 }, rect()),
        blue()
    ));

    // 180° ramps top to bottom.
    let down = LinearGradient {
        start: red(),
        end: blue(),
        angle: 180.0,
    };
    assert!(same(
        down.color_at(Point { x: 60.0, y: 20.0 }, rect()),
        red()
    ));
    assert!(same(
        down.color_at(Point { x: 60.0, y: 60.0 }, rect()),
        blue()
    ));

    // 135° ramps from the top-left corner to the bottom-right.
    let diagonal = LinearGradient {
        start: red(),
        end: blue(),
        angle: 135.0,
    };
    assert!(same(
        diagonal.color_at(Point { x: 10.0, y: 20.0 }, rect()),
        red()
    ));
    assert!(same(
        diagonal.color_at(Point { x: 110.0, y: 60.0 }, rect()),
        blue()
    ));

    let middle = diagonal.color_at(Point { x: 60.0, y: 40.0 }, rect());
    assert!(
        (middle.r - 0.5).abs() < 0.001,
        "midpoint red is half, got {}",
        middle.r
    );
    assert!(
        (middle.b - 0.5).abs() < 0.001,
        "midpoint blue is half, got {}",
        middle.b
    );
}

#[test]
fn drawing() {
    let mut list = DrawList::default();
    let gradient = LinearGradient {
        start: red(),
        end: blue(),
        angle: 90.0,
    };
    list.add_linear_gradient(rect(), 8.0, gradient)
        .expect("rounded gradient fill");
    assert!(
        !list.vertices().is_empty(),
        "the gradient produced geometry"
    );

    let mut saw_transparent_fringe = false;
    for vertex in list.vertices() {
        let coverage = (vertex.color >> 24) as f32 / 255.0;
        if vertex.color >> 24 == 0 {
            // The anti-aliased edge of the rounded rect contributes zero
            // coverage; those vertices are fully transparent.
            saw_transparent_fringe = true;
            continue;
        }
        let expected = gradient
            .color_at(
                Point {
                    x: vertex.x,
                    y: vertex.y,
                },
                rect(),
            )
            .multiplied_alpha(coverage);
        assert_eq!(
            vertex.color,
            expected.rgba8(),
            "vertex at ({}, {}) carries the gradient colour for its position",
            vertex.x,
            vertex.y
        );
    }
    assert!(
        saw_transparent_fringe,
        "a rounded fill keeps the transparent coverage fringe"
    );

    // Opacity applies to both stops.
    let faded = gradient.multiplied_alpha(0.5);
    assert_eq!(faded.start.a, 0.5);
    assert_eq!(faded.end.a, 0.5);
}
