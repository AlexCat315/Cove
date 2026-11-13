//! Authored CSS Grid drives Taffy directly: named areas, `fr`/fixed tracks,
//! gaps, spans and explicit cells all resolve to real frames.

use crate::support;

use cove_composition::{AnyView, EmptyView, LayoutDebugExt, TupleView, View};
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::prelude::*;

const EPSILON: f32 = 0.5;

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

/// A zero-content leaf sized by the frame it carries, so a grid assertion only
/// measures the track it was placed in.
fn cell(width: Option<f32>, height: Option<f32>, name: &str) -> AnyView {
    let leaf = boxed(AnyView::new(EmptyView));
    let leaf = match (width, height) {
        (Some(width), Some(height)) => AnyView::new(leaf.frame_size(width, height)),
        (Some(width), None) => AnyView::new(leaf.frame_width(width)),
        (None, Some(height)) => AnyView::new(leaf.frame_height(height)),
        (None, None) => leaf,
    };
    AnyView::new(leaf.debug_name(name))
}

fn container(content: Vec<AnyView>) -> AnyView {
    boxed(AnyView::new(TupleView::new(content)))
}

fn frame(scene: &support::InteractiveScene, name: &str) -> (f32, f32, f32, f32) {
    let node: Node = scene.named(name).unwrap_or_else(|| panic!("{name} is mounted"));
    let frame = node.frame();
    (frame.x, frame.y, frame.width, frame.height)
}

fn assert_frame(context: &str, actual: (f32, f32, f32, f32), expected: (f32, f32, f32, f32)) {
    let closed = |a: f32, b: f32| (a - b).abs() <= EPSILON;
    assert!(
        closed(actual.0, expected.0)
            && closed(actual.1, expected.1)
            && closed(actual.2, expected.2)
            && closed(actual.3, expected.3),
        "{context}: expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn named_areas_place_every_cell_on_explicit_lines() {
    support::InteractiveScene::run(400.0, 300.0, |scene| {
        let areas = GridAreaTemplate::new(&["header header", "main side"]).expect("rectangular");
        let spec = GridSpec::areas(areas)
            .columns([GridTrack::fr(1.0).into(), GridTrack::points(120.0).into()])
            .rows([GridTrack::points(40.0).into(), GridTrack::fr(1.0).into()])
            .gap(10.0);
        let header = spec.cell("header").expect("header is declared");
        let main = spec.cell("main").expect("main is declared");
        let side = spec.cell("side").expect("side is declared");

        let view = container(vec![
            AnyView::new(cell(None, Some(40.0), "header").grid_cell(header)),
            AnyView::new(cell(None, None, "main").grid_cell(main)),
            AnyView::new(cell(None, None, "side").grid_cell(side)),
        ])
        .frame_height(300.0)
        .grid(spec);

        scene.install(view);

        // 400 wide minus one 10pt gap leaves 390: the fixed track keeps 120 and
        // the `fr` track takes the remaining 270. Rows: 40 fixed, then 300 - 40
        // - 10 = 250 for the `fr` row.
        assert_frame("header spans both columns", frame(scene, "header"), (0.0, 0.0, 400.0, 40.0));
        assert_frame("main fills the free column", frame(scene, "main"), (0.0, 50.0, 270.0, 250.0));
        assert_frame("side sits after the gap", frame(scene, "side"), (280.0, 50.0, 120.0, 250.0));
    });
}

#[test]
fn areas_alone_imply_auto_tracks() {
    support::InteractiveScene::run(320.0, 200.0, |scene| {
        let areas = GridAreaTemplate::new(&["left right"]).expect("rectangular");
        let spec = GridSpec::areas(areas).gap(20.0);
        let left = spec.cell("left").expect("left is declared");
        let right = spec.cell("right").expect("right is declared");
        let view = container(vec![
            AnyView::new(cell(Some(60.0), Some(30.0), "left").grid_cell(left)),
            AnyView::new(cell(Some(60.0), Some(30.0), "right").grid_cell(right)),
        ])
        .grid(spec);
        scene.install(view);

        // The template alone creates two auto tracks, so the second cell lands
        // after the 20pt gap instead of beside the first in a single track.
        assert_frame("left track", frame(scene, "left"), (0.0, 0.0, 60.0, 30.0));
        assert_frame("right track", frame(scene, "right"), (80.0, 0.0, 60.0, 30.0));
    });
}

#[test]
fn spans_and_explicit_cells_follow_css_placement() {
    support::InteractiveScene::run(320.0, 200.0, |scene| {
        let spec = GridSpec::new([
            GridTrack::fr(1.0).into(),
            GridTrack::fr(1.0).into(),
            GridTrack::fr(1.0).into(),
        ])
        .gap(10.0);
        let view = container(vec![
            AnyView::new(cell(None, Some(20.0), "wide").grid_span(2, 1)),
            AnyView::new(cell(None, Some(20.0), "pinned").grid_cell(GridCell::at(3, 1))),
            AnyView::new(cell(None, Some(20.0), "tall").grid_cell(GridCell::spanning(1, 2, 1, 1))),
        ])
        .grid(spec);
        scene.install(view);

        // 320 wide, two 10pt gaps: each `fr` track is 100.
        assert_frame("a two-track span", frame(scene, "wide"), (0.0, 0.0, 210.0, 20.0));
        assert_frame("an explicit third column", frame(scene, "pinned"), (220.0, 0.0, 100.0, 20.0));
        assert_frame(
            "a row span starts on its explicit row",
            frame(scene, "tall"),
            (0.0, 30.0, 100.0, 20.0),
        );
    });
}

#[test]
fn justify_items_centers_without_touching_the_track() {
    support::InteractiveScene::run(200.0, 100.0, |scene| {
        let spec = GridSpec::new([GridTrack::points(200.0).into()]).justify_items(Align::Center);
        let view = container(vec![AnyView::new(cell(Some(60.0), Some(20.0), "badge"))]).grid(spec);
        scene.install(view);
        assert_frame(
            "a centered item keeps its own width",
            frame(scene, "badge"),
            (70.0, 0.0, 60.0, 20.0),
        );
    });
}
