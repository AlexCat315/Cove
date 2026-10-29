//! Translation of CoveUIScene/NodeGeometry.swift.
use super::{placement_step, Affine2D, Node};
use crate::{Point, Rect};

impl Node {
    /// Window-space origin of this node: its local frame composed down the
    /// chain, with every ancestor's scroll offset applied to its content and
    /// every ancestor's `transform` applied about its own anchor.
    pub fn absolute_origin(&self) -> Point {
        self.absolute_transform().origin()
    }
    /// Window-space frame of this node, sized by the accumulated scale.
    pub fn absolute_frame(&self) -> Rect {
        let transform = self.absolute_transform();
        let frame = self.frame();
        Rect::new(transform.e, transform.f, frame.width * transform.a.abs(), frame.height * transform.d.abs())
    }
    /// Convert a window-space point into this node's local coordinates.
    pub fn convert_from_window(&self, point: Point) -> Point {
        self.absolute_transform().inverted().applied(point.x, point.y)
    }
    /// Maps this node's local coordinates into window space.
    pub fn absolute_transform(&self) -> Affine2D {
        let mut transform = placement_step(self.frame(), self.transform(), Point::default());
        let mut ancestor = self.parent();
        while let Some(current) = ancestor {
            transform = transform.followed(placement_step(current.frame(), current.transform(), current.content_offset()));
            ancestor = current.parent();
        }
        transform
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::NodeTransform;

    #[test]
    fn nested_frames_compose_with_scroll_and_scale() {
        let root = Node::new();
        root.set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
        let scroller = Node::new();
        scroller.set_frame(Rect::new(10.0, 20.0, 200.0, 100.0));
        scroller.set_content_offset(Point { x: 0.0, y: 40.0 });
        root.add_child(&scroller);
        let child = Node::new();
        child.set_frame(Rect::new(5.0, 5.0, 50.0, 20.0));
        scroller.add_child(&child);
        assert_eq!(child.absolute_origin(), Point { x: 15.0, y: 25.0 });

        child.set_transform(NodeTransform::anchored(2.0, 0.0, 0.0));
        assert_eq!(child.absolute_origin(), Point { x: 15.0, y: 25.0 });
        assert_eq!(child.absolute_frame().width, 100.0);
        let local = child.convert_from_window(Point { x: 15.0, y: 25.0 });
        assert!(local.x.abs() < 1e-4 && local.y.abs() < 1e-4);
    }
}
