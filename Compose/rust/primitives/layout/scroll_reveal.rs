//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/ScrollReveal.swift.
use cove_core::scene::Node;

/// Keyboard groups reveal through real scroll containers, whose motion
/// controller owns bounds and bindings. Clipping boxes are not scroll views.
pub struct ScrollReveal;
impl ScrollReveal {
    pub fn reveal(target: &Node, nearest_only: bool) {
        target.reveal_in_scroll_ancestors(nearest_only);
    }
    pub fn delta(start: f32, end: f32, viewport_start: f32, viewport_end: f32) -> f32 {
        if viewport_end <= viewport_start {
            return 0.0;
        }
        if start < viewport_start || end - start > viewport_end - viewport_start {
            return start - viewport_start;
        }
        if end > viewport_end {
            return end - viewport_end;
        }
        0.0
    }
}
