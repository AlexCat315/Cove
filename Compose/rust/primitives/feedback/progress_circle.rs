//! Translation of Primitives/Feedback/ProgressCircle.swift.
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align, Justify, LayoutNode,
    Node, NodeResource, Point,
};
use crate::impl_primitive_view;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::style::AppearanceExt;
use crate::theme::tokens::semantic_modifiers::SemanticStyleViewExt;
use crate::primitives::feedback::{ProgressValue, StatusTone};
use crate::primitives::media::LoadingAnimationClock;
use crate::primitives::text::Text;
use crate::{AnyView, PrimitiveView, View};
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::rc::Rc;

/// Determinate or indeterminate circular progress.
pub struct ProgressCircle {
    /// Fraction completed in `0...1`. `None` renders a rotating arc.
    pub value: Option<f64>,
    pub tone: StatusTone,
    /// Explicit diameter. `None` follows the shared control size.
    pub diameter: Option<f32>,
    /// Ring thickness. `None` scales with the diameter so the ring keeps the
    /// same visual weight at every size.
    pub thickness: Option<f32>,
    pub shows_value: bool,
    pub is_enabled: bool,
}

#[derive(Clone, PartialEq)]
struct PaintIdentity {
    value: Option<f64>,
    tone: StatusTone,
    diameter: f32,
    thickness: f32,
    is_enabled: bool,
    indeterminate: bool,
}

impl ProgressCircle {
    pub fn new(value: Option<f64>) -> Self {
        Self {
            value,
            tone: StatusTone::Info,
            diameter: None,
            thickness: None,
            shows_value: false,
            is_enabled: true,
        }
    }

    pub fn diameter(mut self, diameter: f32) -> Self {
        self.diameter = Some(diameter);
        self
    }

    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = Some(thickness);
        self
    }

    pub fn shows_value(mut self, shows_value: bool) -> Self {
        self.shows_value = shows_value;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn clamped(&self) -> f64 {
        match self.value {
            Some(value) if value.is_finite() => value.clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    fn is_indeterminate(&self) -> bool {
        self.value.is_none_or(|value| !value.is_finite())
    }

    fn resolved_diameter(&self, node: &Node) -> f32 {
        if let Some(diameter) = self.diameter
            && diameter.is_finite()
        {
            return diameter.clamp(12.0, 512.0);
        }
        match node.composition_value(&crate::theme::ControlSizeEnvironment::key()) {
            crate::theme::ControlSize::Mini => 16.0,
            crate::theme::ControlSize::Small => 22.0,
            crate::theme::ControlSize::Regular => 32.0,
            crate::theme::ControlSize::Large => 44.0,
        }
    }

    /// Ring thickness for a resolved diameter.
    pub fn resolved_thickness(&self, diameter: f32) -> f32 {
        if let Some(thickness) = self.thickness
            && thickness.is_finite()
        {
            return thickness.clamp(1.0, diameter / 2.0);
        }
        (diameter * 0.11).max(2.0)
    }
}

impl PrimitiveView for ProgressCircle {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let clock: Rc<dyn NodeResource> = LoadingAnimationClock::new();
        node.add_resource(clock);
        let value: Rc<dyn NodeResource> = ProgressValue::new();
        node.add_resource(value);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        node.set_is_focusable(false);
        let resolved = self.clamped();
        let indeterminate = self.is_indeterminate();
        let percent = (resolved * 100.0).round() as i64;
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Progress,
            label: "Progress".to_string(),
            value: if indeterminate { "In progress".to_string() } else { format!("{percent}%") },
            state: AccessibilityState { is_enabled: self.is_enabled, ..Default::default() },
            ..Default::default()
        }));

        let diameter = self.resolved_diameter(node);
        let thickness = self.resolved_thickness(diameter);
        if let Some(layout) = node.layout_node() {
            layout.set_width(Some(diameter));
            layout.set_height(Some(diameter));
            layout.set_align_items(Align::Center);
            layout.set_justify_content(Justify::Center);
            // No measure function: `shows_value` puts a label inside this node,
            // and Yoga rejects children on a measured node. The fixed extent is
            // enough — a progress ring is never sized by its content.
            layout.set_flex_shrink(0.0);
        }

        if let Some(clock) = node.first_resource::<LoadingAnimationClock>() {
            clock.configure(1.1, indeterminate && !node.prefers_reduced_motion());
        }
        if let Some(presentation) = node.first_resource::<ProgressValue>() {
            presentation.update(node, resolved, !indeterminate);
        }

        let identity = PaintIdentity {
            value: Some(resolved), tone: self.tone, diameter, thickness,
            is_enabled: self.is_enabled, indeterminate,
        };
        let weak_node = node.downgrade();
        let clock = node.first_resource::<LoadingAnimationClock>();
        let presentation = node.first_resource::<ProgressValue>();
        let tone = self.tone;
        let is_enabled = self.is_enabled;
        let fallback_thickness = self.thickness;
        node.update_draw(identity, move |list, origin| {
            let Some(owner) = Node::upgrade(&weak_node) else { return };
            let frame = owner.frame();
            let size = frame.width.min(frame.height);
            if size <= 2.0 {
                return;
            }
            let theme = crate::theme::theme_for_node(&owner);
            let base = tone.color().resolve(&theme);
            let alpha = owner.opacity() * if is_enabled { 1.0 } else { 0.5 };
            let cx = origin.x + frame.width / 2.0;
            let cy = origin.y + frame.height / 2.0;
            let ring_thickness = match fallback_thickness {
                Some(value) if value.is_finite() => value.clamp(1.0, size / 2.0),
                _ => (size * 0.11).max(2.0),
            }
            .max(1.5);
            let outer = size / 2.0 - ring_thickness / 2.0;
            let inner = (outer - ring_thickness).max(0.5);
            // Same track tone as Slider: sunken surfaces are white in light themes.
            let track = theme.colors.surface_variant.multiplied_alpha(alpha);
            paint_ring(list, Point::new(cx, cy), inner, outer, 0.0, TAU, track);
            if indeterminate {
                let Some(clock) = clock.as_ref() else { return };
                // A fixed-length arc that rotates; the length is ~35% of the
                // circle so the gap is obvious enough to read as motion.
                let start = clock.phase() as f32 * TAU;
                paint_ring(list, Point::new(cx, cy), inner, outer, start, PI * 0.7, base.multiplied_alpha(alpha));
            } else {
                let Some(presentation) = presentation.as_ref() else { return };
                let value = presentation.value() as f32;
                if value <= 0.0 {
                    return;
                }
                paint_ring(list, Point::new(cx, cy), inner, outer, -FRAC_PI_2, TAU * value, base.multiplied_alpha(alpha));
            }
        });
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        if !self.shows_value {
            return Vec::new();
        }
        let resolved = self.clamped();
        let indeterminate = self.is_indeterminate();
        let percent = (resolved * 100.0).round() as i64;
        let scale = self.resolved_diameter(node);
        let size = if scale >= 56.0 { 13.0 } else if scale >= 40.0 { 11.0 } else { 9.0 };
        let color = if self.is_enabled {
            self.tone.color()
        } else {
            crate::theme::SemanticColorRef::on_surface_disabled()
        };
        vec![AnyView::new(
            Text::new(if indeterminate { String::new() } else { percent.to_string() })
                .font(cove_core::Font::system(size, cove_core::FontWeight::Medium))
                .semantic_foreground(color)
                .accessibility_hidden(true),
        )]
    }
}
impl_primitive_view!(ProgressCircle);

/// Paints an annulus sector as a triangle strip.
fn paint_ring(list: &mut cove_core::DrawList, center: Point, inner: f32, outer: f32, from: f32, sweep: f32, color: cove_core::Color) {
    let _ = list.add_arc_stroke(center, (inner + outer) / 2.0, from, sweep, outer - inner, color);
}
