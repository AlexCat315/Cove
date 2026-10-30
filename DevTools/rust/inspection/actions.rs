//! Accessibility actions on live nodes.
//!
//! Performs one action exactly as an assistive client would, or reveals the node
//! in its scroll containers. Unlike pointer input this needs no visible geometry,
//! so an agent can open an offscreen sidebar page or step a virtualized row.
use cove_core::scene::NodeTree;
use cove_devtools_protocol::{MirrorActivate, MirrorAction};
use std::cell::RefCell;
use std::rc::Rc;

fn find(id: &str, node: &cove_core::scene::Node) -> Option<cove_core::scene::Node> {
    if node.id().0.to_string() == id {
        return Some(node.clone());
    }
    for child in node.children() {
        if let Some(found) = find(id, &child) {
            return Some(found);
        }
    }
    None
}

/// Returns `None` on success, or why the action could not run.
pub fn perform_accessibility_action(tree: &Rc<RefCell<NodeTree>>, request: &MirrorActivate) -> Option<String> {
    let root = tree.borrow().root();
    let node = root.as_ref().and_then(|root| find(&request.node_id, root));
    let Some(node) = node else {
        return Some(format!("Node is missing or unmounted: {}", request.node_id));
    };
    if request.action == MirrorAction::Reveal {
        // Disabled nodes can still be looked at.
        node.reveal_in_scroll_ancestors(false);
        return None;
    }
    if !node.accepts_subtree_input()
        || node.accessibility().is_some_and(|semantics| !semantics.state.is_enabled)
    {
        return Some(format!("Node {} is disabled", request.node_id));
    }
    let actions = node.accessibility_actions();
    let action = match request.action {
        MirrorAction::Activate => actions.activate,
        MirrorAction::Increment => actions.increment,
        MirrorAction::Decrement => actions.decrement,
        MirrorAction::Reveal => None,
    };
    match action {
        Some(action) => {
            action();
            None
        }
        None => Some(format!(
            "Node {} has no {} action",
            request.node_id,
            match request.action {
                MirrorAction::Activate => "activate",
                MirrorAction::Increment => "increment",
                MirrorAction::Decrement => "decrement",
                MirrorAction::Reveal => "reveal",
            }
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cove_core::scene::Node;
    use std::cell::Cell;

    fn tree_with_button(activations: Rc<Cell<u32>>) -> (Rc<RefCell<NodeTree>>, String) {
        let mut tree = NodeTree::new();
        let root = Node::new();
        let button = Node::new();
        button.set_is_hit_testable(true);
        let counter = activations.clone();
        let mut actions = button.accessibility_actions();
        actions.activate = Some(Rc::new(move || counter.set(counter.get() + 1)));
        button.set_accessibility_actions(actions);
        button.set_accessibility(Some(cove_core::scene::AccessibilitySemantics::new(
            cove_core::scene::AccessibilityRole::Button,
        )));
        root.add_child(&button);
        let id = button.id().0.to_string();
        tree.set_root(Some(root));
        (Rc::new(RefCell::new(tree)), id)
    }

    #[test]
    fn activate_runs_the_registered_action() {
        let activations = Rc::new(Cell::new(0));
        let (tree, id) = tree_with_button(activations.clone());
        let request = MirrorActivate { node_id: id, action: MirrorAction::Activate };
        assert_eq!(perform_accessibility_action(&tree, &request), None);
        assert_eq!(activations.get(), 1);
    }

    #[test]
    fn missing_and_disabled_nodes_report_a_reason() {
        let activations = Rc::new(Cell::new(0));
        let (tree, id) = tree_with_button(activations.clone());
        let missing = MirrorActivate { node_id: "nope".into(), action: MirrorAction::Activate };
        assert!(perform_accessibility_action(&tree, &missing).is_some());

        let root = tree.borrow().root().unwrap();
        let button = find(&id, &root).unwrap();
        let mut semantics = button.accessibility().unwrap();
        semantics.state.is_enabled = false;
        button.set_accessibility(Some(semantics));
        let request = MirrorActivate { node_id: id, action: MirrorAction::Activate };
        let reason = perform_accessibility_action(&tree, &request).expect("disabled");
        assert!(reason.contains("disabled"));
        assert_eq!(activations.get(), 0);
    }
}
