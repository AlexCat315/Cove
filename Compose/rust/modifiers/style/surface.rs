//! Translation of Modifiers/Style/SurfaceFinishModifier.swift.
use crate::theme::{ThemeMetric, ThemeReader, theme_for_node};
use crate::{AnyView, ModifiedContent, View, ViewExt, ViewModifier};
use cove_core::Rect;
use cove_core::scene::{Node, NodeWeak, SurfaceShadow};

pub struct SurfaceFinishModifier;

impl ViewModifier for SurfaceFinishModifier {
    fn apply_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        let border = theme.colors.border;
        node.set_shadow(theme.elevation.low);
        let weak: NodeWeak = node.downgrade();
        node.update_overlay_draw(border, move |list, origin| {
            let Some(node) = Node::upgrade(&weak) else {
                return;
            };
            let inset = node.corner_radius();
            let frame = node.frame();
            let rect = Rect::new(
                origin.x + inset,
                origin.y,
                (frame.width - inset * 2.0).max(0.0),
                1.0,
            );
            let _ = list.add_rect(rect, border.multiplied_alpha(0.4 * node.opacity()));
        });
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FloatingSurfaceElevation {
    Panel,
    #[default]
    Modal,
}

pub struct FloatingSurfaceModifier {
    pub corner_radius: f32,
    pub elevation: FloatingSurfaceElevation,
}

impl FloatingSurfaceModifier {
    pub fn new(corner_radius: f32, elevation: FloatingSurfaceElevation) -> Self {
        Self {
            corner_radius: corner_radius.max(0.0),
            elevation,
        }
    }
}

impl ViewModifier for FloatingSurfaceModifier {
    fn apply_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        let colors = theme.colors;
        node.animatable_set_optional(
            "backgroundColor",
            node.background_color(),
            Some(colors.surface_floating),
            {
                let node = node.clone();
                move |value| node.set_background_color(value)
            },
        );
        node.animatable_set_with("cornerRadius", node.corner_radius(), self.corner_radius, {
            let node = node.clone();
            move |value| node.set_corner_radius(value)
        });
        node.animatable_set_optional("borderColor", node.border_color(), Some(colors.border), {
            let node = node.clone();
            move |value| node.set_border_color(value)
        });
        node.animatable_set_with("borderWidth", node.border_width(), 1.0, {
            let node = node.clone();
            move |value| node.set_border_width(value)
        });
        match self.elevation {
            FloatingSurfaceElevation::Modal => {
                let elevation = theme.elevation.floating;
                node.set_ambient_shadow(elevation.ambient);
                node.set_shadow(elevation.key);
            }
            FloatingSurfaceElevation::Panel => {
                node.set_ambient_shadow(SurfaceShadow::default());
                node.set_shadow(theme.elevation.high);
            }
        }
    }
}

pub trait SurfaceStyleExt: View + Sized + 'static {
    fn surface_finish(self) -> ModifiedContent {
        self.modifier(SurfaceFinishModifier)
    }

    fn floating_surface(self, corner_radius: f32) -> ModifiedContent {
        self.modifier(FloatingSurfaceModifier::new(
            corner_radius,
            FloatingSurfaceElevation::Modal,
        ))
    }

    fn floating_surface_with_elevation(
        self,
        corner_radius: f32,
        elevation: FloatingSurfaceElevation,
    ) -> ModifiedContent {
        self.modifier(FloatingSurfaceModifier::new(corner_radius, elevation))
    }

    fn floating_surface_metric(self, metric: ThemeMetric) -> AnyView {
        self.floating_surface_metric_with_elevation(metric, FloatingSurfaceElevation::Modal)
    }

    fn floating_surface_metric_with_elevation(
        self,
        metric: ThemeMetric,
        elevation: FloatingSurfaceElevation,
    ) -> AnyView {
        let content = AnyView::new(self);
        AnyView::new(ThemeReader::new(move |theme| {
            content
                .clone()
                .floating_surface_with_elevation(metric.resolve(theme), elevation)
        }))
    }
}
impl<T: View + Sized + 'static> SurfaceStyleExt for T {}
