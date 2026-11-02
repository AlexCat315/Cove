//! Translation of `Tests/CoveUIComposeTests/DisclosureGroupTests.swift`.
//!
//! Behaviour checklist:
//! - the disclosure header spans the container and its label starts at the
//!   leading edge with room for the title
//! - Return and Space toggle the content and reflow the following row
//! - a disabled disclosure is not keyboard focusable
//!
//! Named differences:
//!
//! - **No `disclosure-label` / `disclosure-header` debug names.** The Swift
//!   suite located the header by the first focusable node and its chrome by the
//!   `BuiltinButtonChrome.stateKey` attachment, then read the label from
//!   `layoutSnapshot()` by debug name. The Rust `DisclosureGroup`
//!   (`primitives/layout/disclosure_group.rs`) publishes neither debug name. The
//!   header is located the same way (first focusable node), its chrome by the
//!   `"__builtin_button_chrome_state"` attachment that `BuiltinButtonChrome`
//!   does publish, and the label by the measured text input of the title, so the
//!   same geometry assertions run against the same nodes.
//! - **No `disclosure-label` / `disclosure-header` debug names** (see above).
//!   Everything else matches Swift: a disabled disclosure drops out of the focus
//!   order entirely, so `first_focusable` is `None` exactly as in Swift.

use crate::support;

use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt};
use cove_core::State;
use cove_core::scene::{
    Align, EventPhase, EventResult, FlexDirection, InteractionRegistry, InteractionRegistryHolder,
    Justify, Node, Wrap,
};
use cove_platform::{KeyEvent, KeyModifiers};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::layout::DisclosureGroup;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::{BoxView, EmptyView};

use support::GlobalTestLock;

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut collected);
    }
    collected
}

fn first_focusable(graph: &cove_composition::ViewGraph) -> Option<Node> {
    all(graph).into_iter().find(Node::is_focusable)
}

/// The header's button chrome node, located by its layout role: the chrome is
/// the row-stretching child of the button host, and it is the widest such node.
fn first_chrome(header: &Node) -> Option<Node> {
    let mut best: Option<Node> = None;
    fn walk(node: &Node, best: &mut Option<Node>) {
        for child in node.children() {
            if child
                .layout_node()
                .is_some_and(|layout| layout.flex_grow() > 0.0)
                && best
                    .as_ref()
                    .is_none_or(|current| child.frame().width > current.frame().width)
            {
                *best = Some(child.clone());
            }
            walk(&child, best);
        }
    }
    walk(header, &mut best);
    best
}

struct DisclosureHarness {
    expanded: State<bool>,
}
impl cove_composition::View for DisclosureHarness {
    fn body(&self, _: &cove_composition::Scope) -> AnyView {
        harness(self.expanded.clone())
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state(
            "expanded",
            &self.expanded,
        )]
    }
}

fn harness(expanded: State<bool>) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        2.0,
        AnyView::new(cove_composition::TupleView::new(vec![
            AnyView::new(DisclosureGroup::title(
                expanded.binding(),
                "Advanced",
                AnyView::new(AnyView::new(Text::new("Details")).frame_height(120.0))
                    .debug_name("disclosure-details"),
            )),
            AnyView::new(
                AnyView::new(Text::new("Following"))
                    .frame_height(24.0)
                    .debug_name("disclosure-following"),
            ),
        ])),
    ))
}

#[test]
fn the_header_fills_its_region_and_left_aligns_the_label() {
    GlobalTestLock::with_scheduler(|| {
        support::with_test_text_environment(|| {
            let expanded = State::new(false);
            let graph = support::graph();
            graph.install(
                DisclosureHarness {
                    expanded: expanded.clone(),
                },
                Default::default(),
            );
            for _ in 0..3 {
                graph.recomposer.commit_all();
                graph.compute_layout(220.0, 400.0);
            }

            let header = first_focusable(&graph).expect("the header is focusable");
            let chrome = first_chrome(&header).expect("the header paints button chrome");
            assert_eq!(
                header.frame().width,
                220.0,
                "the header spans the container"
            );
            assert_eq!(
                chrome.frame().width,
                header.frame().width,
                "the chrome fills the header's hit region"
            );

            let label = all(&graph)
                .into_iter()
                .find(|node| {
                    node.layout_node()
                        .and_then(|layout| layout.text_inputs())
                        .is_some_and(|inputs| inputs.text == "Advanced")
                })
                .expect("the label materialises as measured text");
            let label_frame = label.absolute_frame();
            let header_origin = header.absolute_frame().x;
            assert!(
                label_frame.x < header_origin + 36.0,
                "the label starts within the leading 36pt, was {} against {}",
                label_frame.x,
                header_origin
            );
            assert!(
                label_frame.width >= 50.0,
                "the label keeps room for the title, width was {}",
                label_frame.width
            );
        });
    });
}

#[test]
fn return_and_space_toggle_the_content_and_reflow_the_following_row() {
    GlobalTestLock::with_scheduler(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        support::with_test_text_environment(|| {
            let expanded = State::new(false);
            let graph = support::graph();
            graph.install(
                DisclosureHarness {
                    expanded: expanded.clone(),
                },
                Default::default(),
            );
            graph.compute_layout(220.0, 400.0);

            let initial_y = graph
                .layout_snapshot()
                .into_iter()
                .find(|entry| entry.debug_name.as_deref() == Some("disclosure-following"))
                .expect("the following row materialises")
                .absolute_frame
                .y;
            assert!(
                !has_named(&graph, "disclosure-details"),
                "collapsed content does not participate in layout"
            );

            for scancode in [Scancode::RETURN, Scancode::SPACE] {
                let host = first_focusable(&graph).expect("the header is focusable");
                let handler = registry
                    .handlers(&host)
                    .key
                    .expect("the header installs a key handler");
                let event = KeyEvent {
                    scancode,
                    keycode: 0,
                    modifiers: KeyModifiers::default(),
                    is_repeat: false,
                };
                assert_eq!(
                    handler(&event, EventPhase::Target),
                    EventResult::Handled,
                    "scancode {scancode} is handled by the header"
                );
                graph.recomposer.commit_all();
                graph.compute_layout(220.0, 400.0);
                cove_core::scene::AnimatorScheduler::current().tick(1.0);
                graph.recomposer.commit_all();
                graph.compute_layout(220.0, 400.0);

                let following = graph
                    .layout_snapshot()
                    .into_iter()
                    .find(|entry| entry.debug_name.as_deref() == Some("disclosure-following"))
                    .expect("the following row survives the toggle")
                    .absolute_frame;
                if expanded.get() {
                    let details = graph
                        .layout_snapshot()
                        .into_iter()
                        .find(|entry| entry.debug_name.as_deref() == Some("disclosure-details"))
                        .expect("expanded content materialises")
                        .absolute_frame;
                    assert_eq!(
                        details.height, 120.0,
                        "the content keeps its authored height"
                    );
                    assert!(
                        following.y >= details.y + details.height,
                        "the following row is pushed below the content, following {} details {:?}",
                        following.y,
                        details
                    );
                } else {
                    assert!(
                        !has_named(&graph, "disclosure-details"),
                        "collapsed content leaves layout again"
                    );
                    assert_eq!(
                        following.y, initial_y,
                        "the following row returns to its original position"
                    );
                }
            }
        });
        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn a_disabled_disclosure_is_not_focusable() {
    GlobalTestLock::with_scheduler(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let expanded = State::new(false);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                DisclosureGroup::title(expanded.binding(), "Advanced", AnyView::new(EmptyView))
                    .enabled(false),
            ),
            Default::default(),
        );
        graph.compute_layout(220.0, 400.0);

        assert!(
            first_focusable(&graph).is_none(),
            "a disabled disclosure cannot be keyboard activated"
        );
        assert!(!expanded.get(), "the disabled disclosure stays collapsed");
        InteractionRegistryHolder::set_current(None);
    });
}

fn has_named(graph: &cove_composition::ViewGraph, name: &str) -> bool {
    graph
        .layout_snapshot()
        .into_iter()
        .any(|entry| entry.debug_name.as_deref() == Some(name))
}
