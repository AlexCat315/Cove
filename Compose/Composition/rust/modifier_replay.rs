//! Translation of CoveUIComposeCore/ModifierReplay.swift.
//!
//! Modifiers applied by the caller of a transparent component must run after
//! that component rebuilds its own body. Otherwise internal primitive updates
//! overwrite the caller's frame, flex, style and event configuration.
use cove_core::scene::Node;
use std::cell::RefCell;
use std::rc::Rc;
use crate::ViewGraph;

type Entries = Rc<RefCell<Vec<Rc<dyn Fn()>>>>;

pub struct ModifierReplay;
impl ModifierReplay {
    const KEY: &'static str = "CoveUI.component.modifiers";

    /// A parent update reaches the content boundary before unwinding its
    /// modifiers. Replace that declaration's list; local body updates keep it.
    pub fn reset(node: &Node) {
        node.remove_attachment(Self::KEY);
    }

    pub fn install(node: &Node, apply: Rc<dyn Fn()>) {
        let entries: Entries = match node.attachment(Self::KEY) {
            Some(existing) => existing,
            None => {
                let entries: Entries = Rc::new(RefCell::new(Vec::new()));
                node.set_attachment(Self::KEY, entries.clone());
                entries
            }
        };
        // AnyView can erase several declarations to the same generic type.
        // Preserve every declaration in order instead of deduplicating by type.
        entries.borrow_mut().push(apply);
    }

    /// Replay the nearest caller first, then its enclosing caller. Anchors are
    /// the transparent nodes between the content boundary and the layout root.
    pub fn apply_from(anchor: &Node, graph: &Rc<ViewGraph>) {
        let mut current = Some(anchor.clone());
        while let Some(node) = current {
            if graph.layout_node(&node).is_some() {
                break;
            }
            if let Some(entries) = node.attachment::<Entries>(Self::KEY) {
                let callbacks: Vec<Rc<dyn Fn()>> = entries.borrow().clone();
                for callback in callbacks {
                    callback();
                }
            }
            current = node.parent();
        }
    }
}
