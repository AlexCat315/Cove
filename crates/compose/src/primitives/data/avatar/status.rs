//! Translation of `Primitives/Data/Avatar/Status.swift`.
//!
//! Presence shown as a corner indicator. Each status pairs a hue with a
//! distinct shape (filled, inset ring, bar, hollow ring) so presence stays
//! readable without color.
use crate::theme::theme_environment::theme_for_node;
use crate::theme::tokens::SemanticColorRef;
use crate::{PrimitiveView, impl_primitive_view};
use cove_core::scene::{Align, Color, Edge, LayoutNode, Node, Point, PositionType, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AvatarStatus {
    Online,
    Away,
    Busy,
    Offline,
}
impl AvatarStatus {
    pub const ALL: [Self; 4] = [Self::Online, Self::Away, Self::Busy, Self::Offline];

    pub fn label(self) -> &'static str {
        match self {
            Self::Online => "Online",
            Self::Away => "Away",
            Self::Busy => "Busy",
            Self::Offline => "Offline",
        }
    }

    pub fn color(self) -> SemanticColorRef {
        match self {
            Self::Online => SemanticColorRef::success(),
            Self::Away => SemanticColorRef::warning(),
            Self::Busy => SemanticColorRef::error(),
            Self::Offline => SemanticColorRef::on_surface_muted(),
        }
    }
}

/// Status geometry relative to the avatar's own diameter and shape. Circles
/// put the indicator center on the outline at 45 degrees; square corners tuck
/// it slightly inside so it never floats free of the shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvatarStatusGeometry {
    pub size: f32,
    pub inset: f32,
}
impl AvatarStatusGeometry {
    pub fn new(diameter: f32, shape: super::avatar::AvatarShape) -> Self {
        let size = (diameter * 0.28).round().clamp(6.0, 24.0);
        let radius = shape.radius(diameter);
        // Distance from the bounding corner to the outline along the diagonal.
        let corner_gap = radius * (1.0 - std::f32::consts::FRAC_1_SQRT_2);
        Self {
            size,
            inset: corner_gap - size / 2.0,
        }
    }
}

/// Logical bottom-trailing indicator. Uses the `End` edge, so RTL mirrors it to
/// the bottom-left with no caller involvement.
pub struct AvatarStatusIndicator {
    pub status: AvatarStatus,
    pub geometry: AvatarStatusGeometry,
}
impl AvatarStatusIndicator {
    pub fn new(status: AvatarStatus, geometry: AvatarStatusGeometry) -> Self {
        Self { status, geometry }
    }
}

impl PrimitiveView for AvatarStatusIndicator {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        self.update_layout(&layout);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(self.geometry.inset, Edge::End);
        layout.set_position(self.geometry.inset, Edge::Bottom);
        layout.set_width(Some(self.geometry.size));
        layout.set_height(Some(self.geometry.size));
        layout.set_align_items(Align::Center);
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment("avatar.status", self.status);
        node.set_z_index(1.0);
        let theme = theme_for_node(node);
        let ring = theme.colors.surface;
        let ink = self.status.color().resolve(&theme);
        let size = self.geometry.size;
        let status = self.status;
        let identity = (ring.rgba8(), ink.rgba8(), size.to_bits(), status as u8);
        node.update_draw(identity, move |list, origin: Point| {
            let outer = Rect::new(origin.x, origin.y, size, size);
            let _ = list.add_rounded_rect(outer, size / 2.0, ring);
            let ring_width = (size * 0.16).max(1.5);
            let inner = Rect::new(
                origin.x + ring_width,
                origin.y + ring_width,
                (size - ring_width * 2.0).max(0.0),
                (size - ring_width * 2.0).max(0.0),
            );
            match status {
                AvatarStatus::Online => {
                    let _ = list.add_rounded_rect(inner, inner.width / 2.0, ink);
                }
                AvatarStatus::Away => {
                    let _ = list.add_rounded_rect(inner, inner.width / 2.0, ink);
                    let dot = inner.width * 0.42;
                    let _ = list.add_rounded_rect(
                        Rect::new(
                            inner.x + inner.width * 0.12,
                            inner.y + inner.height * 0.12,
                            dot,
                            dot,
                        ),
                        dot / 2.0,
                        ring,
                    );
                }
                AvatarStatus::Busy => {
                    let _ = list.add_rounded_rect(inner, inner.width / 2.0, ink);
                    let bar = (inner.height * 0.22).max(1.0);
                    let _ = list.add_rounded_rect(
                        Rect::new(
                            inner.x + inner.width * 0.22,
                            inner.y + inner.height / 2.0 - bar / 2.0,
                            inner.width * 0.56,
                            bar,
                        ),
                        bar / 2.0,
                        ring,
                    );
                }
                AvatarStatus::Offline => {
                    let _ = list.add_rounded_rect_stroke(
                        inner,
                        inner.width / 2.0,
                        (inner.width * 0.24).max(1.25),
                        ink,
                    );
                }
            }
        });
    }
}
impl_primitive_view!(AvatarStatusIndicator);

/// Paint identity used by the Swift cache. Kept public for callers that need
/// to compare indicator drawings without rebuilding the hook.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvatarStatusPaint {
    pub ring: Color,
    pub ink: Color,
    pub size_bits: u32,
    pub status: AvatarStatus,
}
