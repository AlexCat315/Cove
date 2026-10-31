//! Translation of CoveUIScene/NodeTransform.swift.
use crate::{Point, Rect};

/// How a node scales its own subtree, and about which point.
///
/// The anchor is normalized within the node's frame: `(0, 0)` is the top-left
/// corner, `(0.5, 0.5)` the centre, `(1, 1)` the bottom-right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeTransform {
    /// Multiplier applied to both axes about the anchor.
    pub scale: f32,
    /// Point the scale is taken about, normalized within the node's frame.
    pub anchor: Point,
    /// Scale shadow offsets and blur with the subtree.
    pub scales_shadow: bool,
}
impl NodeTransform {
    pub const IDENTITY: Self = Self {
        scale: 1.0,
        anchor: Point { x: 0.0, y: 0.0 },
        scales_shadow: true,
    };
    pub const fn new(scale: f32, anchor: Point) -> Self {
        Self {
            scale,
            anchor,
            scales_shadow: true,
        }
    }
    pub fn anchored(scale: f32, x: f32, y: f32) -> Self {
        Self {
            scale,
            anchor: Point { x, y },
            scales_shadow: true,
        }
    }
    pub fn with_scales_shadow(mut self, scales_shadow: bool) -> Self {
        self.scales_shadow = scales_shadow;
        self
    }
    /// Whether this transform leaves the subtree exactly where it already is.
    pub fn is_identity(self) -> bool {
        self.scale == 1.0
    }
}
impl Default for NodeTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Maps points from one space to another as a uniform scale plus a translation.
///
/// Column-major, matching `CGAffineTransform`:
///
/// ```text
/// | a  c  e |
/// | b  d  f |
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine2D {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
    /// Accumulates only transforms that scale elevation, independently of paint.
    pub shadow_scale: f32,
}
impl Affine2D {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
        shadow_scale: 1.0,
    };
    pub const fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Self {
            a,
            b,
            c,
            d,
            e,
            f,
            shadow_scale: 1.0,
        }
    }
    /// This transform applied to a point first, `next` second.
    pub fn followed(self, next: Self) -> Self {
        Self {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            e: next.a * self.e + next.c * self.f + next.e,
            f: next.b * self.e + next.d * self.f + next.f,
            shadow_scale: self.shadow_scale * next.shadow_scale,
        }
    }
    pub fn applied(self, x: f32, y: f32) -> Point {
        Point {
            x: self.a * x + self.c * y + self.e,
            y: self.b * x + self.d * y + self.f,
        }
    }
    /// The point in this transform's source space that maps to `point`.
    pub fn source(self, point: Point) -> Point {
        // A scale plus a translation inverts to a divide and a subtract, and this
        // runs per node on every pointer event.
        if self.b == 0.0 && self.c == 0.0 {
            if self.a.abs() <= 1e-9 {
                return Point { x: 0.0, y: 0.0 };
            }
            return Point {
                x: (point.x - self.e) / self.a,
                y: (point.y - self.f) / self.a,
            };
        }
        self.inverted().applied(point.x, point.y)
    }
    /// Where this transform puts the local origin.
    pub fn origin(self) -> Point {
        Point {
            x: self.e,
            y: self.f,
        }
    }
    /// The inverse, or the identity when the transform collapses a dimension.
    pub fn inverted(self) -> Self {
        let determinant = self.a * self.d - self.b * self.c;
        if determinant.abs() <= 1e-9 {
            return Self::IDENTITY;
        }
        let inverse = 1.0 / determinant;
        Self {
            a: self.d * inverse,
            b: -self.b * inverse,
            c: -self.c * inverse,
            d: self.a * inverse,
            e: (self.c * self.f - self.d * self.e) * inverse,
            f: (self.b * self.e - self.a * self.f) * inverse,
            shadow_scale: if self.shadow_scale.abs() > 1e-9 {
                1.0 / self.shadow_scale
            } else {
                1.0
            },
        }
    }
}
impl Default for Affine2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Maps a node's local space into its parent's, with `scrolling` applied to
/// everything *inside* the node first.
///
/// The order is
/// `frame.origin + (point - scrolling) * scale + anchor * size * (1 - scale)`.
/// Scroll is measured in the content's own units, so it comes out *before* the
/// scale.
pub fn placement_step(frame: Rect, transform: NodeTransform, scrolling: Point) -> Affine2D {
    let scale = transform.scale;
    Affine2D {
        a: scale,
        b: 0.0,
        c: 0.0,
        d: scale,
        shadow_scale: if transform.scales_shadow { scale } else { 1.0 },
        e: frame.x - scrolling.x * scale + transform.anchor.x * frame.width * (1.0 - scale),
        f: frame.y - scrolling.y * scale + transform.anchor.y * frame.height * (1.0 - scale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_transform_is_a_translation() {
        let step = placement_step(
            Rect::new(10.0, 20.0, 100.0, 50.0),
            NodeTransform::IDENTITY,
            Point::default(),
        );
        assert_eq!(step, Affine2D::new(1.0, 0.0, 0.0, 1.0, 10.0, 20.0));
        assert!(NodeTransform::IDENTITY.is_identity());
    }
    #[test]
    fn scale_pins_the_anchor() {
        let frame = Rect::new(0.0, 0.0, 100.0, 100.0);
        let transform = NodeTransform::anchored(0.5, 0.5, 0.5);
        let step = placement_step(frame, transform, Point::default());
        let centre = step.applied(50.0, 50.0);
        assert_eq!(centre, Point { x: 50.0, y: 50.0 });
        let corner = step.applied(0.0, 0.0);
        assert_eq!(corner, Point { x: 25.0, y: 25.0 });
    }
    #[test]
    fn scroll_is_applied_before_scale() {
        let frame = Rect::new(0.0, 0.0, 100.0, 100.0);
        let transform = NodeTransform::anchored(2.0, 0.0, 0.0);
        let step = placement_step(frame, transform, Point { x: 10.0, y: 0.0 });
        assert_eq!(step.applied(0.0, 0.0), Point { x: -20.0, y: 0.0 });
    }
    #[test]
    fn inversion_round_trips_and_degenerate_collapses() {
        let step = placement_step(
            Rect::new(5.0, 7.0, 40.0, 40.0),
            NodeTransform::anchored(1.5, 0.25, 0.75),
            Point::default(),
        );
        let point = Point { x: 13.0, y: 29.0 };
        let placed = step.applied(point.x, point.y);
        let restored = step.source(placed);
        assert!((restored.x - point.x).abs() < 1e-4 && (restored.y - point.y).abs() < 1e-4);
        let collapsed = placement_step(
            Rect::new(0.0, 0.0, 10.0, 10.0),
            NodeTransform::new(0.0, Point::default()),
            Point::default(),
        );
        assert_eq!(collapsed.inverted(), Affine2D::IDENTITY);
    }
}
