//! Translation of Primitives/Media/Skeleton.swift.
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, LayoutNode, MeasureMode, Node, NodeResource, Rect,
};
use crate::impl_primitive_view;
use crate::primitives::media::LoadingAnimationClock;
use crate::theme::SemanticColorRef;
use crate::{PrimitiveView, View};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SkeletonShape {
    /// `None` resolves the theme's row radius so skeletons follow the app scale.
    Rectangle(Option<f32>),
    Circle,
}

#[derive(Clone)]
pub struct SkeletonAppearance {
    pub shape: SkeletonShape,
    pub height: f32,
    pub color: SemanticColorRef,
    pub is_secondary: bool,
}

impl Default for SkeletonAppearance {
    fn default() -> Self {
        Self {
            shape: SkeletonShape::Rectangle(None),
            height: 16.0,
            color: SemanticColorRef::on_surface_muted().opacity(0.24),
            is_secondary: false,
        }
    }
}

impl SkeletonAppearance {
    pub fn new() -> Self {
        Self::default()
    }

    fn validate(&mut self) {
        self.height = if self.height.is_finite() { self.height.clamp(1.0, 1024.0) } else { 16.0 };
        if let SkeletonShape::Rectangle(radius) = self.shape {
            self.shape = SkeletonShape::Rectangle(
                radius.and_then(|value| if value.is_finite() { Some(value.clamp(0.0, 512.0)) } else { None }),
            );
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonMotion {
    pub is_animated: bool,
    pub duration: f64,
    pub minimum_opacity: f32,
}

impl Default for SkeletonMotion {
    fn default() -> Self {
        Self { is_animated: true, duration: 2.0, minimum_opacity: 0.5 }
    }
}

impl SkeletonMotion {
    pub fn new() -> Self {
        Self::default()
    }

    fn validate(&mut self) {
        self.duration = if self.duration.is_finite() { self.duration.clamp(0.25, 60.0) } else { 2.0 };
        self.minimum_opacity = if self.minimum_opacity.is_finite() {
            self.minimum_opacity.clamp(0.1, 1.0)
        } else {
            0.5
        };
    }

    fn opacity(&self, phase: f64) -> f32 {
        // Smooth extrema, with identical first and last frames of each cycle.
        self.minimum_opacity
            + (1.0 - self.minimum_opacity) * ((1.0 + (phase * 2.0 * std::f64::consts::PI).cos()) / 2.0) as f32
    }
}

/// A decorative placeholder with intrinsic height, shaped painting and a
/// node-owned pulse.
pub struct Skeleton {
    pub appearance: SkeletonAppearance,
    pub motion: SkeletonMotion,
}

#[derive(Clone, PartialEq)]
struct PaintIdentity {
    shape: SkeletonShape,
    height: u32,
    is_secondary: bool,
    is_animated: bool,
    duration: u64,
    minimum_opacity: u32,
}

impl Skeleton {
    pub fn new() -> Self {
        Self { appearance: SkeletonAppearance::new(), motion: SkeletonMotion::new() }
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut Self)) -> Self {
        configure(&mut self);
        self
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

impl PrimitiveView for Skeleton {
    fn make_node(&self) -> Node {
        let node = Node::new();
        let clock: Rc<dyn NodeResource> = LoadingAnimationClock::new();
        node.add_resource(clock);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let mut appearance = self.appearance.clone();
        let mut motion = self.motion.clone();
        appearance.validate();
        motion.validate();
        node.set_is_focusable(false);
        node.set_is_hit_testable(false);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Image,
            is_hidden: true,
            ..Default::default()
        }));

        let circle = matches!(appearance.shape, SkeletonShape::Circle);
        let height = appearance.height;
        if let Some(layout) = node.layout_node() {
            layout.set_height(Some(height));
            layout.set_width(if circle { Some(height) } else { None });
            layout.set_measure_func(Some(Rc::new(move |width, width_mode, measured_height, height_mode| {
                if circle {
                    let diameter = if width_mode == MeasureMode::Exactly {
                        width
                    } else if height_mode == MeasureMode::Exactly {
                        measured_height
                    } else {
                        height
                    };
                    return cove_core::Size { width: diameter, height: diameter };
                }
                let width = if width_mode == MeasureMode::Undefined { 120.0 } else { width };
                cove_core::Size { width, height }
            })));
        }

        let animated = motion.is_animated
            && !node.prefers_reduced_motion()
            && motion.minimum_opacity < 1.0;
        if let Some(clock) = node.first_resource::<LoadingAnimationClock>() {
            clock.configure(motion.duration, animated);
        }

        let identity = PaintIdentity {
            shape: appearance.shape,
            height: appearance.height.to_bits(),
            is_secondary: appearance.is_secondary,
            is_animated: motion.is_animated,
            duration: motion.duration.to_bits(),
            minimum_opacity: motion.minimum_opacity.to_bits(),
        };
        let weak_node = node.downgrade();
        let clock = node.first_resource::<LoadingAnimationClock>();
        let identity_shape = appearance.shape;
        let identity_secondary = appearance.is_secondary;
        let identity_color = appearance.color.clone();
        node.update_draw(identity, move |list, origin| {
            let Some(owner) = Node::upgrade(&weak_node) else { return };
            let frame = owner.frame();
            let mut rect = Rect::new(origin.x, origin.y, frame.width, frame.height);
            if !rect.x.is_finite() || !rect.y.is_finite() || !rect.width.is_finite()
                || !rect.height.is_finite() || rect.width <= 0.0 || rect.height <= 0.0
            {
                return;
            }
            let phase_opacity = if animated {
                clock.as_ref().map_or(1.0, |clock| motion.opacity(clock.phase()))
            } else {
                1.0
            };
            let opacity = phase_opacity
                * if identity_secondary { 0.5 } else { 1.0 }
                * owner.opacity();
            let theme = crate::theme::theme_for_node(&owner);
            let color = identity_color.resolve(&theme).multiplied_alpha(opacity);
            let radius = match identity_shape {
                SkeletonShape::Circle => {
                    let diameter = rect.width.min(rect.height);
                    rect.x += (rect.width - diameter) / 2.0;
                    rect.y += (rect.height - diameter) / 2.0;
                    rect.width = diameter;
                    rect.height = diameter;
                    diameter / 2.0
                }
                SkeletonShape::Rectangle(corner_radius) => {
                    corner_radius.unwrap_or(theme.radius.sm).min(rect.width.min(rect.height) / 2.0)
                }
            };
            let _ = list.add_rounded_rect(rect, radius, color);
        });
    }
}
impl_primitive_view!(Skeleton);
