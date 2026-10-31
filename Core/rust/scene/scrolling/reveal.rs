//! Translation of CoveUIScene/Scrolling/Reveal.swift.
use crate::Rect;
use crate::scene::Node;
use std::rc::Rc;

impl Node {
    /// Installed by scroll containers: scrolls so a window-space rectangle of
    /// their content is visible. Clipping boxes are not scroll containers.
    pub fn scroll_reveal(&self) -> Option<Rc<dyn Fn(Rect)>> {
        self.attachment("__scrollview.reveal")
    }
    pub fn set_scroll_reveal(&self, reveal: Option<Rc<dyn Fn(Rect)>>) {
        match reveal {
            Some(reveal) => self.set_attachment("__scrollview.reveal", reveal),
            None => self.remove_attachment("__scrollview.reveal"),
        }
    }
    /// Asks each scroll ancestor, innermost first, to bring this node into view.
    pub fn reveal_in_scroll_ancestors(&self, nearest_only: bool) {
        let frame = self.absolute_frame();
        let mut ancestor = self.parent();
        while let Some(node) = ancestor {
            if let Some(reveal) = node.scroll_reveal() {
                reveal(frame);
                if nearest_only {
                    return;
                }
            }
            ancestor = node.parent();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn reveal_walks_scroll_ancestors_innermost_first() {
        let outer = Node::new();
        let inner = Node::new();
        outer.add_child(&inner);
        let target = Node::new();
        target.set_frame(Rect::new(5.0, 5.0, 10.0, 10.0));
        inner.add_child(&target);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let outer_calls = calls.clone();
        outer.set_scroll_reveal(Some(Rc::new(move |frame| {
            outer_calls.borrow_mut().push(("outer", frame))
        })));
        let inner_calls = calls.clone();
        inner.set_scroll_reveal(Some(Rc::new(move |frame| {
            inner_calls.borrow_mut().push(("inner", frame))
        })));
        target.reveal_in_scroll_ancestors(false);
        let order: Vec<&str> = calls.borrow().iter().map(|(name, _)| *name).collect();
        assert_eq!(order, vec!["inner", "outer"]);
        calls.borrow_mut().clear();
        target.reveal_in_scroll_ancestors(true);
        assert_eq!(calls.borrow().len(), 1);
    }
}
