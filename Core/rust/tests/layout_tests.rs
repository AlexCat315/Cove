//! Translation of `Tests/CoveUIRuntimeTests/LayoutTests.swift`.
//!
//! Behaviour checklist:
//! - a row lays children out horizontally without overlap; a column stacks them
//! - `flex_grow` fills the remaining space and two growers split it equally
//! - `LayoutPass` writes the computed frames back onto `Node`
//! - padding and border offset child positions
//! - `flex_wrap` wraps children onto the next line
//! - an absolute child lands at its specified coordinates
//! - `flex_basis` sets the initial main-axis size
//! - a column gap spaces siblings
//! - a left margin pushes a child away from its sibling
//! - `remove_child` detaches a node from the layout tree
//! - `align_self` overrides the parent's `align_items`
//! - RTL reverses row order

use cove_core::scene::{
    Align, Direction, Edge, FlexDirection, Gutter, LayoutNode, LayoutPass, Node, PositionType,
    Wrap,
};

fn sized(width: f32, height: f32) -> LayoutNode {
    let node = LayoutNode::new();
    node.set_width(Some(width));
    node.set_height(Some(height));
    node
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1.0
}

#[test]
fn row_layout() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_width(Some(200.0));
    root.set_height(Some(100.0));

    let a = sized(80.0, 100.0);
    let b = sized(80.0, 100.0);
    root.add_child(&a);
    root.add_child(&b);
    root.calculate_layout(None, None, Direction::Ltr);

    let (fa, fb) = (a.frame(), b.frame());
    assert!(fa.x < fb.x, "a is left of b");
    assert!(fa.max_x() <= fb.x, "the row does not overlap");
}

#[test]
fn column_layout() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Column);
    root.set_width(Some(100.0));
    root.set_height(Some(200.0));

    let a = sized(100.0, 60.0);
    let b = sized(100.0, 60.0);
    root.add_child(&a);
    root.add_child(&b);
    root.calculate_layout(None, None, Direction::Ltr);

    let (fa, fb) = (a.frame(), b.frame());
    assert!(fa.y < fb.y, "a is above b");
    assert!(fa.max_y() <= fb.y, "the column does not overlap");
}

#[test]
fn flex_grow_fills_row() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_width(Some(200.0));
    root.set_height(Some(100.0));

    let fixed = sized(60.0, 100.0);
    let grow = LayoutNode::new();
    grow.set_flex_grow(1.0);
    grow.set_height(Some(100.0));
    root.add_child(&fixed);
    root.add_child(&grow);
    root.calculate_layout(None, None, Direction::Ltr);

    assert!(close(grow.frame().width, 140.0), "200 - 60 = 140");
}

#[test]
fn flex_grow_equal_split() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_width(Some(300.0));
    root.set_height(Some(100.0));

    let a = LayoutNode::new();
    a.set_flex_grow(1.0);
    a.set_height(Some(100.0));
    let b = LayoutNode::new();
    b.set_flex_grow(1.0);
    b.set_height(Some(100.0));
    root.add_child(&a);
    root.add_child(&b);
    root.calculate_layout(None, None, Direction::Ltr);

    assert!(close(a.frame().width, 150.0));
    assert!(close(b.frame().width, 150.0));
}

#[test]
fn layout_pass_writes_frames() {
    let root_node = Node::new();
    let child_a = Node::new();
    let child_b = Node::new();
    root_node.add_child(&child_a);
    root_node.add_child(&child_b);

    let root_layout = LayoutNode::new();
    root_layout.set_flex_direction(FlexDirection::Row);
    root_layout.set_width(Some(200.0));
    root_layout.set_height(Some(100.0));

    let la = sized(80.0, 100.0);
    let lb = sized(80.0, 100.0);
    root_layout.add_child(&la);
    root_layout.add_child(&lb);

    LayoutPass::run(&root_layout, &root_node, Some(200.0), Some(100.0));

    assert_eq!(child_a.frame().width, 80.0);
    assert_eq!(child_b.frame().width, 80.0);
    assert!(child_a.frame().x < child_b.frame().x);
}

#[test]
fn padding_offsets_children() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_width(Some(200.0));
    root.set_height(Some(100.0));
    root.set_padding(10.0, Edge::All);

    let child = sized(50.0, 80.0);
    root.add_child(&child);
    root.calculate_layout(None, None, Direction::Ltr);

    assert!(child.frame().x >= 10.0, "the child clears the leading padding");
    assert!(child.frame().y >= 10.0, "the child clears the top padding");
}

#[test]
fn flex_wrap_wraps() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_flex_wrap(Wrap::Wrap);
    root.set_width(Some(100.0));
    root.set_height(Some(200.0));

    let a = sized(60.0, 40.0);
    let b = sized(60.0, 40.0);
    root.add_child(&a);
    root.add_child(&b);
    root.calculate_layout(None, None, Direction::Ltr);

    assert!(b.frame().y > a.frame().y, "b wraps to the second line");
}

#[test]
fn absolute_position() {
    let root = LayoutNode::new();
    root.set_width(Some(200.0));
    root.set_height(Some(200.0));

    let child = sized(50.0, 50.0);
    child.set_position_type(PositionType::Absolute);
    child.set_position(10.0, Edge::Left);
    child.set_position(20.0, Edge::Top);
    root.add_child(&child);
    root.calculate_layout(None, None, Direction::Ltr);

    assert!(close(child.frame().x, 10.0));
    assert!(close(child.frame().y, 20.0));
}

#[test]
fn flex_basis_sets_size() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_width(Some(300.0));
    root.set_height(Some(100.0));

    let a = LayoutNode::new();
    a.set_flex_basis(100.0);
    a.set_height(Some(100.0));
    let b = LayoutNode::new();
    b.set_flex_grow(1.0);
    b.set_height(Some(100.0));
    root.add_child(&a);
    root.add_child(&b);
    root.calculate_layout(None, None, Direction::Ltr);

    assert!(close(a.frame().width, 100.0));
    assert!(close(b.frame().width, 200.0));
}

#[test]
fn gap_spacing() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_width(Some(200.0));
    root.set_height(Some(100.0));
    root.set_gap(10.0, Gutter::Column);

    let a = sized(50.0, 100.0);
    let b = sized(50.0, 100.0);
    root.add_child(&a);
    root.add_child(&b);
    root.calculate_layout(None, None, Direction::Ltr);

    assert!(close(b.frame().x, 60.0), "b starts at a's trailing edge plus the gap");
}

#[test]
fn border_offsets_child() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Column);
    root.set_width(Some(100.0));
    root.set_height(Some(100.0));
    root.set_border(5.0, Edge::All);

    let child = sized(50.0, 50.0);
    root.add_child(&child);
    root.calculate_layout(None, None, Direction::Ltr);

    assert!(child.frame().x >= 5.0);
    assert!(child.frame().y >= 5.0);
}

#[test]
fn margin_between_siblings() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_width(Some(300.0));
    root.set_height(Some(100.0));

    let a = sized(50.0, 100.0);
    let b = sized(50.0, 100.0);
    b.set_margin(20.0, Edge::Left);
    root.add_child(&a);
    root.add_child(&b);
    root.calculate_layout(None, None, Direction::Ltr);

    assert!(close(b.frame().x, 70.0), "b starts at a's trailing edge plus the margin");
}

#[test]
fn remove_child_works() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_width(Some(200.0));
    root.set_height(Some(100.0));

    let a = sized(80.0, 100.0);
    let b = sized(80.0, 100.0);
    root.add_child(&a);
    root.add_child(&b);
    root.remove_child(&a);
    root.calculate_layout(None, None, Direction::Ltr);

    assert_eq!(root.children().len(), 1);
    assert!(close(b.frame().x, 0.0), "the survivor moves to the origin");
}

#[test]
fn align_self_override() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_align_items(Align::FlexStart);
    root.set_width(Some(200.0));
    root.set_height(Some(100.0));

    let child = sized(50.0, 30.0);
    child.set_align_self(Align::Center);
    root.add_child(&child);
    root.calculate_layout(None, None, Direction::Ltr);

    assert!(
        close(child.frame().y, 35.0),
        "(100 - 30) / 2 = 35, centred in the container rather than the line",
    );
}

#[test]
fn align_items_center_uses_the_container_extent() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_align_items(Align::Center);
    root.set_width(Some(200.0));
    root.set_height(Some(100.0));

    let child = sized(50.0, 30.0);
    root.add_child(&child);
    root.calculate_layout(None, None, Direction::Ltr);

    assert!(
        close(child.frame().y, 35.0),
        "a lone non-stretched child centres in the 100-high container",
    );
}

#[test]
fn rtl_direction() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_direction(Direction::Rtl);
    root.set_width(Some(200.0));
    root.set_height(Some(100.0));

    let a = sized(50.0, 100.0);
    let b = sized(50.0, 100.0);
    root.add_child(&a);
    root.add_child(&b);
    root.calculate_layout(None, None, Direction::Rtl);

    assert!(close(a.frame().x, 150.0), "in RTL the first child sits at the trailing edge");
    assert!(close(b.frame().x, 100.0), "the second child precedes it");
}

#[test]
fn logical_end_mirrors_under_rtl_when_set_before_parenting() {
    // A view that writes `Edge::End` before it has a parent must still mirror
    // once the surrounding direction is known. The avatar status indicator is
    // exactly this shape: it positions itself absolutely in `make_layout_node`,
    // long before `layout_direction(Rtl)` reaches the subtree.
    for (direction, expected) in [(Direction::Ltr, 72.0f32), (Direction::Rtl, 8.0f32)] {
        let root = LayoutNode::new();
        root.set_flex_direction(FlexDirection::Row);
        root.set_width(Some(100.0));
        root.set_height(Some(100.0));
        let child = LayoutNode::new();
        child.set_position_type(PositionType::Absolute);
        child.set_position(8.0, Edge::End);
        child.set_width(Some(20.0));
        child.set_height(Some(20.0));
        root.add_child(&child);
        root.calculate_layout(Some(100.0), Some(100.0), direction);
        let x = child.frame().x;
        assert!(
            (x - expected).abs() < 1.0,
            "{direction:?}: expected x={expected}, got {x}"
        );
    }
}

#[test]
fn per_node_layout_direction_reverses_row_order() {
    // A node's own `layout_direction` must reach its children's placement, not
    // just be readable through `direction()`. This is the contract the whole
    // RTL story rests on: Sidebar, Toolbar, DataTable frozen columns and the
    // avatar status indicator all depend on it.
    for (direction, first_x, second_x) in
        [(Direction::Ltr, 0.0f32, 44.0f32), (Direction::Rtl, 44.0f32, 0.0f32)]
    {
        let root = LayoutNode::new();
        root.set_flex_direction(FlexDirection::Row);
        root.set_direction(direction);
        root.set_width(Some(88.0));
        root.set_height(Some(44.0));
        let first = LayoutNode::new();
        first.set_width(Some(44.0));
        first.set_height(Some(44.0));
        let second = LayoutNode::new();
        second.set_width(Some(44.0));
        second.set_height(Some(44.0));
        root.add_child(&first);
        root.add_child(&second);
        // The root itself is the node carrying the direction, so the pass starts
        // from the harness default and the override must come from the style.
        root.calculate_layout(Some(88.0), Some(44.0), Direction::Ltr);
        assert!(
            (first.frame().x - first_x).abs() < 1.0,
            "{direction:?} first child: expected x={first_x}, got {}",
            first.frame().x
        );
        assert!(
            (second.frame().x - second_x).abs() < 1.0,
            "{direction:?} second child: expected x={second_x}, got {}",
            second.frame().x
        );
    }
}
