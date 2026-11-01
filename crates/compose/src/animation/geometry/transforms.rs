//! Scale and offset view transforms translated from Animation/Geometry/Transforms.swift.
use crate::{AnyView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{LayoutNode, Node, NodeTransform, Point};

struct TransformHost {
    factor: f32,
    anchor: Point,
    offset: Point,
    content: AnyView,
}

impl PrimitiveView for TransformHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(cove_core::scene::FlexDirection::Column);
        layout.set_align_items(cove_core::scene::Align::Stretch);
    }

    fn update_node(&self, node: &Node) {
        let current = node.transform();
        node.set_transform(NodeTransform {
            anchor: self.anchor,
            scales_shadow: true,
            ..current
        });
        let mounted = node
            .attachment::<bool>("motion.transform.mounted")
            .unwrap_or(false);
        if !mounted {
            node.set_transform(NodeTransform {
                scale: self.factor,
                anchor: self.anchor,
                scales_shadow: true,
            });
            node.set_content_offset(self.offset);
            node.set_attachment("motion.transform.mounted", true);
            return;
        }

        let owner = node.downgrade();
        node.animatable_set_with("motion.scale", current.scale, self.factor, move |scale| {
            if let Some(owner) = Node::upgrade(&owner) {
                let mut transform = owner.transform();
                transform.scale = scale.max(0.0);
                owner.set_transform(transform);
            }
        });
        let owner = node.downgrade();
        node.animatable_set_with(
            "motion.offset",
            node.content_offset(),
            self.offset,
            move |offset| {
                if let Some(owner) = Node::upgrade(&owner) {
                    owner.set_content_offset(offset);
                }
            },
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(TransformHost);

/// Apply a non-negative scale around a normalized anchor point.
pub fn scale_effect(content: impl View + 'static, factor: f32, anchor: Point) -> impl View {
    TransformHost {
        factor: if factor.is_finite() {
            factor.max(0.0)
        } else {
            1.0
        },
        anchor: Point::new(finite_or(anchor.x, 0.5), finite_or(anchor.y, 0.5)),
        offset: Point::default(),
        content: AnyView::new(content),
    }
}

/// Translate the content by the supplied x/y offsets.
pub fn offset(content: impl View + 'static, x: f32, y: f32) -> impl View {
    TransformHost {
        factor: 1.0,
        anchor: Point::default(),
        offset: Point::new(-finite_or(x, 0.0), -finite_or(y, 0.0)),
        content: AnyView::new(content),
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

pub fn scale_effect_anchored(
    content: impl View + 'static,
    factor: f32,
    anchor: Point,
) -> impl View {
    scale_effect(content, factor, anchor)
}

pub trait TransformViewExt: View + Sized + 'static {
    fn scale_effect(self, factor: f32) -> impl View {
        scale_effect(self, factor, Point::new(0.5, 0.5))
    }
    fn scale_effect_anchored(self, factor: f32, anchor: Point) -> impl View {
        scale_effect(self, factor, anchor)
    }
    fn offset(self, x: f32, y: f32) -> impl View {
        offset(self, x, y)
    }
}
impl<T: View + Sized + 'static> TransformViewExt for T {}
