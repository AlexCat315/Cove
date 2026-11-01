//! Translation of `Tests/CoveUIComposeTests/StatePropagationRegressionTests.swift`.
//!
//! Behaviour checklist:
//! - a button action still writes parent state after that parent recomposed
//! - a button's label subtree reflows when the parent's state changes
//! - a text field's `on_change` still writes parent state after the parent
//!   recomposed
//!
//! The Swift suite stashes each `_DebugNode` label in `frame.origin.x` because
//! it deliberately skips layout. `support::DebugNode` records its label in the
//! node's `view_tag`, but `ViewGraph::materialise_primitive` overwrites that
//! with the Rust type path, so the label is unreadable afterwards. This suite
//! therefore uses a local `Marker` primitive that stores its label as a node
//! attachment, which survives materialisation; `max_marker` reads the label
//! length instead of a frame coordinate and still needs no layout pass.

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, Column, DynamicField, PrimitiveView, TupleView, View, ViewGraph};
use cove_core::State;
use cove_core::scene::{
    AccessibilityRole, EventPhase, FocusChain, FocusChainHolder, InteractionRegistry,
    InteractionRegistryHolder, Node, NodeTree, PointerPhase, Recomposer,
};
use cove_platform::{MouseButton, MouseButtonEvent};
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::{TextField, TextFieldEvents};

use support::GlobalTestLock;

/// Attachment key carrying a marker label, mirroring the Swift `_DebugNode`.
const MARKER_KEY: &str = "__state_propagation_marker";

/// A leaf that records its label so the test can read it back after
/// materialisation.
#[derive(Clone)]
struct Marker {
    label: String,
}

impl Marker {
    fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl PrimitiveView for Marker {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment(MARKER_KEY, self.label.clone());
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
}
cove_ui_compose::impl_primitive_view!(Marker);

/// The longest `DebugNode` label in the tree, matching the Swift `maxMarker`.
fn max_marker(node: Option<&Node>) -> usize {
    let Some(node) = node else {
        return 0;
    };
    let own = node
        .attachment::<String>(MARKER_KEY)
        .map(|label| label.len())
        .unwrap_or(0);
    node.children()
        .iter()
        .map(|child| max_marker(Some(child)))
        .max()
        .unwrap_or(0)
        .max(own)
}

fn first_node(root: &Node, predicate: impl Fn(&Node) -> bool + Copy) -> Option<Node> {
    if predicate(root) {
        return Some(root.clone());
    }
    for child in root.children() {
        if let Some(found) = first_node(&child, predicate) {
            return Some(found);
        }
    }
    None
}

fn new_graph() -> Rc<ViewGraph> {
    Rc::new(ViewGraph::new(
        Rc::new(std::cell::RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ))
}

fn markers(graph: &Rc<ViewGraph>) -> usize {
    let root = graph.tree.borrow().root();
    max_marker(root.as_ref())
}

/// The Swift `ButtonHarness`: `@State` flag and count feeding two debug leaves.
struct ButtonHarness {
    flag: State<bool>,
    count: State<i32>,
}

impl View for ButtonHarness {
    fn body(&self) -> AnyView {
        let counter = self.count.clone();
        let shown = self.count.get();
        AnyView::new(Column::new(
            cove_composition::HorizontalAlignment::Leading,
            0.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Marker::new(if self.flag.get() { "yy" } else { "x" })),
                AnyView::new(Button::title("Tap", move || counter.set(7))),
                AnyView::new(Marker::new("x".repeat(shown.max(0) as usize))),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("flag", &self.flag),
            DynamicField::state("count", &self.count),
        ]
    }
}

#[test]
fn a_button_action_still_updates_parent_state_after_the_parent_recomposed() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let registry = Rc::new(InteractionRegistry::new());
            InteractionRegistryHolder::set_current(Some(registry.clone()));

            let flag = State::new(false);
            let count = State::new(0);
            let graph = new_graph();
            graph.install_view(
                AnyView::new(ButtonHarness {
                    flag: flag.clone(),
                    count: count.clone(),
                }),
                Default::default(),
            );

            assert_eq!(markers(&graph), 1, "the initial marker is one character");

            flag.set(true);
            graph.recomposer.commit_all();
            assert_eq!(
                markers(&graph),
                2,
                "recomposing the parent grows the first marker"
            );

            // The Swift suite skips layout and so has to hand the host a frame
            // by hand. Giving the root a real layout pass is the production
            // path: the button then owns a non-empty frame and the (0,0) press
            // lands inside it.
            graph.compute_layout(300.0, 200.0);
            let root = graph.tree.borrow().root().expect("root");
            let host = first_node(&root, |node| registry.handlers(node).pointer.is_some())
                .expect("the button registers a pointer handler");
            let frame = host.frame();
            assert!(
                frame.width > 0.0 && frame.height > 0.0,
                "the layout pass gives the host a real frame: {frame:?}"
            );
            let pointer = registry
                .handlers(&host)
                .pointer
                .expect("the pointer handler exists");

            let event = MouseButtonEvent {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
                clicks: 1,
                modifiers: Default::default(),
                kind: Default::default(),
            };
            pointer(&event, PointerPhase::Down, EventPhase::Target);
            pointer(&event, PointerPhase::Up, EventPhase::Target);
            graph.recomposer.commit_all();

            assert_eq!(
                markers(&graph),
                7,
                "the button action still reaches the parent state after the recompose"
            );
            assert_eq!(count.get(), 7, "the parent state carries the new value");
            InteractionRegistryHolder::set_current(None);
        });
    });
}

/// The Swift `ButtonTextLabelHarness`: the button's own label reads state.
struct ButtonTextLabelHarness {
    flag: State<bool>,
}

impl View for ButtonTextLabelHarness {
    fn body(&self) -> AnyView {
        AnyView::new(Button::new(
            AnyView::new(Marker::new(if self.flag.get() { "xxxxxxx" } else { "x" })),
            || {},
        ))
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("flag", &self.flag)]
    }
}

#[test]
fn a_button_label_reflows_after_the_parent_recomposed() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry));

        let flag = State::new(false);
        let graph = new_graph();
        graph.install_view(
            AnyView::new(ButtonTextLabelHarness { flag: flag.clone() }),
            Default::default(),
        );
        assert_eq!(markers(&graph), 1, "the label starts one character wide");

        flag.set(true);
        graph.recomposer.commit_all();
        assert_eq!(
            markers(&graph),
            7,
            "the button's label subtree reflows with the parent state"
        );
        InteractionRegistryHolder::set_current(None);
    });
}

/// The Swift `TextFieldHarness`: an `on_change` callback writes parent state.
struct TextFieldHarness {
    flag: State<bool>,
    text: State<TextBuffer>,
}

impl View for TextFieldHarness {
    fn body(&self) -> AnyView {
        let writer = self.text.clone();
        let observed = self.text.get();
        let mut events = TextFieldEvents::default();
        events.on_change = Some(Rc::new(move |next: &TextBuffer| {
            writer.set(next.clone());
        }));
        let field = TextField::new("", self.text.binding()).events(events);
        AnyView::new(Column::new(
            cove_composition::HorizontalAlignment::Leading,
            0.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Marker::new(if self.flag.get() { "yy" } else { "x" })),
                AnyView::new(field),
                AnyView::new(Marker::new(
                    "x".repeat(observed.string_value().chars().count()),
                )),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("flag", &self.flag),
            DynamicField::state("text", &self.text),
        ]
    }
}

#[test]
fn a_text_field_binding_still_updates_parent_state_after_the_parent_recomposed() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let previous_focus = FocusChainHolder::current();
            let registry = Rc::new(InteractionRegistry::new());
            let focus = Rc::new(FocusChain::new());
            InteractionRegistryHolder::set_current(Some(registry.clone()));
            FocusChainHolder::set_current(Some(focus.clone()));

            let flag = State::new(false);
            let text = State::new(TextBuffer::empty());
            let graph = new_graph();
            graph.install_view(
                AnyView::new(TextFieldHarness {
                    flag: flag.clone(),
                    text: text.clone(),
                }),
                Default::default(),
            );

            assert_eq!(markers(&graph), 1, "the initial marker is one character");

            flag.set(true);
            graph.recomposer.commit_all();
            assert_eq!(
                markers(&graph),
                2,
                "recomposing the parent grows the first marker"
            );

            let root = graph.tree.borrow().root().expect("root");
            let field = first_node(&root, |node| registry.handlers(node).text.is_some())
                .expect("the field registers a text handler");
            let handler = registry
                .handlers(&field)
                .text
                .expect("the text handler exists");

            focus.focus(Some(&field), true);
            handler("abc", EventPhase::Target);
            graph.recomposer.commit_all();

            assert_eq!(
                markers(&graph),
                3,
                "typing still reaches the parent state after the recompose"
            );
            assert_eq!(
                text.get().string_value(),
                "abc",
                "the parent state carries the typed text"
            );

            FocusChainHolder::set_current(previous_focus);
            InteractionRegistryHolder::set_current(None);
        });
    });
}

const _: fn() = || {
    let _ = AccessibilityRole::Button;
    let _: fn(&Text) = |_| {};
    let _ = |_: &dyn PrimitiveView| {};
};
