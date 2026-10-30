//! Translation of Modifiers/Layout/FrameCaptureModifier.swift.
use cove_core::scene::{LayoutUpdateHook, Node, NodeResource};
use cove_core::Rect;
use std::any::Any;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use crate::{ModifiedContent, View, ViewExt, ViewModifier};

/// Thread-safe snapshot of a node's most recently laid out absolute frame.
#[derive(Clone, Default)]
pub struct FrameBox(Arc<Mutex<Option<Rect>>>);

impl FrameBox {
    pub fn new() -> Self { Self::default() }

    pub fn current(&self) -> Option<Rect> {
        *self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn update(&self, frame: Rect) {
        *self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(frame);
    }
}

struct FrameCaptureResource {
    frame: FrameBox,
    previous_layout: std::cell::RefCell<Option<LayoutUpdateHook>>,
}

impl NodeResource for FrameCaptureResource {
    fn mount(&self, node: &Node) {
        let previous = node.layout_did_update();
        *self.previous_layout.borrow_mut() = previous.clone();
        let frame = self.frame.clone();
        node.set_layout_did_update(Some(Rc::new(move |updated| {
            if let Some(previous) = &previous { previous(updated); }
            frame.update(updated.absolute_frame());
        })));
        self.frame.update(node.absolute_frame());
    }

    fn unmount(&self, node: &Node) {
        node.set_layout_did_update(self.previous_layout.borrow_mut().take());
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> { self }
}

pub struct FrameCaptureModifier(pub FrameBox);

impl ViewModifier for FrameCaptureModifier {
    fn apply_node(&self, node: &Node) {
        if node.first_resource::<FrameCaptureResource>().is_none() {
            node.add_resource(Rc::new(FrameCaptureResource {
                frame: self.0.clone(),
                previous_layout: std::cell::RefCell::new(None),
            }));
        }
    }
}

pub trait CaptureFrameExt: View + Sized + 'static {
    fn capture_frame(self, frame: FrameBox) -> ModifiedContent {
        self.modifier(FrameCaptureModifier(frame))
    }
}
impl<T: View + Sized + 'static> CaptureFrameExt for T {}
