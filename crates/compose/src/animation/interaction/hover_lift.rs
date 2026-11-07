//! A surface that rises toward the pointer.
//!
//! Hovering grows the surface slightly about its centre and deepens its
//! shadow on a spring, so a card answers the pointer like a physical object
//! and settles back when it leaves.
use std::rc::Rc;

use cove_core::scene::{ActiveAnimationContext, HoverPhase, InteractionRegistryHolder, Node};
use cove_core::{Color, Point, SurfaceShadow};

use crate::theme::{SemanticMotionRef, theme_for_node};
use crate::{ModifiedContent, View, ViewExt, ViewModifier};

const RESTING_SHADOW: &str = "__hover_lift.resting";
const PROGRESS: &str = "__hover_lift.progress";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoverLift {
    /// Scale the surface reaches under the pointer.
    pub scale: f32,
}

impl Default for HoverLift {
    fn default() -> Self {
        Self { scale: 1.015 }
    }
}

impl ViewModifier for HoverLift {
    fn apply_node(&self, node: &Node) {
        let progress = node.attachment::<f32>(PROGRESS).unwrap_or(0.0);
        // Other modifiers repaint the resting shadow on every update; remember
        // it so the lift always starts from the authored elevation.
        if progress == 0.0 {
            node.set_attachment(RESTING_SHADOW, node.shadow());
        }
        apply(node, self.scale, progress);
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let weak = node.downgrade();
        let scale = self.scale;
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                let target = if phase == HoverPhase::Enter { 1.0 } else { 0.0 };
                let current = owner.attachment::<f32>(PROGRESS).unwrap_or(0.0);
                let motion = SemanticMotionRef::snappy().resolve(&theme_for_node(&owner));
                let follow = owner.downgrade();
                ActiveAnimationContext::with(Some(motion), None, None, || {
                    owner.animatable_set_with(
                        "hoverLift.progress",
                        current,
                        target,
                        move |value| {
                            if let Some(inner) = Node::upgrade(&follow) {
                                inner.set_attachment(PROGRESS, value);
                                apply(&inner, scale, value);
                            }
                        },
                    );
                });
            }),
        );
    }
}

fn apply(node: &Node, scale: f32, progress: f32) {
    let mut transform = node.transform();
    transform.anchor = Point::new(0.5, 0.5);
    transform.scales_shadow = false;
    transform.scale = 1.0 + (scale - 1.0) * progress;
    node.set_transform(transform);
    let resting = node
        .attachment::<SurfaceShadow>(RESTING_SHADOW)
        .unwrap_or(SurfaceShadow::NONE);
    let raised = theme_for_node(node).elevation.high;
    node.set_shadow(mix_shadow(resting, raised, progress));
}

fn mix_shadow(from: SurfaceShadow, to: SurfaceShadow, amount: f32) -> SurfaceShadow {
    let lerp = |a: f32, b: f32| a + (b - a) * amount;
    let from_color = from.color.unwrap_or(Color::CLEAR);
    let to_color = to.color.unwrap_or(Color::CLEAR);
    SurfaceShadow {
        color: Some(Color {
            r: lerp(from_color.r, to_color.r),
            g: lerp(from_color.g, to_color.g),
            b: lerp(from_color.b, to_color.b),
            a: lerp(from_color.a, to_color.a).max(0.0),
        }),
        offset_x: lerp(from.offset_x, to.offset_x),
        offset_y: lerp(from.offset_y, to.offset_y),
        blur: lerp(from.blur, to.blur).max(0.0),
    }
}

pub trait HoverLiftExt: View + Sized + 'static {
    /// Lift this surface toward the pointer on hover.
    fn hover_lift(self) -> ModifiedContent {
        self.modifier(HoverLift::default())
    }
    /// Lift with a custom scale; wide surfaces need a smaller one.
    fn hover_lift_scaled(self, scale: f32) -> ModifiedContent {
        self.modifier(HoverLift { scale })
    }
}
impl<T: View + Sized + 'static> HoverLiftExt for T {}
