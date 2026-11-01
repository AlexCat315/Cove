//! Translation of `Tests/CoveUIComposeTests/NumberFieldTests.swift`.
//!
//! Behaviour checklist:
//! - pressing Return commits the parsed float to the bound value
//! - losing focus commits the parsed float to the bound value
//! - an unparseable edit is discarded on blur, leaving the previous number

mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use cove_composition::ViewGraph;
use cove_core::Binding;
use cove_core::scene::{
    EventPhase, FocusChain, FocusChainHolder, InteractionRegistry, InteractionRegistryHolder, Node,
    NodeTree, Recomposer,
};
use cove_platform::{KeyEvent, KeyModifiers};
use cove_ui_compose::primitives::controls::fields::numeric::number_field::NumberField;
use cove_ui_compose::primitives::text::text_field::TextFieldSize;

use support::GlobalTestLock;

/// Swift's `Scancode.lgui` chord for "select all": macOS scancode 4.
const SCANCODE_LGUI_CHORD: u32 = 4;
/// macOS Return.
const SCANCODE_RETURN: u32 = 40;

struct Rig {
    graph: Rc<ViewGraph>,
    registry: Rc<InteractionRegistry>,
    focus: Rc<FocusChain>,
    value: Arc<AtomicU32>,
}

fn rig(initial: f32) -> Rig {
    let registry = Rc::new(InteractionRegistry::new());
    let focus = Rc::new(FocusChain::new());
    InteractionRegistryHolder::set_current(Some(registry.clone()));
    FocusChainHolder::set_current(Some(focus.clone()));

    let value = Arc::new(AtomicU32::new(initial.to_bits()));
    let graph = Rc::new(ViewGraph::new(
        Rc::new(RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ));

    let get = value.clone();
    let set = value.clone();
    let binding = Binding::new(
        move || f32::from_bits(get.load(Ordering::SeqCst)),
        move |next| set.store(next.to_bits(), Ordering::SeqCst),
    );

    graph.install(
        NumberField::new(binding).size(TextFieldSize::Small),
        Default::default(),
    );
    Rig {
        graph,
        registry,
        focus,
        value,
    }
}

impl Rig {
    /// The field surface: the node carrying the text-input key handler.
    fn field_node(&self) -> Node {
        let root = self.graph.tree.borrow().root().expect("root");
        let mut nodes = Vec::new();
        all_nodes(&root, &mut nodes);
        nodes
            .into_iter()
            .find(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == cove_core::scene::AccessibilityRole::TextField
                })
            })
            .expect("the number field materialises a text field")
    }

    fn key(&self, node: &Node, scancode: u32, modifiers: KeyModifiers) {
        let handler = self
            .registry
            .handlers(node)
            .key
            .expect("the field registers a key handler");
        let _ = handler(
            &KeyEvent {
                scancode,
                keycode: 0,
                modifiers,
                is_repeat: false,
            },
            EventPhase::Target,
        );
    }

    fn text(&self, node: &Node, text: &str) {
        let handler = self
            .registry
            .handlers(node)
            .text
            .expect("the field registers a text handler");
        let _ = handler(text, EventPhase::Target);
    }

    fn value(&self) -> f32 {
        f32::from_bits(self.value.load(Ordering::SeqCst))
    }
}

fn all_nodes(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        all_nodes(&child, into);
    }
}

#[test]
fn return_commits_the_parsed_float() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let rig = rig(1.0);
            let node = rig.field_node();
            rig.focus.focus(Some(&node), false);
            rig.graph.recomposer.commit_all();

            rig.key(&node, SCANCODE_LGUI_CHORD, KeyModifiers::LEFT_GUI);
            rig.text(&node, "12.5");
            rig.key(&node, SCANCODE_RETURN, KeyModifiers::default());

            assert!(
                (rig.value() - 12.5).abs() < 0.0001,
                "Return commits the parsed value (got {})",
                rig.value()
            );
        });
    });
}

#[test]
fn blur_commits_the_parsed_float() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let rig = rig(3.0);
            let node = rig.field_node();
            rig.focus.focus(Some(&node), false);
            rig.graph.recomposer.commit_all();

            rig.key(&node, SCANCODE_LGUI_CHORD, KeyModifiers::LEFT_GUI);
            rig.text(&node, "7.25");
            rig.focus.clear();
            rig.graph.recomposer.commit_all();

            assert!(
                (rig.value() - 7.25).abs() < 0.0001,
                "blur commits the parsed value (got {})",
                rig.value()
            );
        });
    });
}

#[test]
fn invalid_blur_keeps_the_previous_value() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let rig = rig(4.5);
            let node = rig.field_node();
            rig.focus.focus(Some(&node), false);
            rig.graph.recomposer.commit_all();

            rig.key(&node, SCANCODE_LGUI_CHORD, KeyModifiers::LEFT_GUI);
            rig.text(&node, "abc");
            rig.focus.clear();
            rig.graph.recomposer.commit_all();

            assert!(
                (rig.value() - 4.5).abs() < 0.0001,
                "an unparseable edit leaves the previous number (got {})",
                rig.value()
            );
        });
    });
}
