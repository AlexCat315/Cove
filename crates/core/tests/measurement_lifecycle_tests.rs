//! Translation of
//! `Tests/Headless/Tests/CoveUISceneTests/Layout/MeasurementLifecycleTests.swift`.
//!
//! A measure callback only applies to a childless node, so adding a child has to
//! retire it and removing the last child has to restore it. The dangerous half
//! is the restore: the callback must come back for the reason it was installed,
//! and must not come back once the subtree is genuinely empty of the state that
//! justified it.
//!
//! Behaviour checklist:
//! - a reused container restores its pending measurement after its last child
//!   leaves, and re-measures with the callback in force
//! - a container that never had a measurement does not invent one when a leaf
//!   child leaves, and lays out at zero
//! - a custom layout keeps measurement while authored children come and go, and
//!   a replaced custom layout takes over the size
//!
//! A custom layout installs a real measure callback, so `has_measure_func` is
//! true for it even with authored children. That is what makes `mark_dirty` work
//! on a custom-layout node instead of silently doing nothing.

use cove_core::Size;
use cove_core::scene::{Direction, LayoutNode, MeasureMode};

fn fixed(width: f32, height: f32) -> LayoutNode {
    let node = LayoutNode::new();
    node.set_width(Some(width));
    node.set_height(Some(height));
    node
}

#[test]
fn leaf_container_round_trip() {
    let node = LayoutNode::new();
    node.set_align_items(cove_core::scene::Align::FlexStart);
    node.set_measure_func(Some(std::rc::Rc::new(
        |_width: f32, _width_mode: MeasureMode, _height: f32, _height_mode: MeasureMode| Size {
            width: 40.0,
            height: 20.0,
        },
    )));
    let first = fixed(60.0, 30.0);
    let second = fixed(10.0, 10.0);
    node.add_child(&first);
    node.add_child(&second);
    assert!(
        !node.has_measure_func(),
        "a node with children cannot measure itself"
    );

    node.mark_dirty();
    node.calculate_layout(None, None, Direction::Ltr);
    assert_eq!(
        node.frame().size(),
        Size {
            width: 60.0,
            height: 40.0
        },
        "the container sizes to its tallest child stack"
    );

    node.set_measure_func(Some(std::rc::Rc::new(
        |_width: f32, _width_mode: MeasureMode, _height: f32, _height_mode: MeasureMode| Size {
            width: 70.0,
            height: 25.0,
        },
    )));
    assert!(
        !node.has_measure_func(),
        "replacing the callback does not make it effective while children remain"
    );

    node.remove_child(&first);
    assert!(
        !node.has_measure_func(),
        "one child still remains, so the measure callback stays retired"
    );

    node.remove_child(&second);
    assert!(
        node.has_measure_func(),
        "the last child left, so the pending measurement comes back"
    );

    node.calculate_layout(None, None, Direction::Ltr);
    assert_eq!(
        node.frame().size(),
        Size {
            width: 70.0,
            height: 25.0
        },
        "the restored callback is the current one, not the retired initial one"
    );
}

#[test]
fn stale_measurement_is_discarded() {
    let node = LayoutNode::new();
    let child = LayoutNode::new();
    node.set_measure_func(Some(std::rc::Rc::new(
        |_width: f32, _width_mode: MeasureMode, _height: f32, _height_mode: MeasureMode| Size {
            width: 40.0,
            height: 20.0,
        },
    )));
    node.add_child(&child);
    node.remove_child(&child);
    assert!(
        !node.has_measure_func(),
        "a never-installed measurement must not be invented by removing a child"
    );
    node.calculate_layout(None, None, Direction::Ltr);
    assert_eq!(
        node.frame().size(),
        Size {
            width: 0.0,
            height: 0.0
        },
        "with no children and no measurement the node collapses"
    );
}

#[test]
fn custom_layout_keeps_measurement() {
    let node = LayoutNode::new();
    let child = fixed(60.0, 30.0);
    node.add_child(&child);
    node.set_custom_layout(
        std::rc::Rc::new(
            |_width: Option<f32>, _height: Option<f32>, children: &[LayoutNode]| Size {
                width: children.len() as f32 * 40.0,
                height: 20.0,
            },
        ),
        std::rc::Rc::new(|_rect: cove_core::Rect, _children: &[LayoutNode]| {}),
    );
    assert!(
        node.has_measure_func(),
        "a custom layout measures even with authored children"
    );
    node.calculate_layout(None, None, Direction::Ltr);
    assert_eq!(
        node.frame().size(),
        Size {
            width: 40.0,
            height: 20.0
        },
        "one child measures 1 * 40 wide"
    );

    let second = LayoutNode::new();
    node.add_child(&second);
    assert!(
        node.has_measure_func(),
        "adding an authored child does not retire a custom layout's measurement"
    );
    node.calculate_layout(None, None, Direction::Ltr);
    assert_eq!(
        node.frame().size(),
        Size {
            width: 80.0,
            height: 20.0
        },
        "two children measure 2 * 40 wide"
    );

    node.remove_child(&child);
    assert!(
        node.has_measure_func(),
        "removing an authored child keeps the custom layout measuring"
    );
    node.calculate_layout(None, None, Direction::Ltr);
    assert_eq!(
        node.frame().size(),
        Size {
            width: 40.0,
            height: 20.0
        },
        "one remaining child measures 1 * 40 wide"
    );

    node.set_custom_layout(
        std::rc::Rc::new(
            |_width: Option<f32>, _height: Option<f32>, _children: &[LayoutNode]| Size {
                width: 90.0,
                height: 35.0,
            },
        ),
        std::rc::Rc::new(|_rect: cove_core::Rect, _children: &[LayoutNode]| {}),
    );
    node.calculate_layout(None, None, Direction::Ltr);
    assert_eq!(
        node.frame().size(),
        Size {
            width: 90.0,
            height: 35.0
        },
        "the replacement custom layout takes over the computed size"
    );
}

/// `mark_dirty` is gated on `has_measure_func`, so a custom layout has to be
/// invalidatable through it: a custom layout's measurement depends on its
/// children, so a change to them must be able to force a re-measure.
#[test]
fn custom_layout_mark_dirty_invalidates() {
    let node = LayoutNode::new();
    let child = fixed(60.0, 30.0);
    node.add_child(&child);
    node.set_custom_layout(
        std::rc::Rc::new(
            |_width: Option<f32>, _height: Option<f32>, children: &[LayoutNode]| Size {
                width: children.len() as f32 * 40.0,
                height: 20.0,
            },
        ),
        std::rc::Rc::new(|_rect: cove_core::Rect, _children: &[LayoutNode]| {}),
    );
    node.calculate_layout(None, None, Direction::Ltr);
    assert!(
        !node.subtree_is_dirty(),
        "a completed layout pass leaves the subtree clean"
    );
    node.mark_dirty();
    assert!(
        node.subtree_is_dirty(),
        "mark_dirty must invalidate a custom-layout node, not silently do nothing"
    );
}
