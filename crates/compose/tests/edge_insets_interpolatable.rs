//! Translation of `Tests/CoveUIComposeTests/EdgeInsetsInterpolatableTests.swift`.
//!
//! Behaviour checklist:
//! - each edge interpolates independently
//! - the endpoints are exact (t = 0 returns `from`, t = 1 returns `to`)

mod support;

use cove_core::scene::Interpolatable;
use cove_ui_compose::modifiers::layout::EdgeInsets;

#[test]
fn interpolates_each_edge_independently() {
    let a = EdgeInsets::new(0.0, 0.0, 0.0, 0.0);
    let b = EdgeInsets::new(10.0, 20.0, 30.0, 40.0);
    let mid = EdgeInsets::interpolate(&a, &b, 0.5);
    assert_eq!(mid.top, 5.0);
    assert_eq!(mid.leading, 10.0);
    assert_eq!(mid.bottom, 15.0);
    assert_eq!(mid.trailing, 20.0);
}

#[test]
fn endpoints_are_exact() {
    let a = EdgeInsets::all(4.0);
    let b = EdgeInsets::all(12.0);
    assert_eq!(EdgeInsets::interpolate(&a, &b, 0.0), a);
    assert_eq!(EdgeInsets::interpolate(&a, &b, 1.0), b);
}
