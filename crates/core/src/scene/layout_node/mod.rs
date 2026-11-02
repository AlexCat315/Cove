//! Translation of CoveUIScene/LayoutNode.swift.
//!
//! Swift wrapped `YGNodeRef`. Here the flexbox pass is Taffy, driven directly
//! over this tree through its low-level traits (see `engine`): every node keeps
//! its own style, layout cache and result, so a relayout only revisits dirty
//! paths. Frames are parent-relative, exactly as Yoga reports them.
mod custom;
mod engine;
mod grid;
mod style;
mod value;
pub use grid::*;

pub use custom::{CustomLayoutState, CustomMeasure, CustomPlace};
pub use style::{LayoutCalc, LayoutInsets, LayoutStyle, LayoutStyleValue, StyleBorder, StyleEdges};

use super::{
    Align, BoxSizing, Direction, Display, Edge, FlexDirection, Gutter, Justify, Overflow,
    PositionType, Wrap,
};
use crate::{Rect, Size, TextLayoutCacheEntry, TextMeasureInputs};
use engine::EngineState;
use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::rc::{Rc, Weak};

/// How the parent has constrained the measurement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeasureMode {
    /// No constraint.
    #[default]
    Undefined,
    /// Intrinsic size with every soft wrapping opportunity taken.
    MinContent,
    /// Must be exactly this size.
    Exactly,
    /// Up to this size.
    AtMost,
}

pub type MeasureFunc = Rc<dyn Fn(f32, MeasureMode, f32, MeasureMode) -> Size>;

/// The computed rectangle, in parent coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ComputedLayout {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub padding: LayoutInsets,
    pub margin: LayoutInsets,
    pub border: LayoutInsets,
}
impl ComputedLayout {
    pub fn frame(self) -> Rect {
        Rect::new(self.left, self.top, self.width, self.height)
    }
}

/// Authored state of one layout node. Engine state lives beside it in
/// [`LayoutCell`] so layout can read styles while callbacks borrow this.
pub struct LayoutNodeData {
    children: Vec<LayoutNode>,
    parent: LayoutNodeWeak,
    pub style: LayoutStyle,
    measure: Option<MeasureFunc>,
    pub custom: Option<CustomLayoutState>,
    pub placed_frame: Option<Rect>,
    pub placement_is_visible: bool,
    pub subtree_dirty: bool,
    pub dirty: bool,
    pub attachments: BTreeMap<String, Rc<dyn Any>>,
    pub debug_padding: Option<LayoutInsets>,
    pub minimum_size_floor: Option<Size>,
    pub text_measure: Option<TextLayoutCacheEntry>,
    pub text_inputs: Option<TextMeasureInputs>,
}

pub struct LayoutCell {
    data: RefCell<LayoutNodeData>,
    engine: EngineState,
}

pub type LayoutNodeWeak = Weak<LayoutCell>;

/// Wraps one flex layout node and owns its lifetime.
///
/// Create a `LayoutNode` for every `Node` that participates in flexbox layout.
/// After calling `calculate_layout` on the root, read back `frame` to obtain the
/// computed rectangle.
#[derive(Clone)]
pub struct LayoutNode(Rc<LayoutCell>);

impl PartialEq for LayoutNode {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for LayoutNode {}
impl std::fmt::Debug for LayoutNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "LayoutNode({:?})", self.frame())
    }
}

impl Default for LayoutNode {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutNode {
    pub fn new() -> Self {
        Self(Rc::new(LayoutCell {
            data: RefCell::new(LayoutNodeData {
                children: Vec::new(),
                parent: Weak::new(),
                style: LayoutStyle::default(),
                measure: None,
                custom: None,
                placed_frame: None,
                placement_is_visible: true,
                subtree_dirty: true,
                dirty: true,
                attachments: BTreeMap::new(),
                debug_padding: None,
                minimum_size_floor: None,
                text_measure: None,
                text_inputs: None,
            }),
            engine: EngineState::default(),
        }))
    }
    pub(crate) fn from_cell(cell: Rc<LayoutCell>) -> Self {
        Self(cell)
    }
    pub fn downgrade(&self) -> LayoutNodeWeak {
        Rc::downgrade(&self.0)
    }

    /// Upgrade a weak layout-node handle used by retained measure callbacks.
    pub fn upgrade(weak: &LayoutNodeWeak) -> Option<Self> {
        weak.upgrade().map(Self)
    }

    fn data(&self) -> std::cell::Ref<'_, LayoutNodeData> {
        self.0.data.borrow()
    }
    fn data_mut(&self) -> std::cell::RefMut<'_, LayoutNodeData> {
        self.0.data.borrow_mut()
    }

    // MARK: - Tree

    pub fn children(&self) -> Vec<LayoutNode> {
        self.data().children.clone()
    }
    pub fn child_count(&self) -> usize {
        self.data().children.len()
    }
    pub fn parent(&self) -> Option<LayoutNode> {
        self.data().parent.upgrade().map(Self)
    }
    pub fn add_child(&self, child: &LayoutNode) {
        // Yoga cannot measure a node that has children: adding one discards
        // the measurement rather than reviving it when the child leaves.
        if self.data().custom.is_none() {
            self.data_mut().measure = None;
        }
        child.data_mut().parent = Rc::downgrade(&self.0);
        self.data_mut().children.push(child.clone());
        self.mark_layout_dirty_hint();
    }
    pub fn remove_child(&self, child: &LayoutNode) {
        let removed = {
            let mut data = self.data_mut();
            let before = data.children.len();
            data.children.retain(|candidate| candidate != child);
            data.children.len() != before
        };
        if !removed {
            return;
        }
        {
            let mut data = child.data_mut();
            data.parent = Weak::new();
            data.placed_frame = None;
            data.placement_is_visible = true;
        }
        child.0.engine.forget_measurements();
        self.mark_layout_dirty_hint();
    }
    /// Reassign this node's children to `ordered`, which must be the same set.
    pub fn reorder_children(&self, ordered: &[LayoutNode]) {
        let current = self.children();
        assert_eq!(
            ordered.len(),
            current.len(),
            "LayoutNode.reorderChildren: count mismatch"
        );
        let current_ids: HashSet<*const LayoutCell> =
            current.iter().map(|node| Rc::as_ptr(&node.0)).collect();
        let next_ids: HashSet<*const LayoutCell> =
            ordered.iter().map(|node| Rc::as_ptr(&node.0)).collect();
        assert!(
            current_ids == next_ids,
            "LayoutNode.reorderChildren: membership mismatch"
        );
        if current
            .iter()
            .zip(ordered)
            .all(|(left, right)| left == right)
        {
            return;
        }
        self.data_mut().children = ordered.to_vec();
        self.mark_layout_dirty_hint();
    }

    // MARK: - Style

    pub fn style(&self) -> LayoutStyle {
        self.data().style.clone()
    }
    fn update_style(&self, change: impl FnOnce(&mut LayoutStyle)) {
        let changed = {
            let mut data = self.data_mut();
            let before = data.style.clone();
            change(&mut data.style);
            data.style != before
        };
        if changed {
            self.0.engine.invalidate_style();
            self.mark_layout_dirty_hint();
        }
    }
    /// Update authored styles and invalidate the affected layout path.
    pub fn edit_style(&self, change: impl FnOnce(&mut LayoutStyle)) {
        self.update_style(change);
    }
    pub fn set_grid_style(&self, grid: GridStyle) {
        self.update_style(|style| style.grid = grid);
    }
    pub fn set_width_value(&self, value: LayoutStyleValue) {
        self.update_style(|style| style.width = value);
    }
    pub fn set_height_value(&self, value: LayoutStyleValue) {
        self.update_style(|style| style.height = value);
    }
    pub fn set_min_width_value(&self, value: LayoutStyleValue) {
        self.update_style(|style| style.min_width = value);
    }
    pub fn set_max_width_value(&self, value: LayoutStyleValue) {
        self.update_style(|style| style.max_width = value);
    }
    pub fn set_min_height_value(&self, value: LayoutStyleValue) {
        self.update_style(|style| style.min_height = value);
    }
    pub fn set_max_height_value(&self, value: LayoutStyleValue) {
        self.update_style(|style| style.max_height = value);
    }
    pub fn set_flex_basis_value(&self, value: LayoutStyleValue) {
        self.update_style(|style| style.flex_basis = value);
    }
    pub fn set_padding_value(&self, value: LayoutStyleValue, edge: Edge) {
        self.update_style(|style| style.padding.set(edge, value));
    }
    pub fn set_margin_value(&self, value: LayoutStyleValue, edge: Edge) {
        self.update_style(|style| style.margin.set(edge, value));
    }
    pub fn set_position_value(&self, value: LayoutStyleValue, edge: Edge) {
        self.update_style(|style| style.position.set(edge, value));
    }
    pub fn direction(&self) -> Direction {
        self.data().style.direction
    }
    pub fn set_direction(&self, value: Direction) {
        self.update_style(|style| style.direction = value);
    }
    pub fn flex_direction(&self) -> FlexDirection {
        self.data().style.flex_direction
    }
    pub fn set_flex_direction(&self, value: FlexDirection) {
        self.update_style(|style| style.flex_direction = value);
    }
    pub fn align_items(&self) -> Align {
        self.data().style.align_items
    }
    pub fn set_align_items(&self, value: Align) {
        self.update_style(|style| style.align_items = value);
    }
    pub fn align_content(&self) -> Align {
        self.data().style.align_content
    }
    pub fn set_align_content(&self, value: Align) {
        self.update_style(|style| style.align_content = value);
    }
    pub fn justify_content(&self) -> Justify {
        self.data().style.justify_content
    }
    pub fn set_justify_content(&self, value: Justify) {
        self.update_style(|style| style.justify_content = value);
    }
    pub fn flex_wrap(&self) -> Wrap {
        self.data().style.flex_wrap
    }
    pub fn set_flex_wrap(&self, value: Wrap) {
        self.update_style(|style| style.flex_wrap = value);
    }
    pub fn overflow(&self) -> Overflow {
        self.data().style.overflow
    }
    pub fn set_overflow(&self, value: Overflow) {
        self.update_style(|style| style.overflow = value);
    }
    pub fn display(&self) -> Display {
        self.data().style.display
    }
    pub fn set_display(&self, value: Display) {
        self.update_style(|style| style.display = value);
    }
    pub fn align_self(&self) -> Align {
        self.data().style.align_self
    }
    pub fn set_align_self(&self, value: Align) {
        self.update_style(|style| style.align_self = value);
    }
    pub fn position_type(&self) -> PositionType {
        self.data().style.position_type
    }
    pub fn set_position_type(&self, value: PositionType) {
        self.update_style(|style| style.position_type = value);
    }
    pub fn flex_grow(&self) -> f32 {
        self.data().style.flex_grow
    }
    pub fn set_flex_grow(&self, value: f32) {
        self.update_style(|style| style.flex_grow = value);
    }
    pub fn flex_shrink(&self) -> f32 {
        self.data().style.flex_shrink
    }
    pub fn set_flex_shrink(&self, value: f32) {
        self.update_style(|style| style.flex_shrink = value);
    }
    /// `flex` sets grow, shrink and basis together, as Yoga does.
    pub fn flex(&self) -> f32 {
        self.data().style.flex_grow
    }
    pub fn set_flex(&self, value: f32) {
        self.update_style(|style| {
            style.flex_grow = value;
            style.flex_shrink = 1.0;
            style.flex_basis = LayoutStyleValue::Points(0.0);
        });
    }
    pub fn set_flex_basis(&self, value: f32) {
        self.update_style(|style| style.flex_basis = LayoutStyleValue::Points(value));
    }
    pub fn set_flex_basis_percent(&self, value: f32) {
        self.update_style(|style| style.flex_basis = LayoutStyleValue::Percent(value));
    }
    pub fn set_flex_basis_auto(&self) {
        self.update_style(|style| style.flex_basis = LayoutStyleValue::Auto);
    }
    pub fn width(&self) -> Option<f32> {
        match self.data().style.width {
            LayoutStyleValue::Points(value) => Some(value),
            _ => None,
        }
    }
    pub fn set_width(&self, value: Option<f32>) {
        self.update_style(|style| {
            style.width = value.map_or(LayoutStyleValue::Auto, LayoutStyleValue::Points)
        });
    }
    pub fn set_width_percent(&self, value: f32) {
        self.update_style(|style| style.width = LayoutStyleValue::Percent(value));
    }
    pub fn height(&self) -> Option<f32> {
        match self.data().style.height {
            LayoutStyleValue::Points(value) => Some(value),
            _ => None,
        }
    }
    pub fn set_height(&self, value: Option<f32>) {
        self.update_style(|style| {
            style.height = value.map_or(LayoutStyleValue::Auto, LayoutStyleValue::Points)
        });
    }
    pub fn set_height_percent(&self, value: f32) {
        self.update_style(|style| style.height = LayoutStyleValue::Percent(value));
    }
    pub fn min_width(&self) -> Option<f32> {
        self.data().style.min_width.points()
    }
    pub fn set_min_width(&self, value: Option<f32>) {
        self.update_style(|style| {
            style.min_width = value.map_or(LayoutStyleValue::Auto, LayoutStyleValue::Points)
        });
    }
    pub fn max_width(&self) -> Option<f32> {
        self.data().style.max_width.points()
    }
    pub fn set_max_width(&self, value: Option<f32>) {
        self.update_style(|style| {
            style.max_width = value.map_or(LayoutStyleValue::Auto, LayoutStyleValue::Points)
        });
    }
    pub fn min_height(&self) -> Option<f32> {
        self.data().style.min_height.points()
    }
    pub fn set_min_height(&self, value: Option<f32>) {
        self.update_style(|style| {
            style.min_height = value.map_or(LayoutStyleValue::Auto, LayoutStyleValue::Points)
        });
    }
    pub fn max_height(&self) -> Option<f32> {
        self.data().style.max_height.points()
    }
    pub fn set_max_height(&self, value: Option<f32>) {
        self.update_style(|style| {
            style.max_height = value.map_or(LayoutStyleValue::Auto, LayoutStyleValue::Points)
        });
    }
    /// A transient layout floor, independent of authored minimum constraints.
    ///
    /// The authored size stays the source of truth: the floor only raises the
    /// effective minimum, so clearing it restores the authored geometry.
    pub fn set_minimum_size_floor(&self, size: Option<Size>) {
        let normalized = size
            .filter(|size| size.width.is_finite() && size.height.is_finite())
            .map(|size| Size {
                width: size.width.max(0.0),
                height: size.height.max(0.0),
            });
        if self.data().minimum_size_floor == normalized {
            return;
        }
        self.data_mut().minimum_size_floor = normalized;
        self.0.engine.invalidate_style();
        self.mark_layout_dirty_hint();
    }
    pub fn minimum_size_floor(&self) -> Option<Size> {
        self.data().minimum_size_floor
    }
    pub fn box_sizing(&self) -> BoxSizing {
        self.data().style.box_sizing
    }
    pub fn set_box_sizing(&self, value: BoxSizing) {
        self.update_style(|style| style.box_sizing = value);
    }

    // MARK: - Position / Margin / Padding / Border / Gap

    pub fn set_position(&self, value: f32, edge: Edge) {
        self.update_style(|style| style.position.set(edge, LayoutStyleValue::Points(value)));
    }
    pub fn set_position_percent(&self, value: f32, edge: Edge) {
        self.update_style(|style| style.position.set(edge, LayoutStyleValue::Percent(value)));
    }
    pub fn set_position_auto(&self, edge: Edge) {
        self.update_style(|style| style.position.set(edge, LayoutStyleValue::Auto));
    }
    pub fn set_margin(&self, value: f32, edge: Edge) {
        self.update_style(|style| style.margin.set(edge, LayoutStyleValue::Points(value)));
    }
    pub fn set_margin_percent(&self, value: f32, edge: Edge) {
        self.update_style(|style| style.margin.set(edge, LayoutStyleValue::Percent(value)));
    }
    pub fn set_margin_auto(&self, edge: Edge) {
        self.update_style(|style| style.margin.set(edge, LayoutStyleValue::Auto));
    }
    pub fn set_padding(&self, value: f32, edge: Edge) {
        self.update_style(|style| style.padding.set(edge, LayoutStyleValue::Points(value)));
    }
    pub fn set_padding_percent(&self, value: f32, edge: Edge) {
        self.update_style(|style| style.padding.set(edge, LayoutStyleValue::Percent(value)));
    }
    pub fn set_border(&self, value: f32, edge: Edge) {
        self.update_style(|style| style.border.set(edge, value));
    }
    pub fn set_gap(&self, value: f32, gutter: Gutter) {
        self.set_gap_value(LayoutStyleValue::Points(value), gutter);
    }
    pub fn set_gap_percent(&self, value: f32, gutter: Gutter) {
        self.set_gap_value(LayoutStyleValue::Percent(value), gutter);
    }
    pub fn set_gap_value(&self, value: LayoutStyleValue, gutter: Gutter) {
        self.update_style(|style| match gutter {
            Gutter::Row => style.gap_row = value,
            Gutter::Column => style.gap_column = value,
            Gutter::All => {
                style.gap_row = value;
                style.gap_column = value;
            }
        });
    }
    pub fn debug_padding(&self) -> Option<LayoutInsets> {
        self.data().debug_padding
    }
    pub fn set_debug_padding(&self, value: Option<LayoutInsets>) {
        if self.data().debug_padding == value {
            return;
        }
        self.data_mut().debug_padding = value;
        self.0.engine.invalidate_style();
        self.mark_layout_dirty_hint();
    }
    pub fn resolved_padding(&self) -> LayoutInsets {
        LayoutInsets::from_taffy(self.0.engine.layout().padding)
    }
    pub fn resolved_margin(&self) -> LayoutInsets {
        LayoutInsets::from_taffy(self.0.engine.layout().margin)
    }
    pub fn resolved_border(&self) -> LayoutInsets {
        LayoutInsets::from_taffy(self.0.engine.layout().border)
    }

    // MARK: - Attachments and text measure slot

    pub fn attachment<T: Clone + 'static>(&self, key: &str) -> Option<T> {
        self.data()
            .attachments
            .get(key)
            .and_then(|value| value.downcast_ref::<T>().cloned())
    }
    pub fn set_attachment<T: 'static>(&self, key: &str, value: T) {
        self.data_mut()
            .attachments
            .insert(key.to_owned(), Rc::new(value));
    }
    pub fn remove_attachment(&self, key: &str) {
        self.data_mut().attachments.remove(key);
    }
    pub fn text_measure(&self) -> Option<TextLayoutCacheEntry> {
        self.data().text_measure.clone()
    }
    pub fn set_text_measure(&self, entry: Option<TextLayoutCacheEntry>) {
        self.data_mut().text_measure = entry;
    }
    pub fn text_inputs(&self) -> Option<TextMeasureInputs> {
        self.data().text_inputs.clone()
    }
    pub fn set_text_inputs(&self, inputs: Option<TextMeasureInputs>) {
        self.data_mut().text_inputs = inputs;
    }

    // MARK: - Measure

    pub fn set_measure_func(&self, measure: Option<MeasureFunc>) {
        self.data_mut().measure = measure;
        self.mark_layout_dirty_hint();
    }
    /// True for a measured leaf and for a custom layout, whose children are
    /// placed by its own algorithm rather than by flexbox.
    pub fn has_measure_func(&self) -> bool {
        let data = self.data();
        data.custom.is_some() || (data.measure.is_some() && data.children.is_empty())
    }
    /// Mark this node's measurement as stale.
    pub fn mark_dirty(&self) {
        if !self.has_measure_func() {
            return;
        }
        self.mark_layout_dirty_hint();
    }

    // MARK: - Layout calculation

    /// Run flex layout from this node as the root.
    ///
    /// Yoga's `calculateLayout` sizes the root against the owner viewport: a
    /// root dimension that is `auto` while the owner defines one occupies
    /// exactly the viewport. An explicit root `width`/`height` keeps precedence.
    pub fn calculate_layout(
        &self,
        available_width: Option<f32>,
        available_height: Option<f32>,
        direction: Direction,
    ) {
        self.prepare_custom_layouts();
        engine::layout_root(self, available_width, available_height, direction);
        self.place_custom_layouts();
        self.clear_layout_dirty_hints();
    }
    /// True when this node or any descendant still needs a layout pass.
    pub fn subtree_is_dirty(&self) -> bool {
        let data = self.data();
        data.subtree_dirty || data.dirty
    }
    /// The computed rectangle in parent coordinates. Valid only after
    /// `calculate_layout` has been called.
    pub fn frame(&self) -> Rect {
        if let Some(placed) = self.data().placed_frame {
            return placed;
        }
        let layout = self.0.engine.layout();
        Rect::new(
            layout.location.x,
            layout.location.y,
            layout.size.width,
            layout.size.height,
        )
    }
    pub fn computed(&self) -> ComputedLayout {
        let layout = self.0.engine.layout();
        ComputedLayout {
            left: layout.location.x,
            top: layout.location.y,
            width: layout.size.width,
            height: layout.size.height,
            padding: LayoutInsets::from_taffy(layout.padding),
            margin: LayoutInsets::from_taffy(layout.margin),
            border: LayoutInsets::from_taffy(layout.border),
        }
    }

    /// Drops cached results on this node and every ancestor. Stops early at
    /// an ancestor that is already dirty with nothing cached, since its own
    /// ancestors were cleared when it was marked.
    fn mark_layout_dirty_hint(&self) {
        if let Some(custom) = &self.data().custom {
            custom.invalidate();
        }
        let mut cursor = Some(self.clone());
        while let Some(node) = cursor {
            let settled = {
                let mut data = node.data_mut();
                let settled = data.subtree_dirty && node.0.engine.is_clear();
                data.subtree_dirty = true;
                data.dirty = true;
                settled
            };
            node.0.engine.clear_results();
            if settled {
                return;
            }
            cursor = node.parent();
        }
    }
    fn clear_layout_dirty_hints(&self) {
        {
            let mut data = self.data_mut();
            if !data.subtree_dirty {
                return;
            }
            data.subtree_dirty = false;
            data.dirty = false;
        }
        for child in self.children() {
            child.clear_layout_dirty_hints();
        }
    }
}

#[cfg(test)]
mod tests;
