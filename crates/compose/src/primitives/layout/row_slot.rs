//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/RowSlot.swift.
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode};

/// The host of one list, tree or menu row. Content spans the row's width and
/// sits on its vertical center.
pub fn row_slot(height: Option<f32>, min_height: Option<f32>) -> LayoutNode {
    let layout = LayoutNode::new();
    update_row_slot(&layout, height, min_height);
    layout
}
pub fn update_row_slot(layout: &LayoutNode, height: Option<f32>, min_height: Option<f32>) {
    layout.set_flex_direction(FlexDirection::Column);
    layout.set_align_items(Align::Stretch);
    layout.set_justify_content(Justify::Center);
    layout.set_height(height);
    layout.set_min_height(min_height);
}
