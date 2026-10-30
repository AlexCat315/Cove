//! Generic viewport clamping for transient overlays.
use cove_core::{Point, Rect, Size};

pub struct OverlayPlacement;

impl OverlayPlacement {
    pub fn fit(position: Point, size: Size, bounds: Rect, anchor: Option<Rect>, margin: f32) -> Point {
        if bounds.width <= 0.0 || bounds.height <= 0.0 { return position; }
        let min_x = bounds.x + margin;
        let min_y = bounds.y + margin;
        let max_x = (bounds.max_x() - margin - size.width).max(min_x);
        let max_y = (bounds.max_y() - margin - size.height).max(min_y);
        let mut y = position.y;
        if y + size.height > bounds.max_y() - margin
            && let Some(anchor) = anchor
            && anchor.y - size.height >= min_y
        {
            y = anchor.y - size.height;
        }
        Point::new(position.x.clamp(min_x, max_x), y.clamp(min_y, max_y))
    }
}
