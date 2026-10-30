//! Translation of `Tests/CoveUIComposeTests/ZStackTests.swift`.
//!
//! Behaviour checklist:
//! - a ZStack measures to its largest child and aligns the rest
//! - `TopLeading` places both children at the origin; the default centers them
//! - semantic leading follows the layout direction in RTL
//! - later children receive pointer input first (topmost wins hit testing)
//! - content stays inside its own Box instead of a padded sibling

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, TupleView, View, ViewExt};
use cove_core::scene::{Align, EventDispatcher, FlexDirection, Justify, Node, Wrap};
use cove_platform::{InputEvent, MouseButton, MouseButtonEvent, PointerKind};
use cove_ui_compose::{BoxView, Button, ZStack};

use support::{GlobalTestLock, TestScene};

/// A bare box with an explicit size, the Swift `Box {}.frame(...)` shape.
fn sized(width: f32, height: f32, name: &str) -> AnyView {
    AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
        )
        .frame_width(width)
        .frame_height(height)
        .debug_name(name),
    )
}

fn unsized_box() -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
    ))
}

#[test]
fn measures_and_aligns() {
    GlobalTestLock::with_scheduler(|| {
        let graph = support::graph();
        let stack = ZStack::new(
            Align::TopLeading,
            AnyView::new(TupleView::new(vec![
                sized(100.0, 40.0, "large"),
                sized(20.0, 20.0, "small"),
            ])),
        )
        .debug_name("stack");
        graph.install(stack, Default::default());
        graph.compute_layout(300.0, 200.0);

        let stack_node = named(&graph, "stack");
        assert_eq!(
            (stack_node.frame().width, stack_node.frame().height),
            (100.0, 40.0),
            "the stack takes its largest child's size"
        );
        assert_eq!(named(&graph, "large").frame().x, 0.0);
        assert_eq!(named(&graph, "large").frame().y, 0.0);
        assert_eq!(
            (named(&graph, "small").frame().x, named(&graph, "small").frame().y),
            (0.0, 0.0),
            "TopLeading places the small child at the origin"
        );

        // The default alignment centers the small child within the large one.
        let stack = ZStack::new(
            Align::Center,
            AnyView::new(TupleView::new(vec![
                unsized_box().frame_width(100.0).frame_height(40.0),
                sized(20.0, 20.0, "small"),
            ])),
        )
        .debug_name("stack");
        graph.install(stack, Default::default());
        graph.compute_layout(300.0, 200.0);
        assert_eq!(
            (named(&graph, "small").frame().x, named(&graph, "small").frame().y),
            (40.0, 10.0),
            "a centered child sits at the midpoint of the remaining space"
        );
    });
}

#[test]
fn rtl_alignment() {
    GlobalTestLock::with_scheduler(|| {
        let graph = support::graph();
        let stack = ZStack::new(
            Align::TopLeading,
            AnyView::new(TupleView::new(vec![
                unsized_box().frame_width(100.0).frame_height(40.0),
                sized(20.0, 20.0, "small"),
            ])),
        )
        .debug_name("stack")
        .layout_direction(cove_core::scene::Direction::Rtl);
        graph.install(stack, Default::default());
        graph.compute_layout(300.0, 200.0);
        assert_eq!(
            (named(&graph, "small").frame().x, named(&graph, "small").frame().y),
            (80.0, 0.0),
            "RTL flips semantic leading to the right edge"
        );
    });
}

#[test]
fn topmost_child_wins_hit_testing() {
    TestScene::run(|scene| {
        let hits: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let front_hits = hits.clone();
        let back_hits = hits.clone();
        let front = Button::new(unsized_box(), move || {
            front_hits.borrow_mut().push("front")
        });
        let back = Button::new(unsized_box(), move || {
            back_hits.borrow_mut().push("back")
        });
        let stack = ZStack::new(
            Align::Center,
            AnyView::new(TupleView::new(vec![
                AnyView::new(back.frame_width(100.0).frame_height(100.0)),
                AnyView::new(front.frame_width(100.0).frame_height(100.0)),
            ])),
        )
        .frame_width(100.0)
        .frame_height(100.0);
        scene.graph.install(stack, Default::default());
        scene.graph.compute_layout(200.0, 200.0);

        let event = MouseButtonEvent {
            button: MouseButton::Left,
            x: 50.0,
            y: 50.0,
            clicks: 1,
            modifiers: Default::default(),
            kind: PointerKind::default(),
        };
        let dispatcher = EventDispatcher::new(
            scene.graph.tree.clone(),
            scene.interactions.clone(),
            scene.pointer_capture.clone(),
            scene.focus_chain.clone(),
            0,
        );
        dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
        scene.graph.recomposer.commit_all();
        dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
        assert_eq!(
            hits.borrow().as_slice(),
            ["front"],
            "the topmost child consumes the click"
        );
    });
}

#[test]
fn card_content_stays_inside_its_box() {
    TestScene::run(|scene| {
        support::with_test_text_environment(|| {
            let label = cove_ui_compose::Text::new("Top-left layer").debug_name("label");
            let accent = BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(label),
            )
            .frame_width(120.0)
            .frame_height(72.0)
            .debug_name("accent");
            let stack = ZStack::new(
                Align::TopLeading,
                AnyView::new(TupleView::new(vec![AnyView::new(accent)])),
            )
            .frame_width(320.0)
            .frame_height(180.0);
            scene.graph.install(stack, Default::default());
            scene.graph.compute_layout(320.0, 180.0);

            let accent_frame = scene.named("accent").expect("accent node").absolute_frame();
            let label_frame = scene.named("label").expect("label node").absolute_frame();
            assert!(label_frame.min_x() >= accent_frame.min_x());
            assert!(label_frame.max_x() <= accent_frame.max_x());
            assert!(label_frame.min_y() >= accent_frame.min_y());
            assert!(label_frame.max_y() <= accent_frame.max_y());
        });
    });
}

fn named(graph: &Rc<cove_composition::ViewGraph>, name: &str) -> Node {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut collected);
    }
    collected
        .into_iter()
        .find(|node| node.debug_name().as_deref() == Some(name))
        .unwrap_or_else(|| panic!("node {name} exists"))
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}
