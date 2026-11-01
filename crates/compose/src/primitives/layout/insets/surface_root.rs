//! Translation of Primitives/Layout/Insets/SurfaceRoot.swift.
//!
//! Installs host surface facts once at the root of a scene: safe-area insets,
//! text scale and reduced motion.
//!
//! The root deliberately does *not* pad content. Modern platforms draw apps edge
//! to edge: the app paints its background behind the system bars and only its
//! interactive chrome is inset. `SafeArea` consumes the insets this root
//! publishes, so screens that own a status bar, navigation bar or keyboard get
//! the correct avoidance without a letterboxed surface.
use crate::adaptive::SurfaceEnvironment;
use crate::modifiers::layout::{EdgeInsets, FlexExt, LayoutFrameExt};
use crate::theme::{TextScaler, TextScalerEnvironment, node_text_scaler};
use crate::{AnyView, BoxView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::ReducedMotionEnvironment;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

use super::safe_area::safe_area_insets;

/// Host surface facts installed at the root of a scene.
pub struct SurfaceRoot {
    pub environment: cove_core::Environment,
    content: AnyView,
}

impl SurfaceRoot {
    pub fn new(environment: cove_core::Environment, content: impl View + 'static) -> Self {
        Self {
            environment,
            content: AnyView::new(content),
        }
    }

    /// The safe area is persistent chrome; the keyboard overlays it while it is
    /// visible, so the two never add up.
    pub fn insets(&self) -> EdgeInsets {
        let safe = self.environment.safe_area.normalized();
        let keyboard = if self.environment.keyboard.height.is_finite() {
            self.environment.keyboard.height.max(0.0)
        } else {
            0.0
        };
        EdgeInsets::new(
            safe.top,
            safe.leading,
            safe.bottom.max(keyboard),
            safe.trailing,
        )
    }
}

impl View for SurfaceRoot {
    fn body(&self) -> AnyView {
        let environment = self.environment.clone();
        let content = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(self.content.clone().flex_with(1.0, 1.0, Some(0.0))),
            )
            .flex_with(1.0, 1.0, Some(0.0))
            .frame(None, None, Some(0.0), Some(0.0), None, None),
        );
        // Each surface replaces the inherited system ramp, retaining authored
        // application scaling. Nested surfaces must not multiply host ramps.
        let insets = self.insets();
        let system_scale = environment.text_scale.clone();
        let reduced = environment.reduces_motion;
        let themed = AnyView::new(SurfaceRootProviders {
            content,
            environment,
            reduced,
            system_scale,
        });
        safe_area_insets(themed, insets)
    }
}

/// Publishes the surface facts to every descendant in one pass.
struct SurfaceRootProviders {
    content: AnyView,
    environment: cove_core::Environment,
    reduced: bool,
    system_scale: cove_core::TextScale,
}

impl PrimitiveView for SurfaceRootProviders {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &cove_core::scene::Node) {
        node.set_composition_value(&SurfaceEnvironment::key(), self.environment.clone());
        node.set_composition_value(&ReducedMotionEnvironment::key(), self.reduced);
    }

    fn children(&self, node: &cove_core::scene::Node) -> Vec<AnyView> {
        // The system ramp replaces the inherited one rather than compounding it,
        // so a nested surface cannot multiply host scaling.
        let inherited = node_text_scaler(node);
        let next: TextScaler = inherited.applying_system_scale(self.system_scale.clone());
        node.set_composition_value(&TextScalerEnvironment::key(), next);
        vec![self.content.clone()]
    }
}
impl_primitive_view!(SurfaceRootProviders);
