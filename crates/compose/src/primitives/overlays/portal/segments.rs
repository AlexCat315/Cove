//! Keeps overlays inside one window segment, so no menu, popover, dialog or
//! sheet straddles a fold or sits under a hinge.
use cove_core::scene::{Direction, Node};
use cove_core::{Environment, Point, Rect, display_features};

use crate::adaptive::SurfaceEnvironment;

/// `window` divided by the surface's separating folds and hinges. Segments are
/// derived from the measured window, not the reported viewport, so a surface
/// without folds always yields exactly `window`.
pub fn overlay_segments(node: &Node, window: Rect) -> Vec<Rect> {
    let environment: Environment = node.composition_value(&SurfaceEnvironment::key());
    if environment.display_features.is_empty() {
        return vec![window];
    }
    let segments = display_features::window_segments(window, &environment.display_features);
    if segments.is_empty() {
        vec![window]
    } else {
        segments
    }
}

/// The segment that hosts an overlay anchored to `anchor` (or opened at
/// `origin` when it has no anchor).
pub fn anchored_bounds(node: &Node, window: Rect, anchor: Option<Rect>, origin: Point) -> Rect {
    let segments = overlay_segments(node, window);
    if segments.len() < 2 {
        return window;
    }
    let target = anchor.unwrap_or(Rect::new(origin.x, origin.y, 1.0, 1.0));
    display_features::segment_for(&segments, target).unwrap_or(window)
}

/// The segment that hosts a window-level presentation.
pub fn presentation_bounds(node: &Node, window: Rect) -> Rect {
    let segments = overlay_segments(node, window);
    if segments.len() < 2 {
        return window;
    }
    display_features::presentation_segment(&segments, committed_direction(node) == Direction::Rtl)
        .unwrap_or(window)
}

/// The nearest explicit layout direction committed on `node`'s ancestry.
fn committed_direction(node: &Node) -> Direction {
    let mut current = Some(node.clone());
    while let Some(candidate) = current {
        if let Some(layout) = candidate.layout_node() {
            let value = layout.direction();
            if value != Direction::Inherit {
                return value;
            }
        }
        current = candidate.parent();
    }
    Direction::Ltr
}
