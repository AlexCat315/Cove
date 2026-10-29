//! Translation of CoveUIScene/Input/TabStop.swift.
use super::Node;
use std::sync::atomic::{AtomicU64, Ordering};

/// Bumped when any property that decides sequential focus / subtree input
/// eligibility changes.
pub struct FocusRevision;
impl FocusRevision {
    static CURRENT: AtomicU64 = AtomicU64::new(0);
    pub fn current() -> u64 {
        Self::CURRENT.load(Ordering::Relaxed)
    }
    pub fn bump() {
        Self::CURRENT.fetch_add(1, Ordering::Relaxed);
    }
}

impl Node {
    /// Programmatic and pointer focus remain available outside sequential Tab
    /// traversal.
    pub fn is_tab_stop(&self) -> bool {
        self.attachment::<bool>("__focus.tabStop").unwrap_or(true)
    }
    pub fn set_is_tab_stop(&self, value: bool) {
        if self.is_tab_stop() == value {
            return;
        }
        self.set_attachment("__focus.tabStop", value);
        FocusRevision::bump();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_stop_defaults_to_true_and_bumps_revision() {
        let node = Node::new();
        assert!(node.is_tab_stop());
        let before = FocusRevision::current();
        node.set_is_tab_stop(false);
        assert!(!node.is_tab_stop());
        assert!(FocusRevision::current() > before);
    }
}
