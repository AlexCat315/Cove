//! Translation of Android/Integration/Inspection.swift.

use super::scene::AndroidContentSession;
use cove_composition::LayoutDebugAttachmentKey;
use cove_core::scene::AccessibilityTree;
use cove_core::Rect;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Inspection {
    pub controls: HashMap<String, Rect>,
    pub semantics: Vec<cove_core::scene::AccessibilityElement>,
}

impl AndroidContentSession {
    /// Explicit diagnostic work; input delivery never traverses this snapshot.
    pub fn inspect(&self) -> Inspection {
        let mut controls = HashMap::new();
        let tree = self.tree.borrow();
        if let Some(root) = tree.root() {
            let mut stack = vec![root];
            while let Some(node) = stack.pop() {
                if let Some(name) =
                    node.attachment::<String>(LayoutDebugAttachmentKey::DEBUG_NAME)
                {
                    controls.insert(name, node.absolute_frame());
                }
                stack.extend(node.children());
            }
        }
        let semantics = tree
            .root()
            .map(|root| AccessibilityTree::snapshot(&root))
            .unwrap_or_default();
        Inspection { controls, semantics }
    }
}
