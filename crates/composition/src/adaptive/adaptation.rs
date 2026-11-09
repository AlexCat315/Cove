//! Marks presentations that suit only one kind of device, so audits can find
//! a desktop presentation on a phone or a handheld one on a pointer desktop.
use cove_core::scene::Node;

/// Node attachment naming the device family a presentation is meant for.
pub const ADAPTATION_KEY: &str = "adaptation.only";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdaptationMark {
    /// Pointer-first: hover, right click, dense rows, docked panels.
    DesktopOnly,
    /// Touch-first: bottom bars, wheels, swipe and pull gestures.
    HandheldOnly,
}

impl AdaptationMark {
    pub fn apply(self, node: &Node) {
        node.set_attachment(ADAPTATION_KEY, self);
    }

    pub fn clear(node: &Node) {
        node.remove_attachment(ADAPTATION_KEY);
    }

    pub fn of(node: &Node) -> Option<Self> {
        node.attachment::<Self>(ADAPTATION_KEY)
    }
}
