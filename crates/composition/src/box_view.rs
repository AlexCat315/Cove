//! Translation of CoveUIComposeCore/Box.swift.
//!
//! Swift's `Box` collides with Rust's standard allocation type, so the
//! container is exported as `BoxView`. The behaviour and alignment vocabulary
//! are unchanged.
use crate::{AnyView, PrimitiveView, impl_primitive_view};
use cove_core::scene::{Align, FlexDirection, Gutter, Justify, LayoutNode, Node, Wrap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HorizontalAlignment {
    Leading,
    Center,
    Trailing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Alignment {
    TopLeading,
    Top,
    TopTrailing,
    Leading,
    Center,
    Trailing,
    BottomLeading,
    Bottom,
    BottomTrailing,
}

impl HorizontalAlignment {
    pub fn layout_justify(self, direction: FlexDirection) -> Justify {
        match (direction, self) {
            (FlexDirection::Row, Self::Leading) | (FlexDirection::RowReverse, Self::Trailing) => {
                Justify::FlexStart
            }
            (FlexDirection::Row, Self::Trailing) | (FlexDirection::RowReverse, Self::Leading) => {
                Justify::FlexEnd
            }
            (FlexDirection::Row | FlexDirection::RowReverse, Self::Center) => Justify::Center,
            _ => panic!("HorizontalAlignment only maps to row directions"),
        }
    }
    pub fn layout_align(self) -> Align {
        match self {
            Self::Leading => Align::FlexStart,
            Self::Center => Align::Center,
            Self::Trailing => Align::FlexEnd,
        }
    }
}
impl VerticalAlignment {
    pub fn layout_justify(self, direction: FlexDirection) -> Justify {
        match (direction, self) {
            (FlexDirection::Column, Self::Top) | (FlexDirection::ColumnReverse, Self::Bottom) => {
                Justify::FlexStart
            }
            (FlexDirection::Column, Self::Bottom) | (FlexDirection::ColumnReverse, Self::Top) => {
                Justify::FlexEnd
            }
            (FlexDirection::Column | FlexDirection::ColumnReverse, Self::Center) => Justify::Center,
            _ => panic!("VerticalAlignment only maps to column directions"),
        }
    }
    pub fn layout_align(self) -> Align {
        match self {
            Self::Top => Align::FlexStart,
            Self::Center => Align::Center,
            Self::Bottom => Align::FlexEnd,
        }
    }
}
impl Alignment {
    pub fn components(self) -> (HorizontalAlignment, VerticalAlignment) {
        match self {
            Self::TopLeading => (HorizontalAlignment::Leading, VerticalAlignment::Top),
            Self::Top => (HorizontalAlignment::Center, VerticalAlignment::Top),
            Self::TopTrailing => (HorizontalAlignment::Trailing, VerticalAlignment::Top),
            Self::Leading => (HorizontalAlignment::Leading, VerticalAlignment::Center),
            Self::Center => (HorizontalAlignment::Center, VerticalAlignment::Center),
            Self::Trailing => (HorizontalAlignment::Trailing, VerticalAlignment::Center),
            Self::BottomLeading => (HorizontalAlignment::Leading, VerticalAlignment::Bottom),
            Self::Bottom => (HorizontalAlignment::Center, VerticalAlignment::Bottom),
            Self::BottomTrailing => (HorizontalAlignment::Trailing, VerticalAlignment::Bottom),
        }
    }
    pub fn layout_values(self, direction: FlexDirection) -> (Align, Justify) {
        let (horizontal, vertical) = self.components();
        match direction {
            FlexDirection::Row | FlexDirection::RowReverse => (
                vertical.layout_align(),
                horizontal.layout_justify(direction),
            ),
            FlexDirection::Column | FlexDirection::ColumnReverse => (
                horizontal.layout_align(),
                vertical.layout_justify(direction),
            ),
        }
    }
}

/// A flexible container with explicit flex direction. Most layout primitives
/// (`Row`, `Column`) are thin wrappers around it.
#[derive(Clone)]
pub struct BoxView {
    pub direction: FlexDirection,
    pub align_items: Align,
    pub justify_content: Justify,
    pub wrap: Wrap,
    pub spacing: f32,
    pub content: AnyView,
}
impl BoxView {
    pub fn new(
        direction: FlexDirection,
        align_items: Align,
        justify_content: Justify,
        wrap: Wrap,
        spacing: f32,
        content: AnyView,
    ) -> Self {
        Self {
            direction,
            align_items,
            justify_content,
            wrap,
            spacing,
            content,
        }
    }
    pub fn with_alignment(
        direction: FlexDirection,
        alignment: Alignment,
        wrap: Wrap,
        spacing: f32,
        content: AnyView,
    ) -> Self {
        let (align_items, justify_content) = alignment.layout_values(direction);
        Self::new(
            direction,
            align_items,
            justify_content,
            wrap,
            spacing,
            content,
        )
    }
}
impl PrimitiveView for BoxView {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(self.direction);
        layout.set_align_items(self.align_items);
        layout.set_justify_content(self.justify_content);
        layout.set_flex_wrap(self.wrap);
        // A wrapping grid should pack its lines toward the cross-axis start
        // rather than Yoga's default `stretch`, which would inflate row gaps.
        if self.wrap != Wrap::NoWrap {
            layout.set_align_content(Align::FlexStart);
        }
        if self.spacing > 0.0 {
            layout.set_gap(self.spacing, Gutter::All);
        }
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(BoxView);

/// Horizontal stack — a box with `flex_direction: Row`.
#[derive(Clone)]
pub struct Row {
    pub alignment: VerticalAlignment,
    pub spacing: f32,
    pub content: AnyView,
}
impl Row {
    pub fn new(alignment: VerticalAlignment, spacing: f32, content: AnyView) -> Self {
        Self {
            alignment,
            spacing,
            content,
        }
    }
}
impl PrimitiveView for Row {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(self.alignment.layout_align());
        if self.spacing > 0.0 {
            layout.set_gap(self.spacing, Gutter::All);
        }
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(Row);

/// Vertical stack — a box with `flex_direction: Column`.
#[derive(Clone)]
pub struct Column {
    pub alignment: HorizontalAlignment,
    pub spacing: f32,
    pub content: AnyView,
}
impl Column {
    pub fn new(alignment: HorizontalAlignment, spacing: f32, content: AnyView) -> Self {
        Self {
            alignment,
            spacing,
            content,
        }
    }
}
impl PrimitiveView for Column {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(self.alignment.layout_align());
        if self.spacing > 0.0 {
            layout.set_gap(self.spacing, Gutter::All);
        }
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(Column);

/// Greedy spacer that absorbs free space along the parent's main axis.
#[derive(Clone, Copy, Default)]
pub struct Spacer {
    pub min_length: f32,
}
impl Spacer {
    pub fn new(min_length: f32) -> Self {
        Self { min_length }
    }
}
impl PrimitiveView for Spacer {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_grow(1.0);
        if self.min_length > 0.0 {
            layout.set_flex_basis(self.min_length);
        }
    }
}
impl_primitive_view!(Spacer);
