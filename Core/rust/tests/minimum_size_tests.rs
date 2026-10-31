//! Translation of `Tests/Headless/Tests/CoveUISceneTests/Layout/MinimumSizeTests.swift`.
//!
//! Yoga takes a fixed dimension as the flex basis without clamping it to the
//! minimum; a definite-width parent must still grow to the child's minimum.
//!
//! Behaviour checklist:
//! - an authored `min_width`/`min_height` raises a smaller fixed size and grows
//!   the padded container, at any root width
//! - a touch-target floor does the same without touching the authored minimum
//! - removing the floor restores the authored size and the container shrinks
//!
//! The floor is transient: `measure` and `arrange` clamp against
//! `effective_minimum()` without rewriting the authored style, so setting a floor
//! raises the painted size and clearing it restores the authored value — the same
//! clamp-at-the-Yoga-write rule Swift applies in `writeWidthStyle`.

use cove_core::{Rect, Size};
use cove_core::scene::{Align, Direction, Edge, FlexDirection, LayoutNode};

fn layout(
    fixed_width: bool,
    configure: impl FnOnce(&LayoutNode, &LayoutNode),
) -> (Rect, Rect) {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Column);
    root.set_align_items(Align::FlexStart);
    let container = LayoutNode::new();
    container.set_flex_direction(FlexDirection::Column);
    container.set_align_items(Align::FlexStart);
    container.set_padding(16.0, Edge::All);
    if fixed_width {
        container.set_width(Some(390.0));
    }
    let leaf = LayoutNode::new();
    leaf.set_width(Some(18.0));
    leaf.set_height(Some(18.0));
    configure(&container, &leaf);
    root.add_child(&container);
    container.add_child(&leaf);
    root.calculate_layout(Some(600.0), Some(400.0), Direction::Ltr);
    (container.frame(), leaf.frame())
}

#[test]
fn authored_minimum_grows_parent() {
    for fixed_width in [false, true] {
        let (container, leaf) = layout(fixed_width, |_container, leaf| {
            leaf.set_min_width(Some(44.0));
            leaf.set_min_height(Some(44.0));
        });
        assert_eq!(
            leaf.size(),
            Size { width: 44.0, height: 44.0 },
            "leaf must be raised to its authored minimum (fixed_width={fixed_width})"
        );
        assert!(
            (container.height - 76.0).abs() < 0.01,
            "the padded container must grow to 16 + 44 + 16, got {} (fixed_width={fixed_width})",
            container.height
        );
    }
}

/// The floor raises the leaf to 44x44 and the padded container to 76.
#[test]
fn touch_floor_grows_parent() {
    for fixed_width in [false, true] {
        let (container, leaf) = layout(fixed_width, |_container, leaf| {
            leaf.set_minimum_size_floor(Some(Size {
                width: 44.0,
                height: 44.0,
            }));
        });
        assert_eq!(
            leaf.size(),
            Size {
                width: 44.0,
                height: 44.0
            },
            "the floor must raise the leaf to 44x44 (fixed_width={fixed_width})"
        );
        assert!(
            (container.height - 76.0).abs() < 0.01,
            "the padded container must grow to 76, got {} (fixed_width={fixed_width})",
            container.height
        );
    }
}

/// Clearing the floor returns to the authored 18x18 and a 50pt container,
/// because the floor is transient and never bakes into the authored style.
#[test]
fn removing_floor_restores_authored_size() {
    let (container, leaf) = layout(true, |_container, leaf| {
        leaf.set_minimum_size_floor(Some(Size {
            width: 44.0,
            height: 44.0,
        }));
        leaf.set_minimum_size_floor(None);
    });
    assert_eq!(
        leaf.size(),
        Size {
            width: 18.0,
            height: 18.0
        },
        "removing the floor must restore the authored 18x18"
    );
    assert!(
        (container.height - 50.0).abs() < 0.01,
        "the container must shrink back to 16 + 18 + 16, got {}",
        container.height
    );
}

/// A floor is a `max` against the authored minimum, never a replacement, so a
/// larger authored minimum still wins.
#[test]
fn floor_never_lowers_an_authored_minimum() {
    let (_container, leaf) = layout(true, |_container, leaf| {
        leaf.set_min_width(Some(60.0));
        leaf.set_minimum_size_floor(Some(Size {
            width: 44.0,
            height: 44.0,
        }));
    });
    assert_eq!(leaf.width, 60.0, "the larger authored minimum wins");
}
