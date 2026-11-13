//! CSS Grid placement parity. Every case asserts the frame the engine actually
//! produces, so the table in `Docs/layout-grid-parity.md` is reproducible from
//! this file rather than from a reading of the spec.

use crate::support;

use cove_composition::{AnyView, EmptyView, LayoutDebugExt, TupleView, View};
use cove_core::scene::{Align, FlexDirection, GridAutoFlow, Justify, Node, Wrap};
use cove_ui_compose::prelude::*;

const WIDTH: f32 = 320.0;
const HEIGHT: f32 = 200.0;
const ITEM_HEIGHT: f32 = 20.0;

fn boxed(content: AnyView) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Row,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        content,
    ))
}

fn item(name: &str) -> AnyView {
    AnyView::new(
        boxed(AnyView::new(EmptyView))
            .frame_height(ITEM_HEIGHT)
            .debug_name(name),
    )
}

fn three_columns(flow: GridAutoFlow) -> GridSpec {
    GridSpec::new([
        GridTrack::fr(1.0).into(),
        GridTrack::fr(1.0).into(),
        GridTrack::fr(1.0).into(),
    ])
    .gap(10.0)
    .flow(flow)
}

fn install(scene: &mut support::InteractiveScene, spec: GridSpec, content: Vec<AnyView>) {
    let view = boxed(AnyView::new(TupleView::new(content)))
        .fill()
        .grid(spec);
    scene.install(view);
}

fn frames(scene: &support::InteractiveScene, names: &[&str]) -> Vec<(f32, f32, f32, f32)> {
    names
        .iter()
        .map(|name| {
            let node: Node = scene
                .named(name)
                .unwrap_or_else(|| panic!("{name} is mounted"));
            let frame = node.frame();
            (frame.x, frame.y, frame.width, frame.height)
        })
        .collect()
}

#[test]
fn sparse_flow_leaves_no_hole() {
    support::InteractiveScene::run(WIDTH, HEIGHT, |scene| {
        install(
            scene,
            three_columns(GridAutoFlow::Row),
            vec![
                AnyView::new(item("a").grid_span(2, 1)),
                AnyView::new(item("b")),
            ],
        );
        let (a, b) = {
            let found = frames(scene, &["a", "b"]);
            (found[0], found[1])
        };
        assert_eq!(
            a,
            (0.0, 0.0, 210.0, 20.0),
            "the span covers two 100pt tracks and a gap"
        );
        assert_eq!(
            b,
            (220.0, 0.0, 100.0, 20.0),
            "the next single item fills the third track"
        );
    });
}

#[test]
fn sparse_flow_wraps_a_span_that_does_not_fit() {
    support::InteractiveScene::run(WIDTH, HEIGHT, |scene| {
        install(
            scene,
            three_columns(GridAutoFlow::Row),
            vec![
                AnyView::new(item("a").grid_span(2, 1)),
                AnyView::new(item("b").grid_span(2, 1)),
            ],
        );
        let found = frames(scene, &["a", "b"]);
        assert_eq!(
            found[0],
            (0.0, 0.0, 210.0, 20.0),
            "the first span takes the first two tracks"
        );
        assert_eq!(
            found[1],
            (0.0, 30.0, 210.0, 20.0),
            "the second span wraps instead of straddling a hole"
        );
    });
}

#[test]
fn a_definite_item_pushes_a_later_span() {
    support::InteractiveScene::run(WIDTH, HEIGHT, |scene| {
        install(
            scene,
            three_columns(GridAutoFlow::Row),
            vec![
                AnyView::new(item("pinned").grid_cell(GridCell::spanning(1, 1, 1, 2))),
                AnyView::new(item("wide").grid_span(2, 1)),
            ],
        );
        let found = frames(scene, &["pinned", "wide"]);
        assert_eq!(
            found[0],
            (0.0, 0.0, 100.0, 20.0),
            "the pinned item keeps its line"
        );
        assert_eq!(
            found[1],
            (110.0, 0.0, 210.0, 20.0),
            "the span starts on the second track because the first is occupied"
        );
    });
}

#[test]
fn dense_flow_backfills_a_hole() {
    support::InteractiveScene::run(WIDTH, HEIGHT, |scene| {
        install(
            scene,
            three_columns(GridAutoFlow::RowDense),
            vec![
                AnyView::new(item("pinned").grid_cell(GridCell::at(2, 1))),
                AnyView::new(item("first")),
                AnyView::new(item("second")),
            ],
        );
        let found = frames(scene, &["pinned", "first", "second"]);
        assert_eq!(found[0], (110.0, 0.0, 100.0, 20.0));
        assert_eq!(
            found[1],
            (0.0, 0.0, 100.0, 20.0),
            "dense flow backfills the empty first track"
        );
        assert_eq!(found[2], (220.0, 0.0, 100.0, 20.0));
    });
}

#[test]
fn a_line_beyond_the_explicit_grid_creates_an_implicit_track() {
    support::InteractiveScene::run(WIDTH, HEIGHT, |scene| {
        install(
            scene,
            three_columns(GridAutoFlow::Row),
            vec![
                AnyView::new(item("first")),
                AnyView::new(item("far").grid_cell(GridCell::at(4, 1))),
            ],
        );
        let found = frames(scene, &["first", "far"]);
        // An implicit track is `auto`, so it sizes to content: the empty item
        // contributes no width and the explicit track keeps its own start.
        // Four tracks means three gaps: the three `fr` tracks share 290pt and
        // the empty implicit track collapses to zero, so the far item lands at
        // the container edge. CSS sizes an empty `auto` track the same way.
        assert_eq!(
            found[0].2,
            290.0 / 3.0,
            "the explicit tracks absorb the implicit gap"
        );
        assert_eq!(
            found[1].0, 320.0,
            "the far item opens an implicit track at the edge"
        );
        assert_eq!(found[1].2, 0.0, "an empty implicit track has no width");
    });
}

#[test]
fn per_item_self_alignment_survives_placement() {
    support::InteractiveScene::run(WIDTH, HEIGHT, |scene| {
        let cell = GridCell::at(1, 1);
        install(
            scene,
            GridSpec::new([GridTrack::points(200.0).into()]),
            vec![AnyView::new(
                boxed(AnyView::new(EmptyView))
                    .frame_size(60.0, 20.0)
                    .debug_name("badge")
                    .grid_cell(cell)
                    .grid_justify_self(Align::FlexEnd),
            )],
        );
        let found = frames(scene, &["badge"]);
        assert_eq!(
            found[0],
            (140.0, 0.0, 60.0, 20.0),
            "justify-self end keeps its width and aligns to the track end"
        );
    });
}

#[test]
fn column_flow_fills_down_then_across() {
    support::InteractiveScene::run(WIDTH, HEIGHT, |scene| {
        let spec = GridSpec::new([
            GridTrack::points(100.0).into(),
            GridTrack::points(100.0).into(),
        ])
        .gap(10.0)
        .flow(GridAutoFlow::Column);
        install(
            scene,
            spec,
            vec![
                AnyView::new(item("a")),
                AnyView::new(item("b")),
                AnyView::new(item("c")),
            ],
        );
        let found = frames(scene, &["a", "b", "c"]);
        // No explicit rows means one implicit row, so column flow runs across
        // and opens a third implicit track; CSS derives the row count the same
        // way. Add `.rows([...])` when items should stack.
        assert_eq!(found[0], (0.0, 0.0, 100.0, 20.0));
        assert_eq!(
            found[1],
            (110.0, 0.0, 100.0, 20.0),
            "the next item takes the second column"
        );
        assert_eq!(
            found[2],
            (220.0, 0.0, 0.0, 20.0),
            "an implicit track has no width"
        );
    });
}

#[test]
fn two_items_pinned_to_one_cell_overlap() {
    support::InteractiveScene::run(WIDTH, HEIGHT, |scene| {
        let spec = GridSpec::new([GridTrack::points(100.0).into()]);
        install(
            scene,
            spec,
            vec![
                AnyView::new(item("one").grid_cell(GridCell::at(1, 1))),
                AnyView::new(item("two").grid_cell(GridCell::at(1, 1))),
            ],
        );
        let found = frames(scene, &["one", "two"]);
        assert_eq!(
            found[0], found[1],
            "definite collisions overlap instead of erroring"
        );
    });
}
