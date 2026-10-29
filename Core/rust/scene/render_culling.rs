//! Translation of CoveUIScene/RenderCulling.swift.
use crate::{DrawList, Rect};

pub(crate) struct RenderCulling;
impl RenderCulling {
    pub fn is_outside(rect: Rect, list: &DrawList) -> bool {
        if let Some(clip) = list.current_clip()
            && !intersects(rect, clip)
        {
            return true;
        }
        if let Some(viewport) = list.viewport_bounds
            && !intersects(rect, viewport)
        {
            return true;
        }
        false
    }
    pub fn bounds(list: &DrawList) -> Option<Rect> {
        list.paint_bounds()
    }
}

fn intersects(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_and_viewport_both_cull() {
        let mut list = DrawList::default();
        assert!(!RenderCulling::is_outside(Rect::new(0.0, 0.0, 10.0, 10.0), &list));
        list.push_clip_rect(Rect::new(0.0, 0.0, 100.0, 100.0));
        assert!(RenderCulling::is_outside(Rect::new(200.0, 0.0, 10.0, 10.0), &list));
        assert!(!RenderCulling::is_outside(Rect::new(10.0, 10.0, 10.0, 10.0), &list));
        list.viewport_bounds = Some(Rect::new(0.0, 0.0, 50.0, 50.0));
        assert!(RenderCulling::is_outside(Rect::new(60.0, 60.0, 10.0, 10.0), &list));
    }
}
