//! Translation of `Tests/CoveUIComposeTests/ZStackTests.swift`.
//!
//! Behaviour checklist:
//! - a ZStack measures to its largest child and aligns the rest
//! - `TopLeading` places both children at the origin; the default centers them
//! - semantic leading follows the layout direction in RTL
//! - later children receive pointer input first (topmost wins hit testing)
//! - content stays inside its own Box instead of a padded sibling

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{Alignment, AnyView, LayoutDebugExt, TupleView, ViewGraph};
use cove_core::scene::{Align, Direction, EventDispatcher, FlexDirection, Justify, Node, Wrap};
use cove_platform::{InputEvent, MouseButton, MouseButtonEvent, PointerKind, WindowId};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::zstack::ZStack;
use cove_ui_compose::{BoxView, Row, VerticalAlignment};

use support::GlobalTestLock;

/// A bare box with an explicit size, the Swift `Box {}.frame(...)` shape.
fn sized(width: f32, height: f32, name: &str) -> AnyView {
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

/// The Swift `Row { ... }` wrapper the ZStack fixtures install into.
fn row(children: Vec<AnyView>) -> AnyView {
    AnyView::new(Row::new(
        VerticalAlignment::Center,
        0.0,
        AnyView::new(TupleView::new(children)),
    ))
}

fn unsized_box() -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(cove_composition::EmptyView),
    ))
}

#[test]
fn measures_and_aligns() {
    GlobalTestLock::with_scheduler(|| {
        let graph = support::graph();
        // Swift installs `Row { ZStack { ... } }`. The `Row` matters: a stack
        // installed directly at the root is stretched to the viewport's width by
        // the root's stretch-fit, so it would measure 300 instead of its largest
        // child's 100. Inside a row it is measured as content.
        let stack = AnyView::new(row(vec![AnyView::new(
            AnyView::new(ZStack::new(
                Alignment::TopLeading,
                AnyView::new(TupleView::new(vec![
                    sized(100.0, 40.0, "large"),
                    sized(20.0, 20.0, "small"),
                ])),
            ))
            .debug_name("stack"),
        )]));
        graph.install_view(stack, Default::default());
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
            (
                named(&graph, "small").frame().x,
                named(&graph, "small").frame().y
            ),
            (0.0, 0.0),
            "TopLeading places the small child at the origin"
        );

        // The default alignment centers the small child within the large one.
        let stack = AnyView::new(row(vec![AnyView::new(
            AnyView::new(ZStack::new(
                Alignment::Center,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(unsized_box().frame_width(100.0).frame_height(40.0)),
                    sized(20.0, 20.0, "small"),
                ])),
            ))
            .debug_name("stack"),
        )]));
        graph.install_view(stack, Default::default());
        graph.compute_layout(300.0, 200.0);
        assert_eq!(
            (
                named(&graph, "small").frame().x,
                named(&graph, "small").frame().y
            ),
            (40.0, 10.0),
            "a centered child sits at the midpoint of the remaining space"
        );
    });
}

#[test]
fn rtl_alignment() {
    GlobalTestLock::with_scheduler(|| {
        let graph = support::graph();
        // The direction lives on the stack, so TopLeading resolves to the
        // trailing edge and the small layer sits at the right margin.
        // Apply the modifier to the `ZStack` itself: `AnyView::new(...)` erases the
        // concrete type, and a modifier applied to the erased value attaches to a
        // wrapper node rather than to the stack's own layout node — so the
        // children would still read the inherited direction.
        let stack = AnyView::new(row(vec![AnyView::new(
            ZStack::new(
                Alignment::TopLeading,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(unsized_box().frame_width(100.0).frame_height(40.0)),
                    sized(20.0, 20.0, "small"),
                ])),
            )
            .layout_direction(Direction::Rtl),
        )]));
        graph.install_view(stack, Default::default());
        graph.compute_layout(300.0, 200.0);
        assert_eq!(
            (
                named(&graph, "small").frame().x,
                named(&graph, "small").frame().y
            ),
            (80.0, 0.0),
            "RTL flips semantic leading to the right edge"
        );
    });
}

#[test]
fn topmost_child_wins_hit_testing() {
    GlobalTestLock::with_scheduler(|| {
        let scene = support::TestScene::new();
        let hits: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let front_hits = hits.clone();
        let back_hits = hits.clone();
        let front = Button::new(unsized_box(), move || front_hits.borrow_mut().push("front"));
        let back = Button::new(unsized_box(), move || back_hits.borrow_mut().push("back"));
        let stack = AnyView::new(
            AnyView::new(ZStack::new(
                Alignment::Center,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(back.frame_width(100.0).frame_height(100.0)),
                    AnyView::new(front.frame_width(100.0).frame_height(100.0)),
                ])),
            ))
            .frame_width(100.0)
            .frame_height(100.0),
        );
        scene.graph.install_view(stack, Default::default());
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
            WindowId::MAIN,
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
    GlobalTestLock::with_scheduler(|| {
        support::with_test_text_environment(|| {
            let scene = support::TestScene::new();
            let label = AnyView::new(
                AnyView::new(cove_ui_compose::primitives::text::Text::new(
                    "Top-left layer",
                ))
                .debug_name("label"),
            );
            let accent = AnyView::new(
                AnyView::new(
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::Center,
                        Wrap::NoWrap,
                        0.0,
                        label,
                    ))
                    .frame_width(120.0)
                    .frame_height(72.0),
                )
                .debug_name("accent"),
            );
            let stack = AnyView::new(
                AnyView::new(
                    AnyView::new(ZStack::new(
                        Alignment::TopLeading,
                        AnyView::new(TupleView::new(vec![accent])),
                    ))
                    .frame_width(320.0)
                    .frame_height(180.0),
                )
                .debug_name("stack"),
            );
            scene.graph.install_view(stack, Default::default());
            scene.graph.compute_layout(320.0, 180.0);

            let accent_frame = scene.named("accent").expect("accent node").absolute_frame();
            let label_frame = scene.named("label").expect("label node").absolute_frame();
            assert!(label_frame.x >= accent_frame.x);
            assert!(label_frame.max_x() <= accent_frame.max_x());
            assert!(label_frame.y >= accent_frame.y);
            assert!(label_frame.max_y() <= accent_frame.max_y());
        });
    });
}

fn named(graph: &Rc<ViewGraph>, name: &str) -> Node {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut collected);
    }
    collected
        .into_iter()
        .find(|node| support::debug_name(node).as_deref() == Some(name))
        .unwrap_or_else(|| panic!("node {name} exists"))
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}
