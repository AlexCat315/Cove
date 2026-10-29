//! Translation of CoveUIScene/Rendering/ContentMask.swift.
use crate::{Rect, RoundedRect};
use crate::scene::Node;

/// The rounded mask a clipping container applies to its content and children:
/// its box inset by the border, with the matching inner radius. Square
/// containers need none, since their scissor is already exact.
pub fn content_mask(
    node: &Node,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    border_width: f32,
) -> Option<RoundedRect> {
    if !node.clips_to_bounds() || radius <= 0.0 || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let inset = if node.border_color().is_some() && border_width > 0.0 { border_width } else { 0.0 };
    let rect = Rect::new(
        x + inset,
        y + inset,
        (width - 2.0 * inset).max(0.0),
        (height - 2.0 * inset).max(0.0),
    );
    let mask = RoundedRect::new(rect, radius - inset);
    (mask.radius > 0.0).then_some(mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_containers_need_no_mask() {
        let node = Node::new();
        node.set_clips_to_bounds(true);
        assert!(content_mask(&node, 0.0, 0.0, 10.0, 10.0, 0.0, 0.0).is_none());
        node.set_corner_radius(4.0);
        let mask = content_mask(&node, 0.0, 0.0, 10.0, 10.0, 4.0, 0.0).unwrap();
        assert_eq!(mask.radius, 4.0);
    }

    #[test]
    fn border_width_insets_the_mask() {
        let node = Node::new();
        node.set_clips_to_bounds(true);
        node.set_border_color(Some(crate::Color::BLACK));
        node.set_border_width(2.0);
        let mask = content_mask(&node, 0.0, 0.0, 20.0, 20.0, 6.0, 2.0).unwrap();
        assert_eq!(mask.rect, Rect::new(2.0, 2.0, 16.0, 16.0));
        assert_eq!(mask.radius, 4.0);
    }
}
