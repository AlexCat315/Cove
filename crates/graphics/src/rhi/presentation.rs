//! Translation of NativeRHI/Resources/PresentationMode.swift.
//!
//! The presentation behavior actually selected for one native window. A request
//! to disable VSync can fall back to a synchronized mode when the window system
//! does not support immediate presentation.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PresentationMode {
    /// Presents without waiting for the next display refresh; tearing is
    /// possible.
    Immediate,
    /// Displays the latest pending image at refresh, replacing older pending
    /// images.
    Mailbox,
    /// Presents queued images in order at display refresh.
    Fifo,
    /// The window compositor controls cadence without an explicit refresh
    /// interval.
    Compositor,
}
impl PresentationMode {
    /// Whether presentation remains controlled by display refresh or its
    /// compositor.
    pub fn synchronizes_to_display(self) -> bool {
        self != Self::Immediate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_immediate_presents_unsynchronized() {
        assert!(!PresentationMode::Immediate.synchronizes_to_display());
        assert!(PresentationMode::Fifo.synchronizes_to_display());
        assert!(PresentationMode::Mailbox.synchronizes_to_display());
        assert!(PresentationMode::Compositor.synchronizes_to_display());
    }
}
