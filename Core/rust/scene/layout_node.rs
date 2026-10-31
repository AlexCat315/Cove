//! Translation of CoveUIScene/LayoutNode.swift.
//!
//! Swift wraps `YGNodeRef` (Yoga). Yoga is not vendored in this repository, so
//! the flexbox pass it provided is implemented here: the same style vocabulary,
//! the same measure-callback contract, and the same readback surface.
use super::{
    Align, BoxSizing, Direction, Display, Edge, FlexDirection, Gutter, Justify, Overflow,
    PositionType, Wrap,
};
use crate::{Point, Rect, Size, TextLayoutCacheEntry, TextMeasureInputs};
use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LayoutStyleValue {
    #[default]
    Auto,
    Points(f32),
    Percent(f32),
}
impl LayoutStyleValue {
    /// Prefer an explicit physical value; fall back to the logical one.
    fn or_logical(self, logical: LayoutStyleValue) -> LayoutStyleValue {
        if matches!(self, Self::Auto) {
            logical
        } else {
            self
        }
    }
    fn resolve(self, basis: Option<f32>) -> Option<f32> {
        match self {
            Self::Auto => None,
            Self::Points(value) => Some(value),
            Self::Percent(percent) => basis.map(|basis| basis * percent / 100.0),
        }
    }
    fn is_auto(self) -> bool {
        matches!(self, Self::Auto)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
impl LayoutInsets {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }
    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

/// Per-edge style values. Logical `start`/`end` resolve against `direction`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StyleEdges {
    pub top: LayoutStyleValue,
    pub right: LayoutStyleValue,
    pub bottom: LayoutStyleValue,
    pub left: LayoutStyleValue,
    /// Logical insets kept unresolved so they can bind to the effective
    /// direction at layout time. A view that sets `Edge::End` before it has a
    /// parent would otherwise bake the physical edge from `Direction::Inherit`
    /// (i.e. always LTR), and no later `Rtl` could undo it.
    pub start: LayoutStyleValue,
    pub end: LayoutStyleValue,
}
impl StyleEdges {
    pub fn set(&mut self, edge: Edge, value: LayoutStyleValue, direction: Direction) {
        let rtl = direction == Direction::Rtl;
        match edge {
            Edge::All => {
                self.top = value;
                self.right = value;
                self.bottom = value;
                self.left = value;
            }
            Edge::Horizontal => {
                self.right = value;
                self.left = value;
            }
            Edge::Vertical => {
                self.top = value;
                self.bottom = value;
            }
            Edge::Top => self.top = value,
            Edge::Bottom => self.bottom = value,
            Edge::Left => self.left = value,
            Edge::Right => self.right = value,
            Edge::Start => {
                self.start = value;
                if rtl {
                    self.right = value;
                } else {
                    self.left = value;
                }
            }
            Edge::End => {
                self.end = value;
                if rtl {
                    self.left = value;
                } else {
                    self.right = value;
                }
            }
        }
    }
    /// Percentages resolve against the containing block's width, as in CSS/Yoga.
    fn resolve(self, basis: Option<f32>) -> LayoutInsets {
        let axis = |value: LayoutStyleValue| value.resolve(basis).unwrap_or(0.0);
        LayoutInsets {
            top: axis(self.top),
            right: axis(self.right),
            bottom: axis(self.bottom),
            left: axis(self.left),
        }
    }
    fn resolve_axis(self, horizontal: bool, basis: Option<f32>) -> (f32, f32) {
        if horizontal {
            (
                self.left.resolve(basis).unwrap_or(0.0),
                self.right.resolve(basis).unwrap_or(0.0),
            )
        } else {
            (
                self.top.resolve(basis).unwrap_or(0.0),
                self.bottom.resolve(basis).unwrap_or(0.0),
            )
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StyleBorder {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
impl StyleBorder {
    fn set(&mut self, edge: Edge, value: f32) {
        match edge {
            Edge::All => {
                self.top = value;
                self.right = value;
                self.bottom = value;
                self.left = value;
            }
            Edge::Horizontal => {
                self.right = value;
                self.left = value;
            }
            Edge::Vertical => {
                self.top = value;
                self.bottom = value;
            }
            Edge::Top => self.top = value,
            Edge::Bottom => self.bottom = value,
            Edge::Left | Edge::Start => self.left = value,
            Edge::Right | Edge::End => self.right = value,
        }
    }
    fn insets(self) -> LayoutInsets {
        LayoutInsets {
            top: self.top,
            right: self.right,
            bottom: self.bottom,
            left: self.left,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutStyle {
    pub direction: Direction,
    pub flex_direction: FlexDirection,
    pub align_items: Align,
    pub align_content: Align,
    pub justify_content: Justify,
    pub flex_wrap: Wrap,
    pub overflow: Overflow,
    pub display: Display,
    pub align_self: Align,
    pub position_type: PositionType,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: LayoutStyleValue,
    pub width: LayoutStyleValue,
    pub height: LayoutStyleValue,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub position: StyleEdges,
    pub margin: StyleEdges,
    pub padding: StyleEdges,
    pub border: StyleBorder,
    pub gap_row: LayoutStyleValue,
    pub gap_column: LayoutStyleValue,
    pub box_sizing: BoxSizing,
}
impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            direction: Direction::Inherit,
            flex_direction: FlexDirection::Column,
            align_items: Align::Stretch,
            align_content: Align::FlexStart,
            justify_content: Justify::FlexStart,
            flex_wrap: Wrap::NoWrap,
            overflow: Overflow::Visible,
            display: Display::Flex,
            align_self: Align::Auto,
            position_type: PositionType::Relative,
            flex_grow: 0.0,
            flex_shrink: 0.0,
            flex_basis: LayoutStyleValue::Auto,
            width: LayoutStyleValue::Auto,
            height: LayoutStyleValue::Auto,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            position: StyleEdges::default(),
            margin: StyleEdges::default(),
            padding: StyleEdges::default(),
            border: StyleBorder::default(),
            gap_row: LayoutStyleValue::Auto,
            gap_column: LayoutStyleValue::Auto,
            box_sizing: BoxSizing::BorderBox,
        }
    }
}

/// How the parent has constrained the measurement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeasureMode {
    /// No constraint.
    #[default]
    Undefined,
    /// Must be exactly this size.
    Exactly,
    /// Up to this size.
    AtMost,
}

pub type MeasureFunc = Rc<dyn Fn(f32, MeasureMode, f32, MeasureMode) -> Size>;
pub type CustomMeasure = Rc<dyn Fn(Option<f32>, Option<f32>, &[LayoutNode]) -> Size>;
pub type CustomPlace = Rc<dyn Fn(Rect, &[LayoutNode])>;

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

pub struct CustomLayoutState {
    pub measure: RefCell<CustomMeasure>,
    pub place: RefCell<CustomPlace>,
    placement_bounds: RefCell<Option<(Rect, Direction)>>,
}
impl CustomLayoutState {
    pub fn new(measure: CustomMeasure, place: CustomPlace) -> Self {
        Self {
            measure: RefCell::new(measure),
            place: RefCell::new(place),
            placement_bounds: RefCell::new(None),
        }
    }
    pub fn invalidate(&self) {
        *self.placement_bounds.borrow_mut() = None;
    }
}

pub struct LayoutNodeData {
    children: Vec<LayoutNode>,
    parent: LayoutNodeWeak,
    pub style: LayoutStyle,
    pub computed: ComputedLayout,
    measure: Option<MeasureFunc>,
    pub custom: Option<CustomLayoutState>,
    pub placed_frame: Option<Rect>,
    pub placement_is_visible: bool,
    pub subtree_dirty: bool,
    pub dirty: bool,
    pub attachments: BTreeMap<String, Rc<dyn Any>>,
    pub debug_padding: Option<LayoutInsets>,
    pub text_measure: Option<TextLayoutCacheEntry>,
    pub text_inputs: Option<TextMeasureInputs>,
}

pub type LayoutNodeWeak = Weak<RefCell<LayoutNodeData>>;

/// Wraps the flex layout node and owns its lifetime.
///
/// Create a `LayoutNode` for every `Node` that participates in flexbox layout.
/// After calling `calculate_layout` on the root, read back `frame` to obtain the
/// computed rectangle.
#[derive(Clone)]
pub struct LayoutNode(Rc<RefCell<LayoutNodeData>>);

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
        Self(Rc::new(RefCell::new(LayoutNodeData {
            children: Vec::new(),
            parent: Weak::new(),
            style: LayoutStyle::default(),
            computed: ComputedLayout::default(),
            measure: None,
            custom: None,
            placed_frame: None,
            placement_is_visible: true,
            subtree_dirty: true,
            dirty: true,
            attachments: BTreeMap::new(),
            debug_padding: None,
            text_measure: None,
            text_inputs: None,
        })))
    }
    pub(crate) fn from_cell(cell: Rc<RefCell<LayoutNodeData>>) -> Self {
        Self(cell)
    }
    pub fn downgrade(&self) -> LayoutNodeWeak {
        Rc::downgrade(&self.0)
    }

    /// Upgrade a weak layout-node handle used by retained measure callbacks.
    pub fn upgrade(weak: &LayoutNodeWeak) -> Option<Self> {
        weak.upgrade().map(Self)
    }
    pub(crate) fn cell(&self) -> &Rc<RefCell<LayoutNodeData>> {
        &self.0
    }

    // MARK: - Tree

    pub fn children(&self) -> Vec<LayoutNode> {
        self.0.borrow().children.clone()
    }
    pub fn parent(&self) -> Option<LayoutNode> {
        self.0.borrow().parent.upgrade().map(Self::from_cell)
    }
    pub fn add_child(&self, child: &LayoutNode) {
        if self.0.borrow().custom.is_none() {
            self.set_measure_func(None);
        }
        child.0.borrow_mut().parent = Rc::downgrade(&self.0);
        self.0.borrow_mut().children.push(child.clone());
        self.sync_measure_func_with_children();
        self.mark_layout_dirty_hint();
    }
    pub fn remove_child(&self, child: &LayoutNode) {
        let removed = {
            let mut data = self.0.borrow_mut();
            let before = data.children.len();
            data.children.retain(|candidate| candidate != child);
            data.children.len() != before
        };
        if !removed {
            return;
        }
        child.0.borrow_mut().parent = Weak::new();
        child.0.borrow_mut().placed_frame = None;
        child.0.borrow_mut().placement_is_visible = true;
        self.sync_measure_func_with_children();
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
        assert!(
            current.iter().all(|node| ordered.contains(node)),
            "LayoutNode.reorderChildren: membership mismatch"
        );
        if current
            .iter()
            .zip(ordered)
            .all(|(left, right)| left == right)
        {
            return;
        }
        self.0.borrow_mut().children = ordered.to_vec();
        self.mark_layout_dirty_hint();
    }

    fn sync_measure_func_with_children(&self) {
        let has_children = !self.0.borrow().children.is_empty();
        let wants_measure = self.0.borrow().measure.is_some() && !has_children;
        if wants_measure != self.has_measure_func() {
            // The callback is stored on the node itself; only the effective flag matters.
            self.mark_layout_dirty_hint();
        }
    }

    // MARK: - Style

    pub fn style(&self) -> LayoutStyle {
        self.0.borrow().style
    }
    fn update_style(&self, change: impl FnOnce(&mut LayoutStyle)) {
        let before = self.0.borrow().style;
        change(&mut self.0.borrow_mut().style);
        if self.0.borrow().style != before {
            self.mark_layout_dirty_hint();
        }
    }
    pub fn direction(&self) -> Direction {
        self.0.borrow().style.direction
    }
    pub fn set_direction(&self, value: Direction) {
        self.update_style(|style| style.direction = value);
    }
    pub fn flex_direction(&self) -> FlexDirection {
        self.0.borrow().style.flex_direction
    }
    pub fn set_flex_direction(&self, value: FlexDirection) {
        self.update_style(|style| style.flex_direction = value);
    }
    pub fn align_items(&self) -> Align {
        self.0.borrow().style.align_items
    }
    pub fn set_align_items(&self, value: Align) {
        self.update_style(|style| style.align_items = value);
    }
    pub fn align_content(&self) -> Align {
        self.0.borrow().style.align_content
    }
    pub fn set_align_content(&self, value: Align) {
        self.update_style(|style| style.align_content = value);
    }
    pub fn justify_content(&self) -> Justify {
        self.0.borrow().style.justify_content
    }
    pub fn set_justify_content(&self, value: Justify) {
        self.update_style(|style| style.justify_content = value);
    }
    pub fn flex_wrap(&self) -> Wrap {
        self.0.borrow().style.flex_wrap
    }
    pub fn set_flex_wrap(&self, value: Wrap) {
        self.update_style(|style| style.flex_wrap = value);
    }
    pub fn overflow(&self) -> Overflow {
        self.0.borrow().style.overflow
    }
    pub fn set_overflow(&self, value: Overflow) {
        self.update_style(|style| style.overflow = value);
    }
    pub fn display(&self) -> Display {
        self.0.borrow().style.display
    }
    pub fn set_display(&self, value: Display) {
        self.update_style(|style| style.display = value);
    }
    pub fn align_self(&self) -> Align {
        self.0.borrow().style.align_self
    }
    pub fn set_align_self(&self, value: Align) {
        self.update_style(|style| style.align_self = value);
    }
    pub fn position_type(&self) -> PositionType {
        self.0.borrow().style.position_type
    }
    pub fn set_position_type(&self, value: PositionType) {
        self.update_style(|style| style.position_type = value);
    }
    pub fn flex_grow(&self) -> f32 {
        self.0.borrow().style.flex_grow
    }
    pub fn set_flex_grow(&self, value: f32) {
        self.update_style(|style| style.flex_grow = value);
    }
    pub fn flex_shrink(&self) -> f32 {
        self.0.borrow().style.flex_shrink
    }
    pub fn set_flex_shrink(&self, value: f32) {
        self.update_style(|style| style.flex_shrink = value);
    }
    /// `flex` sets grow, shrink and basis together, as Yoga does.
    pub fn flex(&self) -> f32 {
        self.0.borrow().style.flex_grow
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
        match self.0.borrow().style.width {
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
        match self.0.borrow().style.height {
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
        self.0.borrow().style.min_width
    }
    pub fn set_min_width(&self, value: Option<f32>) {
        self.update_style(|style| style.min_width = value);
    }
    pub fn max_width(&self) -> Option<f32> {
        self.0.borrow().style.max_width
    }
    pub fn set_max_width(&self, value: Option<f32>) {
        self.update_style(|style| style.max_width = value);
    }
    pub fn min_height(&self) -> Option<f32> {
        self.0.borrow().style.min_height
    }
    pub fn set_min_height(&self, value: Option<f32>) {
        self.update_style(|style| style.min_height = value);
    }
    pub fn max_height(&self) -> Option<f32> {
        self.0.borrow().style.max_height
    }
    pub fn set_max_height(&self, value: Option<f32>) {
        self.update_style(|style| style.max_height = value);
    }
    /// A transient layout floor, independent of authored minimum constraints.
    pub fn set_minimum_size_floor(&self, size: Option<Size>) {
        let normalized = size
            .filter(|size| size.width.is_finite() && size.height.is_finite())
            .map(|size| Size {
                width: size.width.max(0.0),
                height: size.height.max(0.0),
            });
        let previous: Option<Option<Size>> = self.attachment("__minimumSizeFloor");
        if previous.flatten() == normalized {
            return;
        }
        self.set_attachment("__minimumSizeFloor", normalized);
        self.apply_minimum_size();
    }
    fn effective_minimum(&self) -> (Option<f32>, Option<f32>) {
        // The attachment stores the whole `Option<Size>`, so it must be read back
        // at that type: `attachment::<Size>()` asks for a `Size` and the downcast
        // fails, which silently disabled the floor.
        let floor: Option<Size> = self.attachment::<Option<Size>>("__minimumSizeFloor").flatten();
        let style = self.0.borrow().style;
        (
            floor.map_or(style.min_width, |floor| {
                Some(style.min_width.unwrap_or(0.0).max(floor.width))
            }),
            floor.map_or(style.min_height, |floor| {
                Some(style.min_height.unwrap_or(0.0).max(floor.height))
            }),
        )
    }
    fn apply_minimum_size(&self) {
        // The authored `width`/`height` stays the source of truth. `measure` and
        // `arrange` clamp it against `effective_minimum()`, so setting or clearing
        // the floor is reversible — overwriting the style here would destroy the
        // authored value and leave nothing to restore when the floor goes away.
        self.mark_layout_dirty_hint();
    }
    pub fn box_sizing(&self) -> BoxSizing {
        self.0.borrow().style.box_sizing
    }
    pub fn set_box_sizing(&self, value: BoxSizing) {
        self.update_style(|style| style.box_sizing = value);
    }

    // MARK: - Position / Margin / Padding / Border / Gap

    fn edge_direction(&self) -> Direction {
        self.0.borrow().style.direction
    }
    pub fn set_position(&self, value: f32, edge: Edge) {
        let direction = self.edge_direction();
        self.update_style(|style| {
            style
                .position
                .set(edge, LayoutStyleValue::Points(value), direction)
        });
    }
    pub fn set_position_percent(&self, value: f32, edge: Edge) {
        let direction = self.edge_direction();
        self.update_style(|style| {
            style
                .position
                .set(edge, LayoutStyleValue::Percent(value), direction)
        });
    }
    pub fn set_position_auto(&self, edge: Edge) {
        let direction = self.edge_direction();
        self.update_style(|style| style.position.set(edge, LayoutStyleValue::Auto, direction));
    }
    pub fn set_margin(&self, value: f32, edge: Edge) {
        let direction = self.edge_direction();
        self.update_style(|style| {
            style
                .margin
                .set(edge, LayoutStyleValue::Points(value), direction)
        });
    }
    pub fn set_margin_percent(&self, value: f32, edge: Edge) {
        let direction = self.edge_direction();
        self.update_style(|style| {
            style
                .margin
                .set(edge, LayoutStyleValue::Percent(value), direction)
        });
    }
    pub fn set_margin_auto(&self, edge: Edge) {
        let direction = self.edge_direction();
        self.update_style(|style| style.margin.set(edge, LayoutStyleValue::Auto, direction));
    }
    pub fn set_padding(&self, value: f32, edge: Edge) {
        let direction = self.edge_direction();
        self.update_style(|style| {
            style
                .padding
                .set(edge, LayoutStyleValue::Points(value), direction)
        });
    }
    pub fn set_padding_percent(&self, value: f32, edge: Edge) {
        let direction = self.edge_direction();
        self.update_style(|style| {
            style
                .padding
                .set(edge, LayoutStyleValue::Percent(value), direction)
        });
    }
    pub fn set_border(&self, value: f32, edge: Edge) {
        self.update_style(|style| style.border.set(edge, value));
    }
    pub fn set_gap(&self, value: f32, gutter: Gutter) {
        self.update_style(|style| match gutter {
            Gutter::Row => style.gap_row = LayoutStyleValue::Points(value),
            Gutter::Column => style.gap_column = LayoutStyleValue::Points(value),
            Gutter::All => {
                style.gap_row = LayoutStyleValue::Points(value);
                style.gap_column = LayoutStyleValue::Points(value);
            }
        });
    }
    pub fn set_gap_percent(&self, value: f32, gutter: Gutter) {
        self.update_style(|style| match gutter {
            Gutter::Row => style.gap_row = LayoutStyleValue::Percent(value),
            Gutter::Column => style.gap_column = LayoutStyleValue::Percent(value),
            Gutter::All => {
                style.gap_row = LayoutStyleValue::Percent(value);
                style.gap_column = LayoutStyleValue::Percent(value);
            }
        });
    }
    pub fn debug_padding(&self) -> Option<LayoutInsets> {
        self.0.borrow().debug_padding
    }
    pub fn set_debug_padding(&self, value: Option<LayoutInsets>) {
        if self.0.borrow().debug_padding == value {
            return;
        }
        self.0.borrow_mut().debug_padding = value;
        self.mark_layout_dirty_hint();
    }
    pub fn resolved_padding(&self) -> LayoutInsets {
        self.0.borrow().computed.padding
    }
    pub fn resolved_margin(&self) -> LayoutInsets {
        self.0.borrow().computed.margin
    }
    pub fn resolved_border(&self) -> LayoutInsets {
        self.0.borrow().computed.border
    }

    // MARK: - Attachments and text measure slot

    pub fn attachment<T: Clone + 'static>(&self, key: &str) -> Option<T> {
        self.0
            .borrow()
            .attachments
            .get(key)
            .and_then(|value| value.downcast_ref::<T>().cloned())
    }
    pub fn set_attachment<T: 'static>(&self, key: &str, value: T) {
        self.0
            .borrow_mut()
            .attachments
            .insert(key.to_owned(), Rc::new(value));
    }
    pub fn text_measure(&self) -> Option<TextLayoutCacheEntry> {
        self.0.borrow().text_measure.clone()
    }
    pub fn set_text_measure(&self, entry: Option<TextLayoutCacheEntry>) {
        self.0.borrow_mut().text_measure = entry;
    }
    pub fn text_inputs(&self) -> Option<TextMeasureInputs> {
        self.0.borrow().text_inputs.clone()
    }
    pub fn set_text_inputs(&self, inputs: Option<TextMeasureInputs>) {
        self.0.borrow_mut().text_inputs = inputs;
    }

    // MARK: - Measure

    pub fn set_measure_func(&self, measure: Option<MeasureFunc>) {
        self.0.borrow_mut().measure = measure;
        self.mark_layout_dirty_hint();
    }
    pub fn has_measure_func(&self) -> bool {
        // Swift installs a real Yoga measure trampoline for a custom layout, so
        // `hasMeasureFunc` is true for a custom-layout node even when it owns
        // authored children. Reporting false here made `mark_dirty` a silent
        // no-op for exactly those nodes.
        let data = self.0.borrow();
        data.custom.is_some() || (data.measure.is_some() && data.children.is_empty())
    }
    /// Mark this node's measurement as stale.
    pub fn mark_dirty(&self) {
        if !self.has_measure_func() {
            return;
        }
        self.0.borrow_mut().dirty = true;
        self.mark_layout_dirty_hint();
    }
    fn measure_size(
        &self,
        width: f32,
        width_mode: MeasureMode,
        height: f32,
        height_mode: MeasureMode,
    ) -> Size {
        let measure = self.0.borrow().measure.clone();
        measure.map_or(Size::default(), |measure| {
            measure(width, width_mode, height, height_mode)
        })
    }

    // MARK: - Layout calculation

    /// Run flex layout from this node as the root.
    ///
    /// Yoga's `calculateLayout` sizes the root against the owner viewport. A
    /// root dimension that is `auto` while the owner defines one is
    /// `StretchFit`: the root occupies exactly the viewport. Without that rule a
    /// viewport-filling root (`Box {}.flex(1, shrink: 1, basis: 0)`) measures at
    /// `AtMost` and collapses to its content height, so `flex_grow` has no free
    /// space to distribute.
    ///
    /// An explicit root `width`/`height` keeps precedence, which is the
    /// `hasDefiniteLength` branch in Yoga.
    pub fn calculate_layout(
        &self,
        available_width: Option<f32>,
        available_height: Option<f32>,
        direction: Direction,
    ) {
        self.prepare_custom_layouts();
        let style = self.0.borrow().style;
        let (padding, border, _) = self.resolved_style_insets(available_width);
        let inset_width = padding.horizontal() + border.horizontal();
        let inset_height = padding.vertical() + border.vertical();
        let stretched_width = style.width.is_auto().then(|| {
            available_width.map(|width| match style.box_sizing {
                BoxSizing::ContentBox => (width - inset_width).max(0.0),
                BoxSizing::BorderBox => width,
            })
        });
        let stretched_height = style.height.is_auto().then(|| {
            available_height.map(|height| match style.box_sizing {
                BoxSizing::ContentBox => (height - inset_height).max(0.0),
                BoxSizing::BorderBox => height,
            })
        });
        let exact_width = stretched_width.flatten();
        let exact_height = stretched_height.flatten();
        let size = self.measure(
            available_width,
            available_height,
            exact_width,
            exact_height,
            direction,
        );
        let size = Size {
            width: exact_width.unwrap_or(size.width),
            height: exact_height.unwrap_or(size.height),
        };
        self.arrange(0.0, 0.0, size.width, size.height, direction);
        self.place_custom_layouts();
        self.clear_layout_dirty_hints();
    }
    /// True when this node or any descendant still needs a layout pass.
    pub fn subtree_is_dirty(&self) -> bool {
        let data = self.0.borrow();
        data.subtree_dirty || data.dirty
    }
    /// The computed rectangle. Valid only after `calculate_layout` has been called.
    pub fn frame(&self) -> Rect {
        self.0
            .borrow()
            .placed_frame
            .unwrap_or_else(|| self.0.borrow().computed.frame())
    }
    pub fn computed(&self) -> ComputedLayout {
        self.0.borrow().computed
    }

    fn mark_layout_dirty_hint(&self) {
        if let Some(custom) = &self.0.borrow().custom {
            custom.invalidate();
        }
        let mut cursor = Some(self.clone());
        while let Some(node) = cursor {
            let already_dirty = node.0.borrow().subtree_dirty;
            node.0.borrow_mut().subtree_dirty = true;
            node.0.borrow_mut().dirty = true;
            if already_dirty {
                return;
            }
            cursor = node.parent();
        }
    }
    fn clear_layout_dirty_hints(&self) {
        if !self.0.borrow().subtree_dirty {
            return;
        }
        self.0.borrow_mut().subtree_dirty = false;
        self.0.borrow_mut().dirty = false;
        for child in self.children() {
            child.clear_layout_dirty_hints();
        }
    }

    // MARK: - Flex pass

    fn resolved_style_insets(
        &self,
        basis: Option<f32>,
    ) -> (LayoutInsets, LayoutInsets, LayoutInsets) {
        let style = self.0.borrow().style;
        let padding = self
            .0
            .borrow()
            .debug_padding
            .unwrap_or_else(|| style.padding.resolve(basis));
        let border = style.border.insets();
        let margin = style.margin.resolve(basis);
        (padding, border, margin)
    }

    /// Intrinsic measurement. `avail_*` are at-most constraints from the parent.
    /// The direction this node lays its children out in.
    ///
    /// A node that sets its own `direction` overrides the inherited one; every
    /// other node passes the inherited direction through. Without this, a
    /// per-node `.layout_direction(Rtl)` was stored but never consulted, so RTL
    /// rows rendered left-to-right no matter what the style said.
    fn effective_direction(&self, inherited: Direction) -> Direction {
        match self.0.borrow().style.direction {
            Direction::Inherit => inherited,
            direction => direction,
        }
    }

    fn measure(
        &self,
        owner_width: Option<f32>,
        owner_height: Option<f32>,
        avail_width: Option<f32>,
        avail_height: Option<f32>,
        direction: Direction,
    ) -> Size {
        let direction = self.effective_direction(direction);
        let style = self.0.borrow().style;
        if style.display == Display::None {
            return Size::default();
        }
        let basis = owner_width.or(avail_width);
        let (padding, border, margin) = self.resolved_style_insets(basis);
        let inset_width = padding.horizontal() + border.horizontal();
        let inset_height = padding.vertical() + border.vertical();

        let mut definite_width = style.width.resolve(owner_width.or(avail_width));
        let mut definite_height = style.height.resolve(owner_height.or(avail_height));
        // Swift clamps only at the Yoga write (`writeWidthStyle`), keeping the
        // authored value intact so a cleared floor restores it. Do the same here:
        // raise a definite size to the effective minimum without rewriting style.
        let (floor_width, floor_height) = self.effective_minimum();
        if let (Some(width), Some(minimum)) = (definite_width.as_mut(), floor_width) {
            *width = width.max(minimum);
        }
        if let (Some(height), Some(minimum)) = (definite_height.as_mut(), floor_height) {
            *height = height.max(minimum);
        }
        if style.box_sizing == BoxSizing::ContentBox {
            definite_width = definite_width.map(|width| width + inset_width);
            definite_height = definite_height.map(|height| height + inset_height);
        }

        // Dispatch order matters and is NOT the same question as
        // `has_measure_func`: a custom layout installs a measure trampoline, so
        // introspection reports true, but its body must still run through the
        // custom closure rather than `measure_size` (which would find no plain
        // callback and return 0x0).
        let size = if let Some(custom) = self.0.borrow().custom.as_ref() {
            let children = self.children();
            let measured = (custom.measure.borrow())(
                avail_width.map(|width| width - inset_width),
                avail_height.map(|height| height - inset_height),
                &children,
            );
            Size {
                width: definite_width.unwrap_or(measured.width + inset_width),
                height: definite_height.unwrap_or(measured.height + inset_height),
            }
        } else if self.has_measure_func() {
            let (width_mode, width) = match definite_width {
                Some(width) => (MeasureMode::Exactly, width),
                None => match avail_width {
                    Some(width) => (MeasureMode::AtMost, width),
                    None => (MeasureMode::Undefined, 0.0),
                },
            };
            let (height_mode, height) = match definite_height {
                Some(height) => (MeasureMode::Exactly, height),
                None => match avail_height {
                    Some(height) => (MeasureMode::AtMost, height),
                    None => (MeasureMode::Undefined, 0.0),
                },
            };
            let measured = self.measure_size(width, width_mode, height, height_mode);
            Size {
                width: definite_width.unwrap_or(measured.width),
                height: definite_height.unwrap_or(measured.height),
            }
        } else {
            let row = style.flex_direction.is_row();
            let content_width = definite_width
                .map(|width| (width - inset_width).max(0.0))
                .or(avail_width.map(|width| (width - inset_width).max(0.0)));
            let content_height = definite_height
                .map(|height| (height - inset_height).max(0.0))
                .or(avail_height.map(|height| (height - inset_height).max(0.0)));
            let gap = if row {
                style.gap_column.resolve(content_width).unwrap_or(0.0)
            } else {
                style.gap_row.resolve(content_height).unwrap_or(0.0)
            };
            let mut main = 0.0f32;
            let mut cross: f32 = 0.0;
            let mut count = 0usize;
            for child in self.children() {
                let child_style = child.style();
                if child_style.display == Display::None
                    || child_style.position_type == PositionType::Absolute
                {
                    continue;
                }
                let (_, _, child_margin) = child.resolved_style_insets(content_width);
                let (child_width, child_height) = if row {
                    (None, content_height)
                } else {
                    (content_width, None)
                };
                let measured = child.measure(
                    content_width,
                    content_height,
                    child_width,
                    child_height,
                    direction,
                );
                if row {
                    main += measured.width + child_margin.horizontal();
                    cross = cross.max(measured.height + child_margin.vertical());
                } else {
                    main += measured.height + child_margin.vertical();
                    cross = cross.max(measured.width + child_margin.horizontal());
                }
                count += 1;
            }
            if count > 1 {
                main += gap * (count - 1) as f32;
            }
            // Stretched children fill the cross axis, so the container must claim
            // the cross extent it was offered rather than hugging the widest
            // child. Without this a column of auto-width rows collapses to zero
            // width even though `align_items` says they should stretch.
            // Only auto-sized children stretch, so the container claims the
            // offered cross extent only when at least one child left its cross
            // size to the layout. A column of explicitly sized rows still hugs
            // its widest child.
            let any_auto_cross = self.children().iter().any(|child| {
                let child_style = child.style();
                child_style.display != Display::None
                    && child_style.position_type != PositionType::Absolute
                    && (if row {
                        child_style.height.is_auto()
                    } else {
                        child_style.width.is_auto()
                    })
            });
            if style.align_items == Align::Stretch && any_auto_cross {
                if row {
                    if let Some(available) = content_height {
                        cross = cross.max(available);
                    }
                } else if let Some(available) = content_width {
                    cross = cross.max(available);
                }
            }
            Size {
                width: definite_width.unwrap_or(if row {
                    main + inset_width
                } else {
                    cross + inset_width
                }),
                height: definite_height.unwrap_or(if row {
                    cross + inset_height
                } else {
                    main + inset_height
                }),
            }
        };

        let _ = margin;
        // Clamp against the *effective* minimum so a transient control-extent
        // floor raises the authored value; using `style.min_width` alone would
        // ignore the floor and leave a box below the platform touch target.
        let (min_width, min_height) = self.effective_minimum();
        Size {
            width: clamp(size.width, min_width, style.max_width),
            height: clamp(size.height, min_height, style.max_height),
        }
    }

    /// Assigns computed frames for this node and its descendants.
    fn arrange(&self, left: f32, top: f32, width: f32, height: f32, direction: Direction) {
        let direction = self.effective_direction(direction);
        let style = self.0.borrow().style;
        if style.display == Display::None {
            self.0.borrow_mut().computed = ComputedLayout {
                left,
                top,
                width: 0.0,
                height: 0.0,
                ..ComputedLayout::default()
            };
            return;
        }
        // A transient floor must survive the parent's placement decision, the
        // same way Swift's `writeWidthStyle` raises the value it hands to Yoga.
        // Clamping here (not in style) keeps the authored size reversible.
        let (floor_width, floor_height) = self.effective_minimum();
        let width = floor_width.map_or(width, |minimum| width.max(minimum));
        let height = floor_height.map_or(height, |minimum| height.max(minimum));
        let (padding, border, margin) = self.resolved_style_insets(Some(width));
        {
            let mut data = self.0.borrow_mut();
            data.computed = ComputedLayout {
                left,
                top,
                width,
                height,
                padding,
                margin,
                border,
            };
        }
        let content = Rect::new(
            left + padding.left + border.left,
            top + padding.top + border.top,
            (width - padding.horizontal() - border.horizontal()).max(0.0),
            (height - padding.vertical() - border.vertical()).max(0.0),
        );
        if let Some(custom) = self.0.borrow().custom.as_ref() {
            let children = self.children();
            let mut placement = custom.placement_bounds.borrow_mut();
            if placement
                .as_ref()
                .is_none_or(|(bounds, previous)| *bounds != content || *previous != direction)
            {
                *placement = Some((content, direction));
                (custom.place.borrow())(content, &children);
            }
            return;
        }
        self.arrange_flex(content, direction);
    }

    fn arrange_flex(&self, content: Rect, direction: Direction) {
        let style = self.0.borrow().style;
        let children = self.children();
        if children.is_empty() {
            return;
        }
        let row = style.flex_direction.is_row();
        let reverse = style.flex_direction.is_reverse();
        let main_extent = if row { content.width } else { content.height };
        let cross_extent = if row { content.height } else { content.width };
        let gap = if row {
            style.gap_column.resolve(Some(content.width)).unwrap_or(0.0)
        } else {
            style.gap_row.resolve(Some(content.height)).unwrap_or(0.0)
        };

        let mut flow: Vec<LayoutNode> = Vec::new();
        for child in children {
            let child_style = child.style();
            if child_style.display == Display::None {
                child.arrange(0.0, 0.0, 0.0, 0.0, direction);
                continue;
            }
            if child_style.position_type == PositionType::Absolute {
                continue;
            }
            flow.push(child);
        }

        // Base sizes along the main axis.
        let mut base = Vec::with_capacity(flow.len());
        let mut cross_sizes = Vec::with_capacity(flow.len());
        let mut margins = Vec::with_capacity(flow.len());
        for child in &flow {
            let child_style = child.style();
            let (_, _, child_margin) = child.resolved_style_insets(Some(content.width));
            let basis_value = if child_style.flex_basis.is_auto() {
                if row {
                    child_style.width
                } else {
                    child_style.height
                }
            } else {
                child_style.flex_basis
            };
            let resolved = basis_value.resolve(Some(main_extent));
            // `main_extent`/`cross_extent` are axis-relative, but `measure` takes
            // physical width and height. In a column the main axis is vertical,
            // so passing `main_extent` as the available *width* transposed the
            // two and shaped every child text against the container's height.
            let (available_width, available_height) = if row {
                (main_extent, cross_extent)
            } else {
                (cross_extent, main_extent)
            };
            let measured = child.measure(
                Some(content.width),
                Some(content.height),
                Some(available_width),
                Some(available_height),
                direction,
            );
            let main = resolved.unwrap_or(if row { measured.width } else { measured.height });
            base.push(main);
            cross_sizes.push(if row { measured.height } else { measured.width });
            margins.push(child_margin);
        }

        // Line breaking.
        let mut lines: Vec<Vec<usize>> = Vec::new();
        let mut current: Vec<usize> = Vec::new();
        let mut used = 0.0f32;
        for (index, size) in base.iter().enumerate() {
            let outer = size
                + if row {
                    margins[index].horizontal()
                } else {
                    margins[index].vertical()
                };
            let with_gap = if current.is_empty() {
                outer
            } else {
                outer + gap
            };
            if style.flex_wrap != Wrap::NoWrap
                && !current.is_empty()
                && used + with_gap > main_extent
            {
                lines.push(std::mem::take(&mut current));
                used = outer;
            } else {
                used += with_gap;
            }
            current.push(index);
        }
        if !current.is_empty() {
            lines.push(current);
        }

        // Grow / shrink each line, then place.
        let mut cross_cursor = 0.0f32;
        for (line_index, line) in lines.iter().enumerate() {
            let line_gap = if line.len() > 1 {
                gap * (line.len() - 1) as f32
            } else {
                0.0
            };
            let occupied: f32 = line
                .iter()
                .map(|&index| {
                    base[index]
                        + if row {
                            margins[index].horizontal()
                        } else {
                            margins[index].vertical()
                        }
                })
                .sum::<f32>()
                + line_gap;
            let free = main_extent - occupied;
            let total_grow: f32 = line.iter().map(|&index| flow[index].flex_grow()).sum();
            let total_shrink: f32 = line
                .iter()
                .map(|&index| flow[index].flex_shrink() * base[index])
                .sum();
            let mut sizes = Vec::with_capacity(line.len());
            for &index in line {
                let child = &flow[index];
                let mut size = base[index];
                if free > 0.0 && total_grow > 0.0 {
                    size += free * (child.flex_grow() / total_grow);
                } else if free < 0.0 && total_shrink > 0.0 {
                    size += free * ((child.flex_shrink() * base[index]) / total_shrink);
                }
                let child_style = child.style();
                size = clamp(
                    size,
                    if row {
                        child_style.min_width
                    } else {
                        child_style.min_height
                    },
                    if row {
                        child_style.max_width
                    } else {
                        child_style.max_height
                    },
                );
                sizes.push(size.max(0.0));
            }
            let line_main: f32 = sizes
                .iter()
                .enumerate()
                .map(|(offset, size)| {
                    size + if row {
                        margins[line[offset]].horizontal()
                    } else {
                        margins[line[offset]].vertical()
                    }
                })
                .sum::<f32>()
                + line_gap;
            let line_cross = line
                .iter()
                .enumerate()
                .map(|(offset, &index)| {
                    let child = &flow[index];
                    let measured = if row {
                        cross_sizes[index]
                    } else {
                        cross_sizes[index]
                    };
                    let _ = child;
                    measured
                        + if row {
                            margins[index].vertical()
                        } else {
                            margins[index].horizontal()
                        }
                        + sizes[offset] * 0.0
                })
                .fold(0.0f32, f32::max);

            let (mut cursor, spacing) =
                justify_offsets(style.justify_content, main_extent, line_main, line.len());
            if reverse {
                cursor = main_extent - line_main - cursor;
            }
            // Yoga resolves a non-stretch child's cross extent against the
            // container, not against the tallest child on the line. Only the
            // wrapped multi-line case subtracts the consumed cross cursor.
            let line_cross_extent =
                if style.align_items == Align::Stretch || style.flex_wrap == Wrap::NoWrap {
                    cross_extent - cross_cursor
                } else {
                    line_cross
                };
            let mut cross_offset = cross_cursor;
            if style.flex_wrap != Wrap::NoWrap {
                cross_offset = match style.align_content {
                    Align::Center => cross_cursor + (cross_extent - line_cross) / 2.0,
                    Align::FlexEnd => cross_cursor + (cross_extent - line_cross),
                    _ => cross_cursor,
                };
            }
            for (offset, &index) in line.iter().enumerate() {
                let child = &flow[index];
                let child_style = child.style();
                let child_main = sizes[offset];
                let child_margin = margins[index];
                let align = if child_style.align_self == Align::Auto {
                    style.align_items
                } else {
                    child_style.align_self
                };
                let stretch = align == Align::Stretch
                    && (if row {
                        child_style.height.is_auto()
                    } else {
                        child_style.width.is_auto()
                    });
                let measured_cross = cross_sizes[index];
                let available_cross = (line_cross_extent
                    - if row {
                        child_margin.vertical()
                    } else {
                        child_margin.horizontal()
                    })
                .max(0.0);
                let child_cross = if stretch {
                    available_cross
                } else {
                    measured_cross
                };
                let cross_shift = if stretch {
                    0.0
                } else {
                    match align {
                        Align::Center => (available_cross - child_cross) / 2.0,
                        Align::FlexEnd => available_cross - child_cross,
                        _ => 0.0,
                    }
                };
                let (child_left, child_top) = if row {
                    // Yoga mirrors the row main axis under RTL, so the first
                    // child sits at the trailing (right) edge. `reverse` has
                    // already offset the cursor, and mirroring on top of it
                    // reproduces Yoga's RTL + `row-reverse` = LTR result.
                    let left = if direction == Direction::Rtl {
                        main_extent - cursor - child_main
                    } else {
                        cursor
                    };
                    (
                        left + child_margin.left,
                        cross_offset + cross_shift + child_margin.top,
                    )
                } else {
                    (
                        cross_offset + cross_shift + child_margin.left,
                        cursor + child_margin.top,
                    )
                };
                let (child_width, child_height) = if row {
                    (child_main, child_cross)
                } else {
                    (child_cross, child_main)
                };
                child.arrange(
                    content.x + child_left,
                    content.y + child_top,
                    child_width,
                    child_height,
                    direction,
                );
                // `line_main` already counted the gap between items, so the
                // cursor must advance by it too — otherwise a gap shrinks the
                // content without moving anything, and the trailing items
                // overlap the leading edge.
                cursor += child_main
                    + if row {
                        child_margin.horizontal()
                    } else {
                        child_margin.vertical()
                    }
                    + if offset + 1 < line.len() { gap } else { 0.0 }
                    + spacing;
            }
            cross_cursor += line_cross
                + if line_index + 1 < lines.len() {
                    gap
                } else {
                    0.0
                };
        }

        // Absolutely positioned children resolve against the padding box.
        for child in self.children() {
            let child_style = child.style();
            if child_style.display == Display::None
                || child_style.position_type != PositionType::Absolute
            {
                continue;
            }
            let measured = child.measure(
                Some(content.width),
                Some(content.height),
                Some(content.width),
                Some(content.height),
                direction,
            );
            // An absolutely-positioned child with auto width/height but both
            // opposing insets resolved stretches between them: Yoga gives
            // `parent - left - right` (and likewise vertically). Using the
            // measured size instead collapsed every pinned-edge overlay — a
            // portal layer pinning all four edges to 0 measured 0x0.
            let right_for_width = child_style.position.right.resolve(Some(content.width));
            let left_for_width = child_style.position.left.resolve(Some(content.width));
            let bottom_for_height = child_style.position.bottom.resolve(Some(content.height));
            let top_for_height = child_style.position.top.resolve(Some(content.height));
            let width = child_style.width.resolve(Some(content.width)).unwrap_or_else(|| {
                match (left_for_width, right_for_width) {
                    (Some(left), Some(right)) => (content.width - left - right).max(0.0),
                    _ => measured.width,
                }
            });
            let height = child_style.height.resolve(Some(content.height)).unwrap_or_else(|| {
                match (top_for_height, bottom_for_height) {
                    (Some(top), Some(bottom)) => (content.height - top - bottom).max(0.0),
                    _ => measured.height,
                }
            });
            // Resolve logical start/end with the direction actually in force at
            // layout time. The eager physical value written by `set` is only a
            // fallback for nodes created before their direction was known.
            let rtl = direction == Direction::Rtl;
            let (logical_left, logical_right) = if rtl {
                (child_style.position.end, child_style.position.start)
            } else {
                (child_style.position.start, child_style.position.end)
            };
            let left_inset = child_style
                .position
                .left
                .or_logical(logical_left)
                .resolve(Some(content.width));
            let right_inset = child_style
                .position
                .right
                .or_logical(logical_right)
                .resolve(Some(content.width));
            let top_inset = child_style.position.top.resolve(Some(content.height));
            let bottom_inset = child_style.position.bottom.resolve(Some(content.height));
            let left = match (left_inset, right_inset) {
                (Some(left), _) => left,
                (None, Some(right)) => content.width - width - right,
                _ => 0.0,
            };
            let top = match (top_inset, bottom_inset) {
                (Some(top), _) => top,
                (None, Some(bottom)) => content.height - height - bottom,
                _ => 0.0,
            };
            child.arrange(content.x + left, content.y + top, width, height, direction);
        }
    }

    // MARK: - Custom layouts

    pub fn set_custom_layout(&self, measure: CustomMeasure, place: CustomPlace) {
        let state = CustomLayoutState::new(measure, place);
        self.0.borrow_mut().custom = Some(state);
        self.0.borrow_mut().measure = None;
        self.mark_layout_dirty_hint();
    }
    pub fn custom_layout_direction(&self) -> Direction {
        if self.direction() != Direction::Inherit {
            return self.direction();
        }
        self.parent()
            .map_or(Direction::Ltr, |parent| parent.custom_layout_direction())
    }
    pub fn measure_for_custom_layout(&self, width: Option<f32>, height: Option<f32>) -> Size {
        let direction = self.custom_layout_direction();
        let owner_width = width.or_else(|| self.parent().map(|parent| parent.computed().width));
        let owner_height = height.or_else(|| self.parent().map(|parent| parent.computed().height));
        let size = self.measure(owner_width, owner_height, width, height, direction);
        self.arrange(0.0, 0.0, size.width, size.height, direction);
        size
    }
    pub fn custom_layout_is_visible(&self) -> bool {
        self.0.borrow().placement_is_visible
    }
    pub fn place_for_custom_layout(
        &self,
        at: Point,
        width: Option<f32>,
        height: Option<f32>,
        is_visible: bool,
    ) {
        self.0.borrow_mut().placement_is_visible = is_visible;
        if !is_visible {
            self.0.borrow_mut().placed_frame = Some(Rect::default());
            return;
        }
        let size = self.measure_for_custom_layout(width, height);
        self.arrange(
            at.x,
            at.y,
            size.width,
            size.height,
            self.custom_layout_direction(),
        );
        self.place_custom_layouts();
        let computed = self.0.borrow().computed;
        self.0.borrow_mut().placed_frame =
            Some(Rect::new(at.x, at.y, computed.width, computed.height));
    }
    fn prepare_custom_layouts(&self) {
        if !self.subtree_is_dirty() {
            return;
        }
        if let Some(custom) = &self.0.borrow().custom {
            custom.invalidate();
        }
        for child in self.children() {
            child.prepare_custom_layouts();
        }
    }
    fn place_custom_layouts(&self) {
        if let Some(custom) = self.0.borrow().custom.as_ref() {
            let computed = self.0.borrow().computed;
            let children = self.children();
            let left = computed.padding.left + computed.border.left;
            let top = computed.padding.top + computed.border.top;
            let bounds = Rect::new(
                left,
                top,
                (computed.width - left - computed.padding.right - computed.border.right).max(0.0),
                (computed.height - top - computed.padding.bottom - computed.border.bottom).max(0.0),
            );
            (custom.place.borrow())(bounds, &children);
        } else {
            for child in self.children() {
                child.place_custom_layouts();
            }
        }
    }
}

fn clamp(value: f32, minimum: Option<f32>, maximum: Option<f32>) -> f32 {
    let lower = minimum.unwrap_or(f32::NEG_INFINITY);
    let upper = maximum.unwrap_or(f32::INFINITY);
    value.max(lower).min(upper).max(0.0)
}

/// Leading offset and inter-item spacing for the main axis.
fn justify_offsets(justify: Justify, available: f32, occupied: f32, count: usize) -> (f32, f32) {
    let free = (available - occupied).max(0.0);
    let gaps = count.saturating_sub(1) as f32;
    match justify {
        Justify::FlexStart => (0.0, 0.0),
        Justify::Center => (free / 2.0, 0.0),
        Justify::FlexEnd => (free, 0.0),
        Justify::SpaceBetween => (0.0, if gaps > 0.0 { free / gaps } else { 0.0 }),
        Justify::SpaceAround => {
            let spacing = if count > 0 { free / count as f32 } else { 0.0 };
            (spacing / 2.0, spacing)
        }
        Justify::SpaceEvenly => {
            let spacing = if count > 0 {
                free / (count + 1) as f32
            } else {
                0.0
            };
            (spacing, spacing)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_stack_sums_heights_and_stretches_cross_axis() {
        let root = LayoutNode::new();
        root.set_flex_direction(FlexDirection::Column);
        root.set_align_items(Align::Stretch);
        let first = LayoutNode::new();
        first.set_height(Some(40.0));
        let second = LayoutNode::new();
        second.set_height(Some(60.0));
        root.add_child(&first);
        root.add_child(&second);
        root.calculate_layout(Some(200.0), None, Direction::Ltr);
        assert_eq!(first.frame(), Rect::new(0.0, 0.0, 200.0, 40.0));
        assert_eq!(second.frame(), Rect::new(0.0, 40.0, 200.0, 60.0));
        assert_eq!(root.frame().height, 100.0);
    }

    #[test]
    fn grow_shares_free_space() {
        let root = LayoutNode::new();
        root.set_flex_direction(FlexDirection::Row);
        root.set_width(Some(300.0));
        root.set_height(Some(50.0));
        let fixed = LayoutNode::new();
        fixed.set_width(Some(100.0));
        fixed.set_height(Some(50.0));
        let grow = LayoutNode::new();
        grow.set_flex_grow(1.0);
        grow.set_height(Some(50.0));
        root.add_child(&fixed);
        root.add_child(&grow);
        root.calculate_layout(Some(300.0), Some(50.0), Direction::Ltr);
        assert_eq!(fixed.frame().width, 100.0);
        assert_eq!(grow.frame().width, 200.0);
        assert_eq!(grow.frame().x, 100.0);
    }

    #[test]
    fn padding_and_gap_offset_children() {
        let root = LayoutNode::new();
        root.set_flex_direction(FlexDirection::Row);
        root.set_padding(10.0, Edge::All);
        root.set_gap(5.0, Gutter::All);
        let first = LayoutNode::new();
        first.set_width(Some(20.0));
        first.set_height(Some(20.0));
        let second = LayoutNode::new();
        second.set_width(Some(20.0));
        second.set_height(Some(20.0));
        root.add_child(&first);
        root.add_child(&second);
        root.calculate_layout(Some(100.0), Some(100.0), Direction::Ltr);
        assert_eq!(first.frame().x, 10.0);
        assert_eq!(second.frame().x, 35.0);
        assert_eq!(root.resolved_padding().left, 10.0);
    }

    #[test]
    fn measure_callback_sizes_a_leaf() {
        let root = LayoutNode::new();
        root.set_flex_direction(FlexDirection::Column);
        let leaf = LayoutNode::new();
        let measure: MeasureFunc = Rc::new(|width, width_mode, _height, _height_mode| {
            let natural: f32 = 80.0;
            Size {
                width: if width_mode == MeasureMode::AtMost {
                    natural.min(width)
                } else {
                    natural
                },
                height: 18.0,
            }
        });
        leaf.set_measure_func(Some(measure));
        root.add_child(&leaf);
        root.calculate_layout(Some(50.0), None, Direction::Ltr);
        assert_eq!(leaf.frame().height, 18.0);
        assert_eq!(leaf.frame().width, 50.0);
    }
}
