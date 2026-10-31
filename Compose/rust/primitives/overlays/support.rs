//! Composition glue shared by the overlay primitives.
//!
//! The Swift overlays are written as modifier chains (`padding`, `frame`,
//! `flex`, `offset`, `zIndex`, `floatingSurface`, …). Those modifiers belong to
//! the Compose modifier layer; the overlay ports need the subset below before
//! that layer lands, so it is implemented here on top of the same
//! `ViewModifier` routing the real modifiers use. Each one writes exactly the
//! node/layout property its Swift counterpart writes.
use crate::ViewModifier;
use crate::theme::ThemeEnvironment;
use cove_core::Insets;
use cove_core::scene::{Direction, Edge, LayoutNode, Node, SurfaceShadow};
use std::rc::Rc;

/// Logical length accepted by `frame`: points or a fraction of the proposal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dim {
    Points(f32),
    Percent(f32),
}
impl Dim {
    pub fn points(value: f32) -> Self {
        Self::Points(value)
    }
    pub fn percent(value: f32) -> Self {
        Self::Percent(value)
    }
}

fn apply_dim_points(
    layout: &LayoutNode,
    dim: Option<Dim>,
    set_points: impl Fn(&LayoutNode, f32),
    set_percent: impl Fn(&LayoutNode, f32),
) {
    match dim {
        Some(Dim::Points(value)) => set_points(layout, value),
        Some(Dim::Percent(value)) => set_percent(layout, value),
        None => {}
    }
}

/// `EdgeInsets` in Swift; `cove_core::Insets` already has the same shape.
pub fn insets_all(value: f32) -> Insets {
    Insets {
        top: value,
        bottom: value,
        leading: value,
        trailing: value,
    }
}
pub fn insets_horizontal(value: f32) -> Insets {
    Insets {
        top: 0.0,
        bottom: 0.0,
        leading: value,
        trailing: value,
    }
}
pub fn insets_vertical(value: f32) -> Insets {
    Insets {
        top: value,
        bottom: value,
        leading: 0.0,
        trailing: 0.0,
    }
}

/// `.padding(EdgeInsets)`.
#[derive(Clone, Copy, Debug)]
pub struct Padding(pub Insets);
impl ViewModifier for Padding {
    fn apply_layout(&self, layout: &LayoutNode) {
        if self.0.top != 0.0 {
            layout.set_padding(self.0.top, Edge::Top);
        }
        if self.0.bottom != 0.0 {
            layout.set_padding(self.0.bottom, Edge::Bottom);
        }
        if self.0.leading != 0.0 {
            layout.set_padding(self.0.leading, Edge::Start);
        }
        if self.0.trailing != 0.0 {
            layout.set_padding(self.0.trailing, Edge::End);
        }
    }
}

/// `.frame(width:height:minWidth:maxWidth:minHeight:maxHeight:)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Frame {
    pub width: Option<Dim>,
    pub height: Option<Dim>,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
}
impl Frame {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn width(mut self, value: Dim) -> Self {
        self.width = Some(value);
        self
    }
    pub fn height(mut self, value: Dim) -> Self {
        self.height = Some(value);
        self
    }
    pub fn min_width(mut self, value: f32) -> Self {
        self.min_width = Some(value);
        self
    }
    pub fn max_width(mut self, value: f32) -> Self {
        self.max_width = Some(value);
        self
    }
    pub fn min_height(mut self, value: f32) -> Self {
        self.min_height = Some(value);
        self
    }
    pub fn max_height(mut self, value: f32) -> Self {
        self.max_height = Some(value);
        self
    }
}
impl ViewModifier for Frame {
    fn apply_layout(&self, layout: &LayoutNode) {
        apply_dim_points(
            layout,
            self.width,
            |l, v| l.set_width(Some(v)),
            |l, v| l.set_width_percent(v),
        );
        apply_dim_points(
            layout,
            self.height,
            |l, v| l.set_height(Some(v)),
            |l, v| l.set_height_percent(v),
        );
        if let Some(value) = self.min_width {
            layout.set_min_width(Some(value));
        }
        if let Some(value) = self.max_width {
            layout.set_max_width(Some(value));
        }
        if let Some(value) = self.min_height {
            layout.set_min_height(Some(value));
        }
        if let Some(value) = self.max_height {
            layout.set_max_height(Some(value));
        }
    }
}

/// `.flex(grow, shrink:basis:)`. A `None` basis is `auto`.
#[derive(Clone, Copy, Debug)]
pub struct Flex {
    pub grow: f32,
    pub shrink: f32,
    pub basis: Option<f32>,
}
impl Flex {
    pub fn new(grow: f32, shrink: f32, basis: Option<f32>) -> Self {
        Self {
            grow,
            shrink,
            basis,
        }
    }
}
impl ViewModifier for Flex {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_flex(self.grow);
        layout.set_flex_shrink(self.shrink);
        match self.basis {
            Some(value) => layout.set_flex_basis(value),
            None => layout.set_flex_basis_auto(),
        }
    }
}

/// Swift `.offset(x:y:)`: a paint and hit-test translation that keeps layout.
#[derive(Clone, Copy, Debug, Default)]
pub struct Offset(pub f32, pub f32);
impl ViewModifier for Offset {
    fn apply_node(&self, node: &Node) {
        node.set_content_offset(cove_core::Point::new(-self.0, -self.1));
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ZIndex(pub f32);
impl ViewModifier for ZIndex {
    fn apply_node(&self, node: &Node) {
        node.set_z_index(self.0);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Opacity(pub f32);
impl ViewModifier for Opacity {
    fn apply_node(&self, node: &Node) {
        node.set_opacity(self.0);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Background(pub cove_core::Color);
impl ViewModifier for Background {
    fn apply_node(&self, node: &Node) {
        node.set_background_color(Some(self.0));
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CornerRadius(pub f32);
impl ViewModifier for CornerRadius {
    fn apply_node(&self, node: &Node) {
        node.set_corner_radius(self.0);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Shadow(pub SurfaceShadow);
impl ViewModifier for Shadow {
    fn apply_node(&self, node: &Node) {
        node.set_shadow(self.0);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ShadowStyle {
    pub color: cove_core::Color,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
}
impl ViewModifier for ShadowStyle {
    fn apply_node(&self, node: &Node) {
        node.set_shadow(SurfaceShadow {
            color: Some(self.color),
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            blur: self.blur,
        });
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Border {
    pub color: cove_core::Color,
    pub width: f32,
}
impl ViewModifier for Border {
    fn apply_node(&self, node: &Node) {
        node.set_border_color(Some(self.color));
        node.set_border_width(self.width);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LayoutDirection(pub Direction);
impl ViewModifier for LayoutDirection {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_direction(self.0);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ClipsToBounds(pub bool);
impl ViewModifier for ClipsToBounds {
    fn apply_node(&self, node: &Node) {
        node.set_clips_to_bounds(self.0);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HitTesting(pub bool);
impl ViewModifier for HitTesting {
    fn apply_node(&self, node: &Node) {
        node.set_allows_hit_testing(self.0);
    }
}

/// `.floatingSurface(radius, elevation:)`.
///
/// Fills with `surfaceFloating`, outlines with the floating hairline and lifts
/// with the theme's layered floating shadow.
#[derive(Clone, Copy, Debug)]
pub struct FloatingSurface {
    pub radius: f32,
    pub elevation: Option<cove_core::LayeredShadow>,
}
impl FloatingSurface {
    pub fn new(radius: f32, elevation: Option<cove_core::LayeredShadow>) -> Self {
        Self { radius, elevation }
    }
}
impl ViewModifier for FloatingSurface {
    fn apply_node(&self, node: &Node) {
        let theme = node.composition_value(&ThemeEnvironment::key());
        node.set_background_color(Some(theme.colors.surface_floating));
        node.set_corner_radius(self.radius);
        node.set_border_color(Some(crate::theme::floating_outline(&theme.colors)));
        node.set_border_width(1.0);
        if let Some(elevation) = self.elevation {
            node.set_shadow(elevation.key);
            node.set_ambient_shadow(elevation.ambient);
        }
    }
}

/// `.animation(_:value:)` is inert without the animation module; the modifier
/// keeps its identity so a port can bind later.
#[derive(Clone, Copy, Debug)]
pub struct AnimationMarker;

/// Convenience callback alias.
pub type VoidCallback = Rc<dyn Fn()>;
pub type SizedCallback<T> = Rc<dyn Fn(T)>;
