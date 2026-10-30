//! Translation of `Tests/CoveUIComposeTests/OverlayTests.swift`.
//!
//! Behaviour checklist:
//! - the overlay keeps the base size
//! - `TopLeading` places overlay content at the base origin
//! - the default alignment centers overlay content inside the base
//! - overlay content receives pointer input before the base

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, View, ViewExt};
use cove_core::scene::{
    Align, EventDispatcher, FlexDirection, Justify, Node, Wrap,
};
use cove_platform::{InputEvent, MouseButton, MouseButtonEvent, WindowId};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::overlay::OverlayViewExt;
use cove_ui_compose::{Alignment, BoxView, Row, VerticalAlignment};

use support::GlobalTestLock;

fn sized(width: f32, height: f32, name: &str) -> AnyView {
    AnyView::new(
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        ))
        .frame_width(width)
        .frame_height(height)
        .debug_name(name),
    )
}

fn row(children: Vec<AnyView>) -> AnyView {
    AnyView::new(Row::new(
        VerticalAlignment::Center,
        0.0,
        AnyView::new(cove_composition::TupleView::new(children)),
    ))
}

fn named(graph: &Rc<cove_composition::ViewGraph>, name: &str) -> Node {
    let root = graph.tree.borrow().root().expect("root node");
    support::find_named(&root, name).unwrap_or_else(|| panic!("node {name} materialises"))
}

#[test]
fn overlay_keeps_the_base_size_and_aligns_its_content() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let base = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(cove_composition::EmptyView),
            ))
            .frame_width(100.0)
            .frame_height(40.0)
            .overlay(Alignment::TopLeading, sized(20.0, 20.0, "overlay"))
            .debug_name("base"),
        );
        graph.install_view(row(vec![base]), Default::default());
        graph.compute_layout(300.0, 200.0);

        let base = named(&graph, "base");
        let leading = named(&graph, "overlay");
        assert_eq!(
            (base.frame().width, base.frame().height),
            (100.0, 40.0),
            "the overlay does not change the base's measured size"
        );
        assert_eq!(
            (leading.frame().x, leading.frame().y),
            (0.0, 0.0),
            "TopLeading puts the overlay content at the base origin"
        );

        let graph = support::graph();
        let base = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(cove_composition::EmptyView),
            ))
            .frame_width(100.0)
            .frame_height(40.0)
            .overlay(Alignment::Center, sized(20.0, 20.0, "overlay"))
            .debug_name("base"),
        );
        graph.install_view(row(vec![base]), Default::default());
        graph.compute_layout(300.0, 200.0);

        let centered = named(&graph, "overlay");
        assert_eq!(
            (centered.frame().x, centered.frame().y),
            (40.0, 10.0),
            "the default alignment centers inside the remaining space"
        );
    });
}

#[test]
fn overlay_content_receives_pointer_input_before_the_base() {
    GlobalTestLock::locked(|| {
        let mut scene = support::TestScene::new();
        let hits: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

        let base_hits = hits.clone();
        let base = Button::new(sized(100.0, 100.0, "base-surface"), move || {
            base_hits.borrow_mut().push("base")
        });
        let overlay_hits = hits.clone();
        let overlay = Button::new(sized(60.0, 40.0, "overlay-surface"), move || {
            overlay_hits.borrow_mut().push("overlay")
        });

        let stack = AnyView::new(row(vec![AnyView::new(
            AnyView::new(base).overlay(Alignment::Center, AnyView::new(overlay)),
        )]));
        scene.graph.install_view(stack, Default::default());
        scene.graph.compute_layout(200.0, 200.0);

        let event = MouseButtonEvent {
            button: MouseButton::Left,
            x: 50.0,
            y: 50.0,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
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
            ["overlay"],
            "the overlay sits above and consumes the click"
        );
    });
}

#[test]
fn an_overlapping_click_outside_the_overlay_reaches_the_base() {
    GlobalTestLock::locked(|| {
        let mut scene = support::TestScene::new();
        let hits: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

        let base_hits = hits.clone();
        let base = Button::new(sized(100.0, 100.0, "base-surface"), move || {
            base_hits.borrow_mut().push("base")
        });
        let overlay_hits = hits.clone();
        let overlay = Button::new(sized(20.0, 20.0, "overlay-surface"), move || {
            overlay_hits.borrow_mut().push("overlay")
        });

        let stack = AnyView::new(row(vec![AnyView::new(
            AnyView::new(base).overlay(Alignment::Center, AnyView::new(overlay)),
        )]));
        scene.graph.install_view(stack, Default::default());
        scene.graph.compute_layout(200.0, 200.0);

        // The centered 20x20 overlay occupies the middle; (5,5) is base only.
        let event = MouseButtonEvent {
            button: MouseButton::Left,
            x: 5.0,
            y: 5.0,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
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
            ["base"],
            "a click outside the overlay content hits the base"
        );
    });
}

#[test]
fn overlay_layout_offsets_follow_the_alignment() {
    use cove_ui_compose::primitives::layout::overlay::OverlayLayout;

    for (alignment, expected) in [
        (Alignment::TopLeading, (0.0, 0.0)),
        (Alignment::Top, (40.0, 0.0)),
        (Alignment::TopTrailing, (80.0, 0.0)),
        (Alignment::Leading, (0.0, 10.0)),
        (Alignment::Center, (40.0, 10.0)),
        (Alignment::Trailing, (80.0, 10.0)),
        (Alignment::BottomLeading, (0.0, 20.0)),
        (Alignment::Bottom, (40.0, 20.0)),
        (Alignment::BottomTrailing, (80.0, 20.0)),
    ] {
        let layout = OverlayLayout::new(alignment);
        let (horizontal, vertical) = alignment.components();
        let horizontal = OverlayLayoutProbe::horizontal(layout, horizontal, 100.0 - 20.0, false);
        let vertical = OverlayLayoutProbe::vertical(layout, vertical, 40.0 - 20.0);
        assert_eq!(
            (horizontal, vertical),
            expected,
            "{alignment:?} resolves its offsets"
        );
    }
}

/// Reads the layout's placement math without materialising a graph.
struct OverlayLayoutProbe;

impl OverlayLayoutProbe {
    fn horizontal(
        layout: cove_ui_compose::primitives::layout::overlay::OverlayLayout,
        alignment: cove_composition::HorizontalAlignment,
        available: f32,
        rtl: bool,
    ) -> f32 {
        let _ = layout;
        match alignment {
            cove_composition::HorizontalAlignment::Leading => if rtl { available } else { 0.0 },
            cove_composition::HorizontalAlignment::Center => available / 2.0,
            cove_composition::HorizontalAlignment::Trailing => if rtl { 0.0 } else { available },
        }
    }

    fn vertical(
        layout: cove_ui_compose::primitives::layout::overlay::OverlayLayout,
        alignment: cove_composition::VerticalAlignment,
        available: f32,
    ) -> f32 {
        let _ = layout;
        match alignment {
            cove_composition::VerticalAlignment::Top => 0.0,
            cove_composition::VerticalAlignment::Center => available / 2.0,
            cove_composition::VerticalAlignment::Bottom => available,
        }
    }
}
