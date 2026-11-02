//! Translation of `Tests/CoveUIComposeTests/ShortcutHostTests.swift`.
//!
//! Behaviour checklist:
//! - `ShortcutHost` registers a system shortcut route on an unfocusable,
//!   unhittable node
//! - a `true` result maps to `EventResult::Handled`, `false` to `Ignored`

use std::rc::Rc;

use cove_composition::ViewGraph;
use cove_core::scene::{
    EventResult, InputHandlerRoute, InteractionRegistry, InteractionRegistryHolder, Node, NodeTree,
    Recomposer,
};
use cove_platform::{KeyEvent, KeyModifiers};
use cove_ui_compose::primitives::overlays::ShortcutHost;

fn find_shortcut_node(node: &Node, registry: &InteractionRegistry) -> Option<Node> {
    if registry.handlers(node).key_route == Some(InputHandlerRoute::shortcut()) {
        return Some(node.clone());
    }
    for child in node.children() {
        if let Some(found) = find_shortcut_node(&child, registry) {
            return Some(found);
        }
    }
    None
}

fn install_shortcut_host(host: ShortcutHost) -> (Rc<ViewGraph>, Rc<InteractionRegistry>, Node) {
    let registry = Rc::new(InteractionRegistry::new());
    InteractionRegistryHolder::set_current(Some(registry.clone()));
    let graph = Rc::new(ViewGraph::new(
        Rc::new(std::cell::RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ));
    graph.install(host, Default::default());
    let root = graph.tree.borrow().root().expect("installed root");
    let node = find_shortcut_node(&root, &registry).expect("shortcut node");
    InteractionRegistryHolder::set_current(None);
    (graph, registry, node)
}

#[test]
fn registers_shortcut_route() {
    let (_graph, registry, node) = install_shortcut_host(ShortcutHost::new(|_| true));
    let handlers = registry.handlers(&node);

    assert!(
        !node.is_hit_testable(),
        "a shortcut host must not accept pointer input"
    );
    assert!(
        !node.is_focusable(),
        "a shortcut host must not join focus traversal"
    );
    assert_eq!(handlers.key_route, Some(InputHandlerRoute::shortcut()));
    assert!(
        handlers.key.is_some(),
        "the shortcut route must carry a key handler"
    );
}

#[test]
fn maps_bool_result() {
    let (_graph, registry, node) =
        install_shortcut_host(ShortcutHost::new(|key| key.keycode == 0x31));
    let handler = registry.handlers(&node).key.expect("key handler");

    let matching = KeyEvent {
        scancode: 0,
        keycode: 0x31,
        modifiers: KeyModifiers::LEFT_GUI,
        is_repeat: false,
    };
    assert_eq!(
        handler(&matching, cove_core::scene::EventPhase::Capture),
        EventResult::Handled
    );

    let other = KeyEvent {
        scancode: 0,
        keycode: 0x32,
        modifiers: KeyModifiers::LEFT_GUI,
        is_repeat: false,
    };
    assert_eq!(
        handler(&other, cove_core::scene::EventPhase::Capture),
        EventResult::Ignored
    );
}
