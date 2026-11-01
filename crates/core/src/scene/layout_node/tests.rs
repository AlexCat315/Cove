use super::*;
use crate::Rect;

#[test]
fn column_stack_sums_heights_and_stretches_cross_axis() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Column);
    root.set_align_items(Align::Stretch);
    let first = LayoutNode::new();
    first.set_height(Some(40.0));
    let second = LayoutNode::new();
    second.set_height(Some(60.0));
    root.add_child(&first);
    root.add_child(&second);
    root.calculate_layout(Some(200.0), None, Direction::Ltr);
    assert_eq!(first.frame(), Rect::new(0.0, 0.0, 200.0, 40.0));
    assert_eq!(second.frame(), Rect::new(0.0, 40.0, 200.0, 60.0));
    assert_eq!(root.frame().height, 100.0);
}

#[test]
fn grow_shares_free_space() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_width(Some(300.0));
    root.set_height(Some(50.0));
    let fixed = LayoutNode::new();
    fixed.set_width(Some(100.0));
    fixed.set_height(Some(50.0));
    let grow = LayoutNode::new();
    grow.set_flex_grow(1.0);
    grow.set_height(Some(50.0));
    root.add_child(&fixed);
    root.add_child(&grow);
    root.calculate_layout(Some(300.0), Some(50.0), Direction::Ltr);
    assert_eq!(fixed.frame().width, 100.0);
    assert_eq!(grow.frame().width, 200.0);
    assert_eq!(grow.frame().x, 100.0);
}

#[test]
fn padding_and_gap_offset_children() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_padding(10.0, Edge::All);
    root.set_gap(5.0, Gutter::All);
    let first = LayoutNode::new();
    first.set_width(Some(20.0));
    first.set_height(Some(20.0));
    let second = LayoutNode::new();
    second.set_width(Some(20.0));
    second.set_height(Some(20.0));
    root.add_child(&first);
    root.add_child(&second);
    root.calculate_layout(Some(100.0), Some(100.0), Direction::Ltr);
    assert_eq!(first.frame().x, 10.0);
    assert_eq!(second.frame().x, 35.0);
    assert_eq!(root.resolved_padding().left, 10.0);
}

#[test]
fn measure_callback_sizes_a_leaf() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Column);
    let leaf = LayoutNode::new();
    let measure: MeasureFunc = Rc::new(|width, width_mode, _height, _height_mode| {
        let natural: f32 = 80.0;
        Size {
            width: if width_mode == MeasureMode::AtMost {
                natural.min(width)
            } else {
                natural
            },
            height: 18.0,
        }
    });
    leaf.set_measure_func(Some(measure));
    root.add_child(&leaf);
    root.calculate_layout(Some(50.0), None, Direction::Ltr);
    assert_eq!(leaf.frame().height, 18.0);
    assert_eq!(leaf.frame().width, 50.0);
}
