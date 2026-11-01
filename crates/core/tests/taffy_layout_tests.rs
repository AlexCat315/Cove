use cove_core::scene::*;
use cove_core::{Rect, Size};
use std::rc::Rc;

fn leaf(width: f32, height: f32) -> LayoutNode {
    let node = LayoutNode::new();
    node.set_width(Some(width));
    node.set_height(Some(height));
    node
}
fn close(actual: f32, expected: f32) { assert!((actual - expected).abs() < 0.01, "{actual} != {expected}"); }

#[test]
fn calc_reflows_on_resize_and_style_changes() {
    let root = LayoutNode::new();
    let child = leaf(1.0, 20.0);
    child.set_width_value(LayoutStyleValue::calc(100.0, -24.0));
    root.add_child(&child);
    for width in [200.0, 400.0, 120.0] {
        root.calculate_layout(Some(width), None, Direction::Ltr);
        close(child.frame().width, width - 24.0);
    }
    child.set_width_value(LayoutStyleValue::calc(50.0, 8.0));
    assert!(root.subtree_is_dirty());
    root.calculate_layout(Some(200.0), None, Direction::Ltr);
    close(child.frame().width, 108.0);
    child.set_width(Some(30.0));
    root.calculate_layout(Some(200.0), None, Direction::Ltr);
    close(child.frame().width, 30.0);
}

#[test]
fn expressions_resolve_for_root_edges_gap_and_constraints() {
    let root = LayoutNode::new();
    root.set_flex_direction(FlexDirection::Row);
    root.set_padding_value(LayoutStyleValue::calc(5.0, 5.0), Edge::All);
    root.set_margin_value(LayoutStyleValue::calc(2.0, 1.0), Edge::Left);
    root.set_gap_value(LayoutStyleValue::calc(10.0, 2.0), Gutter::Column);
    let first = leaf(20.0, 10.0);
    let second = leaf(1000.0, 10.0);
    second.set_max_width_value(LayoutStyleValue::calc(50.0, -5.0));
    root.add_child(&first); root.add_child(&second);
    root.calculate_layout(Some(200.0), None, Direction::Ltr);
    close(root.resolved_padding().left, 15.0);
    close(root.resolved_margin().left, 5.0);
    close(second.frame().x, 54.0); // 15 + 20 + (170 * .1 + 2)
    close(second.frame().width, 80.0);
    first.set_min_width_value(LayoutStyleValue::calc(50.0, -10.0));
    first.set_minimum_size_floor(Some(Size { width: 90.0, height: 0.0 }));
    root.calculate_layout(Some(200.0), None, Direction::Ltr);
    close(first.frame().width, 90.0);
    first.set_minimum_size_floor(None);
    root.calculate_layout(Some(200.0), None, Direction::Ltr);
    close(first.frame().width, 75.0);
}

#[test]
fn calc_absolute_insets_and_height() {
    let root = LayoutNode::new();
    let child = leaf(20.0, 20.0);
    child.set_position_type(PositionType::Absolute);
    child.set_position_value(LayoutStyleValue::calc(50.0, -10.0), Edge::Start);
    child.set_height_value(LayoutStyleValue::calc(50.0, -5.0));
    root.add_child(&child);
    root.calculate_layout(Some(200.0), Some(100.0), Direction::Ltr);
    close(child.frame().x, 90.0); close(child.frame().height, 45.0);
    root.calculate_layout(Some(300.0), Some(200.0), Direction::Rtl);
    close(child.frame().x, 140.0); close(child.frame().height, 95.0);
}

#[test]
fn native_grid_tracks_spans_rtl_and_invalidation() {
    let root = LayoutNode::new(); root.set_display(Display::Grid);
    root.set_grid_style(GridStyle { columns: vec![GridTrack::fr(1.0).into(), GridTrack::fr(2.0).into()], ..Default::default() });
    root.set_gap(10.0, Gutter::Column); root.set_gap(5.0, Gutter::Row);
    let children: Vec<_> = (0..3).map(|_| { let n = LayoutNode::new(); n.set_height(Some(20.0)); root.add_child(&n); n }).collect();
    children[2].set_grid_style(GridStyle { column: [GridPlacement::Line(1.into()), GridPlacement::Span(2)], ..Default::default() });
    root.calculate_layout(Some(310.0), None, Direction::Ltr);
    assert_eq!(children[0].frame(), Rect::new(0.0, 0.0, 100.0, 20.0));
    assert_eq!(children[1].frame(), Rect::new(110.0, 0.0, 200.0, 20.0));
    assert_eq!(children[2].frame(), Rect::new(0.0, 25.0, 310.0, 20.0));
    root.calculate_layout(Some(310.0), None, Direction::Rtl);
    close(children[0].frame().x, 210.0);
    root.edit_style(|style| style.grid.columns = vec![GridTrack::fr(1.0).into(); 2]);
    root.calculate_layout(Some(310.0), None, Direction::Ltr);
    close(children[0].frame().width, 150.0);
}

#[test]
fn grid_auto_repeat_and_calc_tracks() {
    let root = LayoutNode::new(); root.set_display(Display::Grid);
    root.set_grid_style(GridStyle { columns: vec![GridTemplateTrack::Repeat {
        count: GridRepeat::AutoFill, tracks: vec![GridTrack::length(LayoutStyleValue::calc(0.0, 80.0))],
    }], ..Default::default() });
    root.set_gap(10.0, Gutter::All);
    let children: Vec<_> = (0..4).map(|_| { let n = leaf(20.0, 20.0); root.add_child(&n); n }).collect();
    root.calculate_layout(Some(270.0), None, Direction::Ltr);
    close(children[2].frame().x, 180.0); close(children[3].frame().y, 30.0);
    root.calculate_layout(Some(170.0), None, Direction::Ltr);
    close(children[2].frame().x, 0.0); close(children[2].frame().y, 30.0);
}

#[test]
fn content_keywords_request_distinct_intrinsic_measurements() {
    let root = LayoutNode::new(); root.set_align_items(Align::FlexStart);
    let child = LayoutNode::new();
    child.set_measure_func(Some(Rc::new(|width, mode, _, _| Size { width: match mode {
        MeasureMode::MinContent => 40.0,
        MeasureMode::Undefined => 160.0,
        _ => width.clamp(40.0, 160.0),
    }, height: 20.0 })));
    root.add_child(&child);
    for (value, expected) in [(LayoutStyleValue::MinContent, 40.0), (LayoutStyleValue::MaxContent, 160.0),
        (LayoutStyleValue::FitContent, 100.0), (LayoutStyleValue::FitContentLimit(LayoutCalc::new(50.0, 10.0)), 60.0)] {
        child.set_width_value(value);
        root.calculate_layout(Some(100.0), None, Direction::Ltr);
        close(child.frame().width, expected);
    }
}

#[test]
fn calc_inside_custom_layout_probe() {
    let root = LayoutNode::new();
    root.set_custom_layout(Rc::new(|width, _, children| children[0].measure_for_custom_layout(width, None)),
        Rc::new(|bounds, children| children[0].place_for_custom_layout(cove_core::Point::new(0.0, 0.0), Some(bounds.width), None, true)));
    let child = leaf(1.0, 20.0); child.set_width_value(LayoutStyleValue::calc(100.0, -12.0)); root.add_child(&child);
    root.calculate_layout(Some(200.0), None, Direction::Ltr);
    close(child.frame().width, 188.0);
}
