//! Translation of CoveUIComposeCore/LayoutTree.swift.
//!
//! Wraps the layout tree root. The per-`LayoutNode` text measure cache lives on
//! `LayoutNode` itself, so this type only owns the root and the wholesale cache
//! reset used by tests and DevTools when the font environment changes.
use cove_core::scene::LayoutNode;

pub struct LayoutTree {
    pub root: LayoutNode,
}
impl LayoutTree {
    pub fn new() -> Self {
        Self { root: LayoutNode::new() }
    }
    pub fn reset_caches(&self) {
        clear_text_measure_cache(&self.root);
    }
}
impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}

fn clear_text_measure_cache(node: &LayoutNode) {
    node.set_text_measure(None);
    for child in node.children() {
        clear_text_measure_cache(&child);
    }
}
