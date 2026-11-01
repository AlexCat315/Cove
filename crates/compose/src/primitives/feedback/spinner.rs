//! Translation of Primitives/Feedback/Spinner.swift.
use crate::primitives::media::LoadingAnimationClock;
use crate::{PrimitiveView, impl_primitive_view};
use cove_core::scene::{LayoutNode, Node, NodeResource, Point};
use std::f32::consts::{PI, TAU};
use std::rc::Rc;

/// Indeterminate activity indicator. Its node-owned clock stops when the
/// spinner leaves the scene tree and freezes under reduced motion.
pub struct Spinner {
    pub size: f32,
    pub is_animating: bool,
}

impl Spinner {
    pub fn new(size: f32) -> Self {
        Self::with_animation(size, true)
    }

    pub fn with_animation(size: f32, is_animating: bool) -> Self {
        Self {
            size: if size.is_finite() {
                size.max(8.0)
            } else {
                16.0
            },
            is_animating,
        }
    }

    pub fn animating(mut self, is_animating: bool) -> Self {
        self.is_animating = is_animating;
        self
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new(16.0)
    }
}

impl PrimitiveView for Spinner {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let clock: Rc<dyn NodeResource> = LoadingAnimationClock::new();
        node.add_resource(clock);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(self.size));
        layout.set_height(Some(self.size));
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_width(Some(self.size));
        layout.set_height(Some(self.size));
    }

    fn update_node(&self, node: &Node) {
        let Some(clock) = node.first_resource::<LoadingAnimationClock>() else {
            return;
        };
        clock.configure(0.75, self.is_animating && !node.prefers_reduced_motion());
        let weak_node = node.downgrade();
        let draw_clock = clock.clone();
        let size = self.size;
        node.update_draw((size.to_bits(), self.is_animating), move |list, origin| {
            let Some(owner) = Node::upgrade(&weak_node) else {
                return;
            };
            let frame = owner.frame();
            let diameter = frame.width.min(frame.height).min(size);
            let thickness = (diameter * 0.09375).max(1.0);
            let radius = (diameter / 2.0 - thickness).max(2.0);
            let center = Point::new(origin.x + frame.width / 2.0, origin.y + frame.height / 2.0);
            let phase = (draw_clock.phase() as f32).rem_euclid(1.0) * TAU;
            let color = owner
                .inherited_foreground_color()
                .unwrap_or_else(|| crate::theme::theme_for_node(&owner).colors.on_surface)
                .multiplied_alpha(owner.opacity());
            for index in 0..24 {
                let start = phase + index as f32 / 24.0 * PI * 1.5;
                let end = phase + (index + 1) as f32 / 24.0 * PI * 1.5;
                let from = Point::new(
                    center.x + start.cos() * radius,
                    center.y + start.sin() * radius,
                );
                let to = Point::new(center.x + end.cos() * radius, center.y + end.sin() * radius);
                let _ = list.add_line(
                    from,
                    to,
                    thickness,
                    color.multiplied_alpha((index + 1) as f32 / 24.0),
                );
            }
        });
    }
}

impl_primitive_view!(Spinner);
