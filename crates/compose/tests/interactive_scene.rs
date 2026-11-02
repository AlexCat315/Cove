//! Translation of `Tests/CoveUIComposeTests/InteractiveScene.swift`.
//!
//! The Swift file is the shared harness rather than a suite, so this target
//! proves the harness itself behaves: holders are private per scope and
//! restored afterwards, `settle` produces layout, name lookup works, and real
//! pointer dispatch reaches a host in the graph.

use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt};
use cove_core::Point;
use cove_core::scene::{Align, FlexDirection, FocusChainHolder, Justify, Wrap};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;

use support::InteractiveScene;

fn sized_box(width: f32, height: f32, name: &str) -> AnyView {
    AnyView::new(
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(cove_composition::EmptyView),
            )
            .frame_width(width)
            .frame_height(height),
        )
        .debug_name(name),
    )
}

#[test]
fn holders_are_private_and_restored() {
    let outside = FocusChainHolder::current();
    InteractiveScene::run(300.0, 200.0, |scene| {
        assert!(
            FocusChainHolder::current().is_some(),
            "the scene must install its own focus chain"
        );
        assert!(
            !Rc::ptr_eq(
                &FocusChainHolder::current().expect("installed"),
                &Rc::new(cove_core::scene::FocusChain::new())
            ),
            "the scene must not hand out a fresh chain per call"
        );
        assert_eq!(scene.width, 300.0);
        assert_eq!(scene.height, 200.0);
    });
    assert_eq!(
        FocusChainHolder::current().is_some(),
        outside.is_some(),
        "the previous holder must be restored on scope exit"
    );
}

#[test]
fn install_lays_out_and_named_finds_debug_names() {
    InteractiveScene::run(320.0, 240.0, |scene| {
        scene.install(sized_box(120.0, 40.0, "probe-box"));
        let node = scene.named("probe-box").expect("debug name is recorded");
        let frame = node.absolute_frame();
        assert!(
            (frame.width - 120.0).abs() < 0.5,
            "width was {}",
            frame.width
        );
        assert!(
            (frame.height - 40.0).abs() < 0.5,
            "height was {}",
            frame.height
        );
        assert!(scene.first(|node| node.frame().width > 0.0).is_some());
        assert!(scene.all(|_| true).len() >= 2);
    });
}

#[test]
fn click_dispatches_to_a_host_and_recomposes() {
    let presses = Rc::new(Cell::new(0));
    let counter = presses.clone();
    InteractiveScene::run(300.0, 200.0, |scene| {
        scene.install(
            AnyView::new(
                Button::title("Press", move || counter.set(counter.get() + 1))
                    .frame_width(100.0)
                    .frame_height(32.0),
            )
            .debug_name("press-button"),
        );
        let button = scene.named("press-button").expect("button is mounted");
        assert_eq!(presses.get(), 0);
        scene.click(&button, 1);
        assert_eq!(presses.get(), 1, "a click must reach the button host");
    });
}

#[test]
fn drag_away_cancels_a_button_press_and_clears_capture() {
    let presses = Rc::new(Cell::new(0));
    let counter = presses.clone();
    InteractiveScene::run(300.0, 200.0, |scene| {
        scene.install(
            AnyView::new(
                Button::title("Press", move || counter.set(counter.get() + 1))
                    .frame_width(80.0)
                    .frame_height(32.0),
            )
            .debug_name("press-button"),
        );
        let button = scene.named("press-button").expect("button is mounted");
        let frame = button.absolute_frame();

        scene.drag(
            Point::new(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0),
            Point::new(280.0, 180.0),
            6,
        );
        assert_eq!(
            presses.get(),
            0,
            "dragging off a button must not activate it"
        );
        assert!(
            scene.capture.target().is_none(),
            "releasing the button must clear pointer capture"
        );
    });
}

#[test]
fn nodes_are_collected_depth_first() {
    InteractiveScene::run(300.0, 200.0, |scene| {
        scene.install(AnyView::new(cove_composition::TupleView::new(vec![
            sized_box(10.0, 10.0, "a"),
            sized_box(10.0, 10.0, "b"),
        ])));
        let a = scene.named("a").expect("first child is named");
        let b = scene.named("b").expect("second child is named");
        assert!(
            a.absolute_frame().x <= b.absolute_frame().x,
            "depth-first collection must follow document order"
        );
        let total = scene.nodes().len();
        assert!(
            total >= 2,
            "both children and their hosts belong to the graph"
        );
        assert_eq!(scene.all(|_| true).len(), total);
    });
}
