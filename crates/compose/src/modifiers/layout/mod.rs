//! Layout modifiers: padding, flex, frames, absolute positioning and authored
//! CSS Grid. Grid geometry itself lives in the core engine (Taffy); the grid
//! module only declares the authored surface for it.
pub mod frame_capture;
mod grid;
mod values;
pub use frame_capture::*;
pub use grid::*;
pub use values::*;

pub use cove_core::scene::{
    GridAreaError, GridAreaSpan, GridAreaTemplate, GridAutoFlow, GridBreadth, GridPlacement,
    GridRepeat, GridTemplateTrack, GridTrack,
};

use crate::{ModifiedContent, View, ViewExt, ViewModifier};
use cove_core::scene::{Edge, Interpolatable, LayoutNode, Node, PositionType};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgeInsets {
    pub top: f32,
    pub leading: f32,
    pub bottom: f32,
    pub trailing: f32,
}
impl EdgeInsets {
    pub fn new(top: f32, leading: f32, bottom: f32, trailing: f32) -> Self {
        Self {
            top,
            leading,
            bottom,
            trailing,
        }
    }
    pub fn all(value: f32) -> Self {
        Self::new(value, value, value, value)
    }
    pub fn horizontal_vertical(horizontal: f32, vertical: f32) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }
}
impl Interpolatable for EdgeInsets {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            top: f32::interpolate(&from.top, &to.top, t),
            leading: f32::interpolate(&from.leading, &to.leading, t),
            bottom: f32::interpolate(&from.bottom, &to.bottom, t),
            trailing: f32::interpolate(&from.trailing, &to.trailing, t),
        }
    }
}

const PADDING_ANIMATION_PROPERTY: &str = "__layout.padding.insets";
const PADDING_ATTACHMENT: &str = "__layout.padding.insets";

pub struct PaddingModifier {
    pub insets: EdgeInsets,
}
impl PaddingModifier {
    pub fn new(insets: EdgeInsets) -> Self {
        Self { insets }
    }
    fn apply(&self, node: &Node, layout: &LayoutNode) {
        let current = layout
            .attachment::<EdgeInsets>(PADDING_ATTACHMENT)
            .unwrap_or_default();
        let target = self.insets;
        let owner = layout.clone();
        node.animatable_set_with(PADDING_ANIMATION_PROPERTY, current, target, move |value| {
            owner.set_attachment(PADDING_ATTACHMENT, value);
            owner.set_padding(value.top, Edge::Top);
            owner.set_padding(value.leading, Edge::Start);
            owner.set_padding(value.bottom, Edge::Bottom);
            owner.set_padding(value.trailing, Edge::End);
        });
    }
}
impl ViewModifier for PaddingModifier {
    fn apply_node(&self, node: &Node) {
        if let Some(layout) = node.layout_node() {
            self.apply(node, &layout);
        }
    }
}

pub struct FlexModifier {
    pub grow: f32,
    pub shrink: f32,
    pub basis: Option<f32>,
}
impl FlexModifier {
    pub fn new(grow: f32, shrink: f32, basis: Option<f32>) -> Self {
        Self {
            grow,
            shrink,
            basis,
        }
    }
    fn apply(&self, node: &Node, layout: &LayoutNode) {
        let grow_node = node.clone();
        let grow_layout = layout.clone();
        node.animatable_set_with(
            "__layout.flex.grow",
            layout.flex_grow(),
            self.grow,
            move |value| {
                grow_layout.set_flex_grow(value);
                grow_node
                    .mark_render_dirty(cove_core::scene::InvalidationSource::style_set("flexGrow"));
            },
        );
        layout.set_flex_shrink(self.shrink);
        // Express the authored basis completely: a reused node keeps the
        // previous branch's basis otherwise, and a stale zero basis collapses
        // the main axis when the container changes direction.
        match self.basis {
            Some(basis) => layout.set_flex_basis(basis),
            None => layout.set_flex_basis_auto(),
        }
    }
}
impl ViewModifier for FlexModifier {
    fn apply_node(&self, node: &Node) {
        if let Some(layout) = node.layout_node() {
            self.apply(node, &layout);
        }
    }
}

/// Fills the parent's main axis unless the content authored a height.
pub struct FillUnlessSizedModifier;
impl ViewModifier for FillUnlessSizedModifier {
    fn apply_node(&self, node: &Node) {
        if let Some(layout) = node.layout_node()
            && matches!(
                FrameModifier::current_height(&layout),
                FrameDimensionSpec::Auto
            )
        {
            FlexModifier::new(1.0, 1.0, Some(0.0)).apply(node, &layout);
        }
    }
}

pub struct FlexShrinkModifier(pub f32);
impl ViewModifier for FlexShrinkModifier {
    fn apply_node(&self, node: &Node) {
        if let Some(layout) = node.layout_node() {
            layout.set_flex_shrink(self.0);
        }
    }
}

pub struct FlexBasisModifier(pub Option<f32>);
impl ViewModifier for FlexBasisModifier {
    fn apply_node(&self, node: &Node) {
        if let Some(layout) = node.layout_node() {
            match self.0 {
                Some(value) => layout.set_flex_basis(value),
                None => layout.set_flex_basis_auto(),
            }
        }
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        match self.0 {
            Some(value) => layout.set_flex_basis(value),
            None => layout.set_flex_basis_auto(),
        }
    }
}

pub struct AbsolutePositionModifier {
    pub left: Option<f32>,
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
}
impl AbsolutePositionModifier {
    pub fn new(
        left: Option<f32>,
        top: Option<f32>,
        right: Option<f32>,
        bottom: Option<f32>,
    ) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
    fn apply(&self, layout: &LayoutNode) {
        layout.set_position_type(PositionType::Absolute);
        for (edge, value) in [
            (Edge::Left, self.left),
            (Edge::Top, self.top),
            (Edge::Right, self.right),
            (Edge::Bottom, self.bottom),
        ] {
            if let Some(value) = value {
                layout.set_position(value, edge);
            } else {
                layout.set_position_auto(edge);
            }
        }
    }
}
impl ViewModifier for AbsolutePositionModifier {
    fn apply_node(&self, node: &Node) {
        if let Some(layout) = node.layout_node() {
            self.apply(&layout);
        }
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        self.apply(layout);
    }
}

pub use cove_core::scene::LayoutStyleValue as FrameDimension;
type FrameDimensionSpec = FrameDimension;

const FRAME_WIDTH_KEY: &str = "__layout.frame.width";
const FRAME_HEIGHT_KEY: &str = "__layout.frame.height";
const FRAME_MIN_WIDTH_KEY: &str = "__layout.frame.minWidth";
const FRAME_MIN_HEIGHT_KEY: &str = "__layout.frame.minHeight";
const FRAME_MAX_WIDTH_KEY: &str = "__layout.frame.maxWidth";
const FRAME_MAX_HEIGHT_KEY: &str = "__layout.frame.maxHeight";
const FRAME_WIDTH_ATTACHMENT: &str = "__layout.frame.widthSpec";
const FRAME_HEIGHT_ATTACHMENT: &str = "__layout.frame.heightSpec";

pub struct FrameModifier {
    pub width: Option<FrameDimension>,
    pub height: Option<FrameDimension>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
}
impl FrameModifier {
    pub fn new(
        width: Option<FrameDimension>,
        height: Option<FrameDimension>,
        min_width: Option<f32>,
        min_height: Option<f32>,
        max_width: Option<f32>,
        max_height: Option<f32>,
    ) -> Self {
        Self {
            width,
            height,
            min_width,
            min_height,
            max_width,
            max_height,
        }
    }

    fn spec(value: FrameDimension) -> FrameDimensionSpec {
        value
    }
    fn apply_width(spec: FrameDimensionSpec, layout: &LayoutNode) {
        let spec = Self::finite_extent(spec);
        layout.set_width_value(spec);
        layout.set_attachment(FRAME_WIDTH_ATTACHMENT, spec);
    }
    fn apply_height(spec: FrameDimensionSpec, layout: &LayoutNode) {
        let spec = Self::finite_extent(spec);
        layout.set_height_value(spec);
        layout.set_attachment(FRAME_HEIGHT_ATTACHMENT, spec);
    }
    /// Reads "as wide/tall as the parent allows" as a percentage, not a point
    /// value of infinity.
    ///
    /// `frame_size(f32::INFINITY, 44.0)` is this framework's spelling for
    /// "fill the parent", but the layout engine solves a point extent literally:
    /// an unbounded parent hands the child `width = inf`, and the write-back
    /// stores `x = -inf, width = inf` on the node. Nothing downstream can paint
    /// that — the draw pass snaps it to a zero-size rect at a NaN origin and the
    /// RHI rejects the whole frame (`invalid UI vertices or indices`). A
    /// percentage cannot produce that geometry: it resolves against the parent's
    /// definite size, and reads as `auto` when the parent has none.
    fn finite_extent(spec: FrameDimensionSpec) -> FrameDimensionSpec {
        match spec {
            FrameDimensionSpec::Points(value) if !value.is_finite() => {
                FrameDimensionSpec::Percent(100.0)
            }
            other => other,
        }
    }
    fn current_width(layout: &LayoutNode) -> FrameDimensionSpec {
        layout
            .attachment::<FrameDimensionSpec>(FRAME_WIDTH_ATTACHMENT)
            .or_else(|| layout.width().map(FrameDimensionSpec::Points))
            .unwrap_or(FrameDimensionSpec::Auto)
    }
    fn current_height(layout: &LayoutNode) -> FrameDimensionSpec {
        layout
            .attachment::<FrameDimensionSpec>(FRAME_HEIGHT_ATTACHMENT)
            .or_else(|| layout.height().map(FrameDimensionSpec::Points))
            .unwrap_or(FrameDimensionSpec::Auto)
    }
    fn apply_dimension(
        node: &Node,
        layout: &LayoutNode,
        key: &'static str,
        current: FrameDimensionSpec,
        target: FrameDimensionSpec,
        width: bool,
    ) {
        match (current, target) {
            (FrameDimensionSpec::Points(from), FrameDimensionSpec::Points(to)) => {
                let owner = layout.clone();
                node.animatable_set_with(key, from, to, move |value| {
                    let spec = FrameDimensionSpec::Points(value);
                    if width {
                        Self::apply_width(spec, &owner);
                    } else {
                        Self::apply_height(spec, &owner);
                    }
                });
            }
            (FrameDimensionSpec::Percent(from), FrameDimensionSpec::Percent(to)) => {
                let owner = layout.clone();
                node.animatable_set_with(key, from, to, move |value| {
                    let spec = FrameDimensionSpec::Percent(value);
                    if width {
                        Self::apply_width(spec, &owner);
                    } else {
                        Self::apply_height(spec, &owner);
                    }
                });
            }
            _ => {
                node.cancel_animation(key);
                if width {
                    Self::apply_width(target, layout);
                } else {
                    Self::apply_height(target, layout);
                }
            }
        }
    }
    fn animate_optional(
        node: &Node,
        layout: &LayoutNode,
        key: &'static str,
        target: Option<f32>,
        write: fn(&LayoutNode, Option<f32>),
    ) {
        let owner = layout.clone();
        node.animatable_set_optional(
            key,
            match key {
                FRAME_MIN_WIDTH_KEY => layout.min_width(),
                FRAME_MIN_HEIGHT_KEY => layout.min_height(),
                FRAME_MAX_WIDTH_KEY => layout.max_width(),
                _ => layout.max_height(),
            },
            target,
            move |value| write(&owner, value),
        );
    }
}
impl ViewModifier for FrameModifier {
    fn apply_node(&self, node: &Node) {
        let Some(layout) = node.layout_node() else {
            return;
        };
        if let Some(width) = self.width {
            Self::apply_dimension(
                node,
                &layout,
                FRAME_WIDTH_KEY,
                Self::current_width(&layout),
                Self::spec(width),
                true,
            );
        }
        if let Some(height) = self.height {
            Self::apply_dimension(
                node,
                &layout,
                FRAME_HEIGHT_KEY,
                Self::current_height(&layout),
                Self::spec(height),
                false,
            );
        }
        if let Some(value) = self.min_width {
            Self::animate_optional(
                node,
                &layout,
                FRAME_MIN_WIDTH_KEY,
                Some(value),
                LayoutNode::set_min_width,
            );
        }
        if let Some(value) = self.min_height {
            Self::animate_optional(
                node,
                &layout,
                FRAME_MIN_HEIGHT_KEY,
                Some(value),
                LayoutNode::set_min_height,
            );
        }
        if let Some(value) = self.max_width {
            Self::animate_optional(
                node,
                &layout,
                FRAME_MAX_WIDTH_KEY,
                Some(value),
                LayoutNode::set_max_width,
            );
        }
        if let Some(value) = self.max_height {
            Self::animate_optional(
                node,
                &layout,
                FRAME_MAX_HEIGHT_KEY,
                Some(value),
                LayoutNode::set_max_height,
            );
        }
    }
}

pub trait PaddingExt: View + Sized + 'static {
    fn padding(self, value: f32) -> ModifiedContent {
        self.padding_insets(EdgeInsets::all(value))
    }
    fn padding_insets(self, insets: EdgeInsets) -> ModifiedContent {
        self.modifier(PaddingModifier::new(insets))
    }
    fn padding_edges(self, horizontal: f32, vertical: f32) -> ModifiedContent {
        self.padding_insets(EdgeInsets::horizontal_vertical(horizontal, vertical))
    }
}
impl<T: View + Sized + 'static> PaddingExt for T {}

pub trait FlexExt: View + Sized + 'static {
    /// Swift's `flex(_:shrink:basis:)` with its default `shrink: 1`.
    fn flex(self, grow: f32) -> ModifiedContent {
        self.modifier(FlexModifier::new(grow, 1.0, None))
    }
    fn flex_with(self, grow: f32, shrink: f32, basis: Option<f32>) -> ModifiedContent {
        self.modifier(FlexModifier::new(grow, shrink, basis))
    }
    fn shrink(self, value: f32) -> ModifiedContent {
        self.modifier(FlexShrinkModifier(value))
    }
    fn flex_basis(self, value: Option<f32>) -> ModifiedContent {
        self.modifier(FlexBasisModifier(value))
    }
}
impl<T: View + Sized + 'static> FlexExt for T {}

pub trait LayoutFrameExt: View + Sized + 'static {
    fn frame(
        self,
        width: Option<FrameDimension>,
        height: Option<FrameDimension>,
        min_width: Option<f32>,
        min_height: Option<f32>,
        max_width: Option<f32>,
        max_height: Option<f32>,
    ) -> ModifiedContent {
        self.modifier(FrameModifier::new(
            width, height, min_width, min_height, max_width, max_height,
        ))
    }
    fn frame_size(self, width: f32, height: f32) -> ModifiedContent {
        self.frame(
            Some(FrameDimension::Points(width)),
            Some(FrameDimension::Points(height)),
            None,
            None,
            None,
            None,
        )
    }
    fn frame_height(self, height: f32) -> ModifiedContent {
        self.frame(
            None,
            Some(FrameDimension::Points(height)),
            None,
            None,
            None,
            None,
        )
    }
    fn frame_min_width(self, width: f32) -> ModifiedContent {
        self.frame(None, None, Some(width), None, None, None)
    }
    fn frame_width(self, width: f32) -> ModifiedContent {
        self.frame(
            Some(FrameDimension::Points(width)),
            None,
            None,
            None,
            None,
            None,
        )
    }
    /// Takes the parent's extent on both axes (CSS `width: 100%; height: 100%`).
    ///
    /// The layout root fills the viewport by itself, but its children size to
    /// their content; an app shell calls this on the view that should cover the
    /// window. A percentage resolves against the parent's definite size, which
    /// a viewport-filling parent always has.
    fn fill(self) -> ModifiedContent {
        self.frame(
            Some(FrameDimension::Percent(100.0)),
            Some(FrameDimension::Percent(100.0)),
            None,
            None,
            None,
            None,
        )
    }
    fn fill_width(self) -> ModifiedContent {
        self.frame(
            Some(FrameDimension::Percent(100.0)),
            None,
            None,
            None,
            None,
            None,
        )
    }
    fn fill_height(self) -> ModifiedContent {
        self.frame(
            None,
            Some(FrameDimension::Percent(100.0)),
            None,
            None,
            None,
            None,
        )
    }
}
impl<T: View + Sized + 'static> LayoutFrameExt for T {}
