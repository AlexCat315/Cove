//! Translation of `Tests/CoveUIComposeTests/Scrolling/KeyboardFocusTests.swift`.
//!
//! Behaviour checklist:
//! - a non-overflowing viewport is not a keyboard stop
//! - `focusNext` skips it and lands on the next real control
//! - once the content overflows the viewport, the scroll view becomes a stop
//! - adding a focusable child control takes the stop away again and releases
//!   focus, because the child already provides an entrance
//! - `focusNext` then lands on that child
//! - removing the child restores the stop; shrinking the content removes it
//!   again and clears focus
//!
//! Model difference: the Swift harness mutates `@State` and re-settles. Here the
//! two pieces of state live in [`State`] and are exposed through
//! `dynamic_properties`, so writing them recomposes the tree exactly as Swift's
//! `@State` does.

use cove_composition::Scope;
mod support;

use cove_composition::{AnyView, TupleView, View, ViewExt};
use cove_core::State;
use cove_core::scene::{
    Align, FlexDirection, FocusChainHolder, InteractionRegistryHolder, Justify,
    PointerCaptureHolder, Wrap,
};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::scroll_view::{ScrollAxis, ScrollView};

use support::GlobalTestLock;

/// The Swift harness: a read-only scroll view over a block of text, optionally
/// overflowing, optionally containing one interactive child.
struct Harness {
    tall: State<bool>,
    interactive: State<bool>,
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let tall = self.tall.get();
        let interactive = self.interactive.get();

        let content = if interactive {
            AnyView::new(TupleView::new(vec![
                sized(20.0, 200.0),
                AnyView::new(
                    AnyView::new(Button::title("Open reading", || {}))
                        .accessibility_label("Open reading"),
                ),
            ]))
        } else {
            AnyView::new(sized(if tall { 400.0 } else { 20.0 }, 200.0))
        };

        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    ScrollView::new(ScrollAxis::Vertical, content)
                        .accessibility_label("Readings")
                        .frame_width(200.0)
                        .frame_height(100.0),
                ),
                AnyView::new(
                    AnyView::new(Button::title("After readings", || {}))
                        .accessibility_label("After readings"),
                ),
            ])),
        ))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("tall", &self.tall),
            cove_composition::DynamicField::state("interactive", &self.interactive),
        ]
    }
}

fn sized(height: f32, width: f32) -> AnyView {
    AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        )
        .frame(
            Some(FrameDimension::Points(width)),
            Some(FrameDimension::Points(height)),
            None,
            None,
            None,
            None,
        ),
    )
}

fn collect(node: &cove_core::scene::Node, into: &mut Vec<cove_core::scene::Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all_nodes(graph: &cove_composition::ViewGraph) -> Vec<cove_core::scene::Node> {
    let mut all = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut all);
    }
    all
}

fn labelled(graph: &cove_composition::ViewGraph, label: &str) -> cove_core::scene::Node {
    let nodes = all_nodes(graph);
    if let Some(found) = nodes.iter().find(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.label == label)
    }) {
        return found.clone();
    }
    let dump: Vec<String> = nodes
        .iter()
        .map(|node| {
            format!(
                "{:?}/{:?}/focusable={}",
                node.id(),
                node.accessibility().map(|s| (s.role, s.label)),
                node.is_focusable()
            )
        })
        .collect();
    panic!("node {label} is mounted; nodes = {dump:#?}");
}

struct Holders;

impl Holders {
    fn new() -> Self {
        InteractionRegistryHolder::set_current(Some(std::rc::Rc::new(
            cove_core::scene::InteractionRegistry::new(),
        )));
        PointerCaptureHolder::set_current(Some(std::rc::Rc::new(
            cove_core::scene::PointerCapture::new(),
        )));
        FocusChainHolder::set_current(Some(std::rc::Rc::new(cove_core::scene::FocusChain::new())));
        Self
    }
}

impl Drop for Holders {
    fn drop(&mut self) {
        InteractionRegistryHolder::set_current(None);
        PointerCaptureHolder::set_current(None);
        FocusChainHolder::set_current(None);
    }
}

fn settle(graph: &cove_composition::ViewGraph) {
    for _ in 0..4 {
        graph.recomposer.commit_all();
        graph.compute_layout(300.0, 600.0);
    }
}

#[test]
fn only_an_overflowing_viewport_without_child_controls_enters_the_keyboard_chain() {
    GlobalTestLock::locked(|| {
        let _holders = Holders::new();
        let tall = State::new(false);
        let interactive = State::new(false);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Harness {
                tall: tall.clone(),
                interactive: interactive.clone(),
            }),
            Default::default(),
        );
        settle(&graph);

        let focus = FocusChainHolder::current().expect("focus chain");
        let scroll = labelled(&graph, "Readings");
        let after = labelled(&graph, "After readings");
        assert!(
            !scroll.is_focusable(),
            "a short, non-overflowing viewport is not a keyboard stop"
        );

        let root = graph.tree.borrow().root().expect("root");
        focus.focus_next(&root);
        assert!(
            focus
                .focused()
                .is_some_and(|focused| focused.id() == after.id()),
            "focusNext skips the short viewport and lands on the next control"
        );

        tall.set(true);
        settle(&graph);
        let scroll = labelled(&graph, "Readings");
        assert!(
            scroll.is_focusable(),
            "an overflowing read-only viewport becomes a keyboard stop"
        );

        focus.focus(Some(&scroll), false);
        assert!(
            focus
                .focused()
                .is_some_and(|focused| focused.id() == scroll.id()),
            "the scroll view can hold focus"
        );

        interactive.set(true);
        settle(&graph);
        let scroll = labelled(&graph, "Readings");
        assert!(
            !scroll.is_focusable(),
            "a child control already provides the entrance, so the stop is removed"
        );
        assert!(
            focus.focused().is_none(),
            "losing its stop releases focus from the scroll view"
        );

        let root = graph.tree.borrow().root().expect("root");
        focus.focus_next(&root);
        let open = labelled(&graph, "Open reading");
        assert!(
            focus
                .focused()
                .is_some_and(|focused| focused.id() == open.id()),
            "focusNext lands on the child control instead"
        );

        interactive.set(false);
        settle(&graph);
        let scroll = labelled(&graph, "Readings");
        assert!(
            scroll.is_focusable(),
            "removing the child restores the keyboard stop"
        );
        focus.focus(Some(&scroll), false);

        tall.set(false);
        settle(&graph);
        let scroll_after = labelled(&graph, "Readings");
        assert!(
            !scroll_after.is_focusable(),
            "shrinking the content below the viewport removes the stop again"
        );
        assert!(focus.focused().is_none(), "and clears focus");
        assert_eq!(
            scroll_after.id(),
            scroll.id(),
            "the scroll view survives every state change as the same node"
        );
    });
}
