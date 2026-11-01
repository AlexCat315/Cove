//! Expression-aware layout modifiers. Existing point-based modifiers remain available.
use crate::{ModifiedContent, View, ViewExt, ViewModifier};
use cove_core::scene::{Edge, GridStyle, Gutter, LayoutNode, LayoutStyleValue, Node};

#[derive(Clone, Copy, Debug, Default)]
pub struct FrameValues {
    pub width: Option<LayoutStyleValue>,
    pub height: Option<LayoutStyleValue>,
    pub min_width: Option<LayoutStyleValue>,
    pub min_height: Option<LayoutStyleValue>,
    pub max_width: Option<LayoutStyleValue>,
    pub max_height: Option<LayoutStyleValue>,
}
impl ViewModifier for FrameValues {
    fn apply_node(&self, node: &Node) {
        if let Some(layout) = node.layout_node() {
            // Cancel point animations before replacing the authored expression.
            for (value, key) in [
                (self.width, super::FRAME_WIDTH_KEY), (self.height, super::FRAME_HEIGHT_KEY),
                (self.min_width, super::FRAME_MIN_WIDTH_KEY), (self.min_height, super::FRAME_MIN_HEIGHT_KEY),
                (self.max_width, super::FRAME_MAX_WIDTH_KEY), (self.max_height, super::FRAME_MAX_HEIGHT_KEY),
            ] { if value.is_some() { node.cancel_animation(key); } }
            if let Some(value) = self.width { super::FrameModifier::apply_width(value, &layout); }
            if let Some(value) = self.height { super::FrameModifier::apply_height(value, &layout); }
            self.apply_layout(&layout);
        }
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.edit_style(|style| {
            if let Some(value) = self.width { style.width = value; }
            if let Some(value) = self.height { style.height = value; }
            if let Some(value) = self.min_width { style.min_width = value; }
            if let Some(value) = self.min_height { style.min_height = value; }
            if let Some(value) = self.max_width { style.max_width = value; }
            if let Some(value) = self.max_height { style.max_height = value; }
        });
    }
}
#[derive(Clone)]
pub enum LayoutValueModifier {
    Padding(LayoutStyleValue, Edge),
    Margin(LayoutStyleValue, Edge),
    Position(LayoutStyleValue, Edge),
    Gap(LayoutStyleValue, Gutter),
    FlexBasis(LayoutStyleValue),
    Grid(GridStyle),
}
impl ViewModifier for LayoutValueModifier {
    fn apply_node(&self, node: &Node) {
        if matches!(self, Self::Padding(..)) { node.cancel_animation(super::PADDING_ANIMATION_PROPERTY); }
        if let Some(layout) = node.layout_node() { self.apply_layout(&layout); }
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        match self {
            Self::Padding(value, edge) => layout.set_padding_value(*value, *edge),
            Self::Margin(value, edge) => layout.set_margin_value(*value, *edge),
            Self::Position(value, edge) => layout.set_position_value(*value, *edge),
            Self::Gap(value, gutter) => layout.set_gap_value(*value, *gutter),
            Self::FlexBasis(value) => layout.set_flex_basis_value(*value),
            Self::Grid(value) => layout.set_grid_style(value.clone()),
        }
    }
}
pub trait LayoutValuesExt: View + Sized + 'static {
    fn frame_values(self, values: FrameValues) -> ModifiedContent { self.modifier(values) }
    fn padding_value(self, value: LayoutStyleValue, edge: Edge) -> ModifiedContent { self.modifier(LayoutValueModifier::Padding(value, edge)) }
    fn margin_value(self, value: LayoutStyleValue, edge: Edge) -> ModifiedContent { self.modifier(LayoutValueModifier::Margin(value, edge)) }
    fn position_value(self, value: LayoutStyleValue, edge: Edge) -> ModifiedContent { self.modifier(LayoutValueModifier::Position(value, edge)) }
    fn gap_value(self, value: LayoutStyleValue, gutter: Gutter) -> ModifiedContent { self.modifier(LayoutValueModifier::Gap(value, gutter)) }
    fn flex_basis_value(self, value: LayoutStyleValue) -> ModifiedContent { self.modifier(LayoutValueModifier::FlexBasis(value)) }
    /// Grid item placement or container tracks. Use CssGrid for a grid container.
    fn grid_style(self, style: GridStyle) -> ModifiedContent { self.modifier(LayoutValueModifier::Grid(style)) }
}
impl<T: View + Sized + 'static> LayoutValuesExt for T {}
