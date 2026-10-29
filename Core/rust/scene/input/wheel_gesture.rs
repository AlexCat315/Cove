//! Translation of CoveUIScene/Input/WheelGesture.swift.
use crate::scene::{Node, NodeTree};
use cove_platform::{ScrollPhase, ScrollInput};

/// A phased stream keeps its consuming viewport even when the pointer moves.
/// A click catches OS momentum; only fresh input can start scrolling again.
#[derive(Default)]
pub struct WheelGesture {
    target: Option<Node>,
    has_target: bool,
    interrupted: bool,
}
impl WheelGesture {
    pub fn interrupt(&mut self) {
        self.interrupted = true;
    }
    pub fn prepare(&mut self, input: ScrollInput) -> bool {
        if input.phase == ScrollPhase::MayBegin {
            self.target = None;
            self.has_target = false;
            self.interrupted = true;
            return true;
        }
        if input.phase == ScrollPhase::Began || !input.is_continuation() {
            self.target = None;
            self.has_target = false;
            self.interrupted = false;
        }
        if input.momentum_phase == ScrollPhase::None && (input.phase == ScrollPhase::Changed || input.phase == ScrollPhase::Stationary) {
            self.interrupted = false;
        }
        !self.interrupted
    }
    pub fn path(&self, tree: &NodeTree) -> Option<Vec<Node>> {
        if !self.has_target {
            return None;
        }
        let mut path = Vec::new();
        let mut current = self.target.clone();
        while let Some(node) = current {
            if !node.accepts_subtree_input() {
                return Some(Vec::new());
            }
            path.push(node.clone());
            current = node.parent();
        }
        if path.last() != tree.root().as_ref() {
            return Some(Vec::new());
        }
        path.reverse();
        Some(path)
    }
    pub fn consumed(&mut self, node: &Node, input: ScrollInput) {
        if input.phase == ScrollPhase::None && input.momentum_phase == ScrollPhase::None {
            return;
        }
        self.target = Some(node.clone());
        self.has_target = true;
    }
    pub fn finish(&mut self, input: ScrollInput) {
        if input.phase == ScrollPhase::Cancelled || input.momentum_phase == ScrollPhase::Ended || input.momentum_phase == ScrollPhase::Cancelled {
            self.target = None;
            self.has_target = false;
        }
        if input.phase == ScrollPhase::Cancelled || input.momentum_phase == ScrollPhase::Cancelled {
            self.interrupted = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_stream_pins_the_consuming_viewport() {
        let mut gesture = WheelGesture::default();
        let mut input = ScrollInput { phase: ScrollPhase::Began, ..ScrollInput::default() };
        assert!(gesture.prepare(input));
        let viewport = Node::new();
        gesture.consumed(&viewport, input);
        input.phase = ScrollPhase::Changed;
        assert!(gesture.prepare(input));
        assert!(gesture.path(&NodeTree::new()).is_some());

        gesture.interrupt();
        assert!(!gesture.prepare(input));
        input.phase = ScrollPhase::Began;
        assert!(gesture.prepare(input));
        gesture.finish(ScrollInput { phase: ScrollPhase::Cancelled, ..ScrollInput::default() });
        assert!(gesture.path(&NodeTree::new()).is_none());
    }
}
