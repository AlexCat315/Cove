mod support;
use cove_composition::{AnyView, CssGrid, EmptyView, LayoutDebugExt, TupleView};
use cove_core::scene::{GridStyle, GridTrack, LayoutCalc, LayoutStyleValue as Value, Node};
use cove_ui_compose::modifiers::layout::{FrameValues, LayoutFrameExt, LayoutValuesExt};
use cove_ui_compose::primitives::text::Text;
use support::GlobalTestLock;

fn named(node: &Node, name: &str) -> Option<Node> {
    if support::debug_name(node).as_deref() == Some(name) { return Some(node.clone()); }
    node.children().iter().find_map(|child| named(child, name))
}
fn box_view() -> cove_composition::BoxView {
    cove_composition::BoxView::new(cove_core::scene::FlexDirection::Column, cove_core::scene::Align::Stretch,
        cove_core::scene::Justify::FlexStart, cove_core::scene::Wrap::NoWrap, 0.0, AnyView::new(EmptyView))
}
#[test]
fn expression_modifiers_reflow_on_viewport_changes() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let view = box_view().frame_values(FrameValues {
            width: Some(Value::calc(100.0, -24.0)), height: Some(Value::Points(20.0)),
            max_width: Some(Value::Calc(LayoutCalc::new(50.0, 0.0).with_minimum(80.0))),
            ..Default::default()
        }).debug_name("expression");
        graph.install_view(AnyView::new(view), Default::default());
        for (width, expected) in [(200.0, 100.0), (300.0, 150.0)] {
            graph.compute_layout(width, 100.0);
            let root = graph.tree.borrow().root().unwrap();
            assert_eq!(named(&root, "expression").unwrap().frame().width, expected);
        }
    });
}
#[test]
fn css_grid_uses_fractional_tracks_in_the_retained_tree() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let content = TupleView::new(vec![
            AnyView::new(box_view().frame_height(20.0).debug_name("one")),
            AnyView::new(box_view().frame_height(30.0).debug_name("two")),
        ]);
        let mut grid = CssGrid::new(GridStyle {
            columns: vec![GridTrack::fr(1.0).into(), GridTrack::fr(2.0).into()], ..Default::default()
        }, AnyView::new(content));
        grid.column_gap = Value::calc(0.0, 10.0);
        graph.install_view(AnyView::new(grid), Default::default());
        graph.compute_layout(310.0, 100.0);
        let root = graph.tree.borrow().root().unwrap();
        assert_eq!(named(&root, "one").unwrap().frame().width, 100.0);
        assert_eq!(named(&root, "two").unwrap().frame().x, 110.0);
        assert_eq!(named(&root, "two").unwrap().frame().width, 200.0);
    });
}
#[test]
fn text_min_content_measures_words_instead_of_characters() {
    GlobalTestLock::locked(|| support::with_test_text_environment(|| {
        let graph = support::graph();
        let content = TupleView::new(vec![
            AnyView::new(Text::new("Hello world").frame(Some(Value::MinContent), None, None, None, None, None).debug_name("minimum")),
            AnyView::new(Text::new("Hello world").frame(Some(Value::MaxContent), None, None, None, None, None).debug_name("maximum")),
            AnyView::new(Text::new("world").frame(Some(Value::MaxContent), None, None, None, None, None).debug_name("word")),
        ]);
        let mut container = box_view(); container.content = AnyView::new(content);
        graph.install_view(AnyView::new(container), Default::default());
        graph.compute_layout(300.0, 200.0);
        let root = graph.tree.borrow().root().unwrap();
        let minimum = named(&root, "minimum").unwrap().frame();
        let maximum = named(&root, "maximum").unwrap().frame();
        let word = named(&root, "word").unwrap().frame();
        assert!(word.width > 0.0);
        assert!(minimum.width >= word.width - 0.01);
        assert!(minimum.width < maximum.width);
        assert!(minimum.height > maximum.height);
    }));
}
