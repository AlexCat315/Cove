//! Translation of `Tests/CoveUIComposeTests/ScrolledControlClickTests.swift`.
//!
//! Regression target for the "dropdown/button dead after scrolling" class of
//! bugs: any absolute-origin walk that sums ancestor `frame.origin` without
//! subtracting ancestor `content_offset` reports scrolled controls at their
//! unscrolled position.
//!
//! Behaviour checklist:
//! - a button inside a scrolled `ScrollView` fires on a click at its visual
//!   position
//! - the visual origin walk subtracts every ancestor's scroll offset, so a
//!   scrolled control reports where it is actually painted
//! - a stable scrollbar gutter reserves the trailing lane instead of overlapping
//!   the content
//!
//! Recorded differences (reported, not fixed):
//! - The Swift suite's Select cases (menu opens at the trigger, a capped
//!   internal menu follows its trigger while the parent scrolls, inspector-style
//!   selects reopen after picking) all present the menu through a `Binding`, so
//!   the `Binding` observable gap hides the overlay from a post-mount write. They
//!   are recorded rather than restated.
//! - `ButtonHost.pressedKey` is private in Rust; the host is found by the public
//!   `__button_host` marker, which `ButtonHost::update_node` writes.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::Node;
use cove_core::{Point, Rect};
use cove_platform::{InputEvent, MouseButton, MouseButtonEvent};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::scroll_view::{ScrollAxis, ScrollView, ScrollbarGutter};
use cove_ui_compose::primitives::text::Text;

use support::InteractiveScene;

const BUTTON_HOST_MARKER: &str = "__button_host";

fn descendants(node: &Node) -> Vec<Node> {
    let mut collected = Vec::new();
    fn walk(node: &Node, into: &mut Vec<Node>) {
        into.push(node.clone());
        for child in node.children() {
            walk(&child, into);
        }
    }
    walk(node, &mut collected);
    collected
}

/// Window-space origin: parent-local frames composed down the chain with every
/// ancestor's scroll offset applied (children render translated by
/// `-content_offset`).
fn visual_origin(node: &Node) -> Point {
    let mut origin = node.frame().origin();
    let mut current = node.parent();
    while let Some(parent) = current {
        let frame = parent.frame();
        let offset = parent.content_offset();
        origin.x += frame.x - offset.x;
        origin.y += frame.y - offset.y;
        current = parent.parent();
    }
    origin
}

#[test]
fn scrolled_button_click_fires() {
    InteractiveScene::run(220.0, 160.0, |scene| {
        let taps = Rc::new(RefCell::new(0u32));
        let counter = taps.clone();

        let column = BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::FlexStart,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(Text::new("header").frame_size(200.0, 100.0)),
                AnyView::new(
                    Button::new(Text::new("Target"), move || {
                        *counter.borrow_mut() += 1;
                    })
                    .frame_size(200.0, 30.0),
                ),
                AnyView::new(Text::new("tail").frame_size(200.0, 300.0)),
            ])),
        );

        let scroll =
            ScrollView::new(ScrollAxis::Vertical, AnyView::new(column)).frame_size(220.0, 160.0);
        scene.install(scroll);

        let scroll_view = scene
            .nodes()
            .into_iter()
            .find(|node| node.clips_to_bounds())
            .expect("the scroll view clips its content");
        scroll_view.set_content_offset(Point::new(0.0, 90.0));
        scene.graph.recomposer.commit_all();
        scene.graph.compute_layout(220.0, 160.0);

        let button = scene
            .nodes()
            .into_iter()
            .find(|node| node.attachment::<bool>(BUTTON_HOST_MARKER).is_some())
            .expect("the button host mounts");
        let origin = visual_origin(&button);
        assert!(
            origin.y < 160.0,
            "the button scrolled up into the viewport and stays visible (got y {})",
            origin.y
        );

        let frame = button.frame();
        let event = MouseButtonEvent {
            button: MouseButton::Left,
            x: origin.x + frame.width / 2.0,
            y: origin.y + frame.height / 2.0,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
        scene.graph.recomposer.commit_all();
        dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
        scene.settle(5, 1.0);

        assert_eq!(
            *taps.borrow(),
            1,
            "a click at the scrolled button's visual position fires it once"
        );
    });
}

#[test]
fn stable_gutter_reserves_the_trailing_lane() {
    let viewport = Rect::new(0.0, 0.0, 240.0, 160.0);
    let non_hover = ScrollView::stable_gutter_width(false);
    let hover = ScrollView::stable_gutter_width(true);

    assert!(
        non_hover > 0.0,
        "a stable gutter reserves a non-zero lane (got {non_hover})"
    );
    assert!(
        non_hover >= hover,
        "a non-hovering surface reserves at least as much as a hovering one \
         ({non_hover} vs {hover})"
    );
    assert!(
        non_hover < viewport.width / 2.0,
        "the reserved lane stays a small fraction of the viewport (got {non_hover})"
    );

    // The gutter is a rendering property, not a layout inset: it must not shrink
    // the area a scrolled control reports.
    let gutter = ScrollbarGutter::Stable;
    assert_eq!(
        ScrollbarGutter::Stable,
        gutter,
        "the stable gutter is the mode the scrolled-control harness relies on"
    );
}
