//! Translation of CoveUIScene/Input/PointerCapture.swift and the per-window
//! ambient holders declared in PlatformInputContext.swift.
use crate::scene::Node;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Tracks an explicit pointer-capture target.
///
/// When a node calls `acquire` during a `mouseButtonDown` handler, all
/// subsequent `mouseMotion` and `mouseButtonUp` events for that pointer go
/// directly to the captured node (skipping hit-test) until `release`.
pub struct PointerCapture {
    target: RefCell<Option<Node>>,
    allows_gesture_transfer: Cell<bool>,
}
impl PointerCapture {
    pub fn new() -> Self {
        Self { target: RefCell::new(None), allows_gesture_transfer: Cell::new(false) }
    }
    pub fn target(&self) -> Option<Node> {
        self.target.borrow().clone()
    }
    pub fn allows_gesture_transfer(&self) -> bool {
        self.allows_gesture_transfer.get()
    }
    pub fn is_active(&self) -> bool {
        self.target.borrow().is_some()
    }
    pub fn acquire(&self, node: &Node, allows_gesture_transfer: bool) {
        if self.target.borrow().as_ref() == Some(node) {
            self.allows_gesture_transfer.set(allows_gesture_transfer);
            return;
        }
        let previous = self.target.borrow_mut().replace(node.clone());
        self.allows_gesture_transfer.set(allows_gesture_transfer);
        if let Some(previous) = previous {
            previous.attachment::<Rc<dyn Fn()>>(PointerCaptureAttachmentKey::DID_LOSE_CAPTURE).map(|callback| callback());
        }
    }
    pub fn release(&self) {
        let previous = self.target.borrow_mut().take();
        self.allows_gesture_transfer.set(false);
        if let Some(previous) = previous {
            previous.attachment::<Rc<dyn Fn()>>(PointerCaptureAttachmentKey::DID_LOSE_CAPTURE).map(|callback| callback());
        }
    }
}
impl Default for PointerCapture {
    fn default() -> Self {
        Self::new()
    }
}

/// Controls can cancel a pending press when an ancestor gesture takes capture.
pub struct PointerCaptureAttachmentKey;
impl PointerCaptureAttachmentKey {
    pub const DID_LOSE_CAPTURE: &'static str = "__pointerCapture.didLoseCapture";
}

thread_local! {
    static CURRENT_CAPTURE: RefCell<Option<Rc<PointerCapture>>> = const { RefCell::new(None) };
}
pub struct PointerCaptureHolder;
impl PointerCaptureHolder {
    pub fn current() -> Option<Rc<PointerCapture>> {
        CURRENT_CAPTURE.with(|slot| slot.borrow().clone())
    }
    pub fn set_current(capture: Option<Rc<PointerCapture>>) {
        CURRENT_CAPTURE.with(|slot| *slot.borrow_mut() = capture);
    }
    pub fn current_target() -> Option<Node> {
        Self::current().and_then(|capture| capture.target())
    }
    pub fn release_current() {
        if let Some(capture) = Self::current() {
            capture.release();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn capture_transfers_and_notifies_the_previous_owner() {
        let capture = PointerCapture::new();
        let first = Node::new();
        let second = Node::new();
        let lost = Rc::new(Cell::new(0));
        let counter = lost.clone();
        first.set_attachment(PointerCaptureAttachmentKey::DID_LOSE_CAPTURE, Rc::new(move || counter.set(counter.get() + 1)) as Rc<dyn Fn()>);
        capture.acquire(&first, false);
        assert!(capture.is_active());
        capture.acquire(&second, true);
        assert_eq!(lost.get(), 1);
        assert!(capture.allows_gesture_transfer());
        capture.release();
        assert!(!capture.is_active());
        assert!(!capture.allows_gesture_transfer());
    }
}
