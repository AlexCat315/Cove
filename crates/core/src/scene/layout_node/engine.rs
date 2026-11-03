//! Taffy over the `LayoutNode` tree.
//!
//! The tree is addressed in place: a `NodeId` is the address of a node's
//! `LayoutCell`, so no parallel arena is built or synchronised. A custom
//! layout's probe (a flex-start column around one child) has the child's
//! address with the low bit set; it owns no storage beyond the child's probe
//! cache.
//!
//! Each node keeps its Taffy style, cache and unrounded layout in
//! [`EngineState`]. The style is rebuilt lazily from the authored
//! `LayoutStyle` when that changed or when the inherited direction differs,
//! always before any frame of the flex algorithm can read it: a container
//! refreshes its children's styles before it lays them out.
use super::value::{CalcStorage, resolve_calc};
use super::{LayoutCell, LayoutNode, MeasureMode};
use crate::Size;
use crate::scene::Direction;
use std::cell::{Cell, RefCell, UnsafeCell};
use std::rc::Rc;
use taffy::{
    AvailableSpace, Cache, CacheTree, Layout, LayoutContainingBlock, LayoutFlexboxContainer,
    LayoutGridContainer, LayoutInput, LayoutOutput, LayoutPartialTree, Line, MaybeResolve, NodeId,
    RequestedAxis, ResolveOrZero, RunMode, SizingMode, Style, TraversePartialTree,
    compute_cached_layout, compute_flexbox_layout, compute_grid_layout, compute_hidden_layout,
    compute_leaf_layout, compute_oof_layout,
};

const PROBE_BIT: usize = 1;
const PLACE_BIT: usize = 2;
const _: () = assert!(align_of::<LayoutCell>() > PLACE_BIT);

/// Swift's measurement cache depth per child of a custom layout.
const MEASUREMENTS: usize = 4;

thread_local! {
    /// The probe container: a flex-start column with Yoga's zero minimums.
    static PROBE_STYLE: &'static Style = Box::leak(Box::new(probe_style(taffy::AlignItems::FLEX_START)));
    /// The placement probe: the same column, but stretching across the slot.
    ///
    /// A custom layout assigns each child an exact slot. Measurement has to stay
    /// flex-start so an intrinsic probe reports the child's own size, but
    /// placement must hand the child that width: a flex container child only
    /// shrinks its own children when it knows its extent, and an auto width
    /// resolved from min-content (a nowrap label) is as wide as the label.
    static PLACE_STYLE: &'static Style = Box::leak(Box::new(probe_style(taffy::AlignItems::STRETCH)));
}

fn probe_style(align_items: taffy::AlignItems) -> Style {
    Style {
        flex_direction: taffy::FlexDirection::Column,
        align_items,
        align_content: taffy::AlignContent::FLEX_START,
        position: taffy::Position::Relative,
        min_size: taffy::Size {
            width: taffy::LengthPercentageAuto::length(0.0),
            height: taffy::LengthPercentageAuto::length(0.0),
        },
        ..Style::DEFAULT
    }
}

/// Which synthetic container stands in for a custom layout's child.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Probe {
    /// Intrinsic measurement: the child keeps its own size.
    Measure,
    /// Placement: the child takes the slot the layout assigned it.
    Place,
}

#[derive(Clone, Copy, PartialEq)]
struct Proposal {
    width: Option<f32>,
    height: Option<f32>,
    direction: Direction,
}

pub(super) struct EngineState {
    /// Written only by `refresh_style`, never while a reference is live.
    style: UnsafeCell<Style>,
    calc: RefCell<CalcStorage>,
    style_stale: Cell<bool>,
    /// The resolved direction the style was built for.
    built_direction: Cell<Direction>,
    cache: RefCell<Cache>,
    layout: Cell<Layout>,
    probe_cache: RefCell<Cache>,
    place_cache: RefCell<Cache>,
    measurements: RefCell<Vec<(Proposal, Size)>>,
}

impl Default for EngineState {
    fn default() -> Self {
        Self {
            style: UnsafeCell::new(Style::DEFAULT),
            calc: RefCell::new(CalcStorage::default()),
            style_stale: Cell::new(true),
            built_direction: Cell::new(Direction::Ltr),
            cache: RefCell::new(Cache::new()),
            layout: Cell::new(Layout::new()),
            probe_cache: RefCell::new(Cache::new()),
            place_cache: RefCell::new(Cache::new()),
            measurements: RefCell::new(Vec::new()),
        }
    }
}

impl EngineState {
    pub(super) fn layout(&self) -> Layout {
        self.layout.get()
    }
    pub(super) fn invalidate_style(&self) {
        self.style_stale.set(true);
    }
    pub(super) fn is_clear(&self) -> bool {
        self.cache.borrow().is_empty()
            && self.probe_cache.borrow().is_empty()
            && self.place_cache.borrow().is_empty()
            && self.measurements.borrow().is_empty()
    }
    pub(super) fn clear_results(&self) {
        self.cache.borrow_mut().clear();
        self.probe_cache.borrow_mut().clear();
        self.place_cache.borrow_mut().clear();
        self.forget_measurements();
    }
    pub(super) fn forget_measurements(&self) {
        self.measurements.borrow_mut().clear();
    }
    pub(super) fn measurement(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        direction: Direction,
    ) -> Option<Size> {
        let key = Proposal {
            width,
            height,
            direction,
        };
        self.measurements
            .borrow()
            .iter()
            .find(|(proposal, _)| *proposal == key)
            .map(|(_, size)| *size)
    }
    pub(super) fn store_measurement(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        direction: Direction,
        size: Size,
    ) {
        let mut entries = self.measurements.borrow_mut();
        if entries.len() == MEASUREMENTS {
            entries.remove(0);
        }
        entries.push((
            Proposal {
                width,
                height,
                direction,
            },
            size,
        ));
    }
}

fn node_id(node: &LayoutNode) -> NodeId {
    NodeId::from(Rc::as_ptr(&node.0) as usize)
}
fn probe_id(node: &LayoutNode, probe: Probe) -> NodeId {
    let bit = match probe {
        Probe::Measure => PROBE_BIT,
        Probe::Place => PLACE_BIT,
    };
    NodeId::from(Rc::as_ptr(&node.0) as usize | bit)
}
fn probe_kind(id: NodeId) -> Option<Probe> {
    let bits = usize::from(id) & (PROBE_BIT | PLACE_BIT);
    match bits {
        0 => None,
        PROBE_BIT => Some(Probe::Measure),
        PLACE_BIT => Some(Probe::Place),
        _ => unreachable!("a node address is only ever tagged with one probe bit"),
    }
}
fn is_probe(id: NodeId) -> bool {
    probe_kind(id).is_some()
}
/// # Safety
/// `id` must name a node of the tree being laid out; the root's strong
/// references keep every such cell alive for the whole pass.
unsafe fn cell<'a>(id: NodeId) -> &'a LayoutCell {
    // SAFETY: the caller guarantees the tagged address belongs to a live tree node.
    unsafe { &*((usize::from(id) & !(PROBE_BIT | PLACE_BIT)) as *const LayoutCell) }
}

/// Rebuild `cell`'s Taffy style for `inherited` if it is stale.
fn refresh_style(cell: &LayoutCell, inherited: Direction) -> Direction {
    let data = cell.data.borrow();
    let direction = match data.style.direction {
        Direction::Inherit => inherited,
        direction => direction,
    };
    let engine = &cell.engine;
    let redirected = engine.built_direction.get() != direction;
    if engine.style_stale.get() || redirected {
        let calc = CalcStorage::default();
        let style = data.style.to_taffy(
            direction,
            data.debug_padding,
            data.minimum_size_floor,
            &calc,
        );
        drop(data);
        // SAFETY: styles are only borrowed by the layout algorithm of the
        // node's parent, which refreshes them before taking any reference.
        unsafe { *engine.style.get() = style };
        *engine.calc.borrow_mut() = calc;
        engine.style_stale.set(false);
        engine.built_direction.set(direction);
        if redirected {
            engine.cache.borrow_mut().clear();
            engine.probe_cache.borrow_mut().clear();
        }
    }
    direction
}

fn refresh_children(cell: &LayoutCell, direction: Direction) {
    let children = cell.data.borrow().children.clone();
    for child in &children {
        refresh_style(&child.0, direction);
    }
}

/// The resolved direction a node passes to its children.
pub(super) fn resolved_direction(cell: &LayoutCell) -> Direction {
    cell.engine.built_direction.get()
}

pub(super) struct LayoutTree;

pub(super) struct ChildIter<'a> {
    cell: &'a LayoutCell,
    probe: bool,
    index: usize,
    count: usize,
}
impl Iterator for ChildIter<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        if self.index >= self.count {
            return None;
        }
        let id = if self.probe {
            NodeId::from(self.cell as *const LayoutCell as usize)
        } else {
            NodeId::from(Rc::as_ptr(&self.cell.data.borrow().children[self.index].0) as usize)
        };
        self.index += 1;
        Some(id)
    }
}

impl TraversePartialTree for LayoutTree {
    type ChildIter<'a> = ChildIter<'a>;

    fn child_ids(&self, parent: NodeId) -> ChildIter<'_> {
        // SAFETY: see `cell`.
        let cell = unsafe { cell(parent) };
        ChildIter {
            cell,
            probe: is_probe(parent),
            index: 0,
            count: self.child_count(parent),
        }
    }
    fn child_count(&self, parent: NodeId) -> usize {
        if is_probe(parent) {
            return 1;
        }
        // SAFETY: see `cell`.
        let cell = unsafe { cell(parent) };
        let data = cell.data.borrow();
        // A custom layout places its own children; flexbox sees a leaf.
        if data.custom.is_some() {
            0
        } else {
            data.children.len()
        }
    }
    fn get_child_id(&self, parent: NodeId, index: usize) -> NodeId {
        // SAFETY: see `cell`.
        let cell = unsafe { cell(parent) };
        if is_probe(parent) {
            return NodeId::from(cell as *const LayoutCell as usize);
        }
        NodeId::from(Rc::as_ptr(&cell.data.borrow().children[index].0) as usize)
    }
}

impl LayoutPartialTree for LayoutTree {
    type CoreContainerStyle<'a> = &'a Style;
    type CustomIdent = String;

    fn resolve_calc_value(&self, ptr: *const (), basis: f32) -> f32 {
        resolve_calc(ptr, basis)
    }

    fn get_core_container_style(&self, id: NodeId) -> &Style {
        style_of(id)
    }
    fn set_unrounded_layout(&mut self, id: NodeId, layout: &Layout) {
        if !is_probe(id) {
            // SAFETY: see `cell`.
            unsafe { cell(id) }.engine.layout.set(*layout);
        }
    }
    fn compute_child_layout(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        if inputs.run_mode == RunMode::PerformHiddenLayout {
            self.set_unrounded_layout(id, &Layout::with_order(0));
            return compute_hidden_layout(self, id);
        }
        compute_cached_layout(self, id, inputs, |tree, id, inputs| {
            // SAFETY: see `cell`.
            let node = unsafe { cell(id) };
            let mut output = if is_probe(id) {
                compute_flexbox_layout(tree, id, inputs)
            } else {
                compute_node(tree, node, id, inputs)
            };
            if inputs.run_mode == RunMode::PerformLayout {
                compute_oof_layout(tree, id, &mut output);
            }
            output
        })
    }
}

fn style_of<'a>(id: NodeId) -> &'a Style {
    match probe_kind(id) {
        Some(Probe::Measure) => PROBE_STYLE.with(|style| *style),
        Some(Probe::Place) => PLACE_STYLE.with(|style| *style),
        None => {
            // SAFETY: see `cell` and `refresh_style`.
            unsafe { &*cell(id).engine.style.get() }
        }
    }
}

fn compute_node(
    tree: &mut LayoutTree,
    node: &LayoutCell,
    id: NodeId,
    mut inputs: LayoutInput,
) -> LayoutOutput {
    let (custom, has_children, measure) = {
        let data = node.data.borrow();
        if data.style.display == crate::scene::Display::None {
            drop(data);
            return compute_hidden_layout(tree, id);
        }
        (
            data.custom
                .as_ref()
                .map(|state| state.measure.borrow().clone()),
            !data.children.is_empty(),
            data.measure.clone(),
        )
    };
    // Taffy's leaf primitive does not resolve authored sizing keywords itself.
    // Resolve the width before measuring its dependent height; otherwise a
    // column's provisional stretched width can cache a single-line height.
    if (custom.is_some() || !has_children)
        && ((inputs.sizing_mode == SizingMode::InherentSize
            && inputs.known_dimensions.width.is_none())
            || (inputs.run_mode == RunMode::ComputeSize
                && inputs.axis == RequestedAxis::Vertical
                && inputs.known_dimensions.height.is_none()))
        && let Some(width) = intrinsic_leaf_width(tree, node, id, inputs)
    {
        inputs.known_dimensions.width = Some(width);
    }
    if let Some(measure) = custom {
        let children = node.data.borrow().children.clone();
        return compute_leaf_layout(inputs, style_of(id), resolve_calc, |_, available| {
            let size = measure(
                proposal_axis(available.width),
                proposal_axis(available.height),
                &children,
            );
            taffy::Size {
                width: size.width,
                height: size.height,
            }
        });
    }
    if has_children || style_of(id).display == taffy::Display::Grid {
        refresh_children(node, resolved_direction(node));
        return if style_of(id).display == taffy::Display::Grid {
            compute_grid_layout(tree, id, inputs)
        } else {
            compute_flexbox_layout(tree, id, inputs)
        };
    }
    compute_leaf_layout(inputs, style_of(id), resolve_calc, |known, available| {
        let Some(measure) = &measure else {
            return taffy::Size::ZERO;
        };
        let (width, width_mode) = axis(known.width, available.width);
        let (height, height_mode) = axis(known.height, available.height);
        let size = measure(width, width_mode, height, height_mode);
        taffy::Size {
            width: size.width,
            height: size.height,
        }
    })
}

fn intrinsic_leaf_width(
    tree: &mut LayoutTree,
    node: &LayoutCell,
    id: NodeId,
    inputs: LayoutInput,
) -> Option<f32> {
    use super::LayoutStyleValue as Value;
    let value = node.data.borrow().style.width;
    let mut probe = |space| {
        let mut probe = inputs;
        probe.sizing_mode = SizingMode::ContentSize;
        probe.run_mode = RunMode::ComputeSize;
        probe.axis = RequestedAxis::Horizontal;
        probe.known_dimensions.width = None;
        probe.known_dimensions.height = None;
        probe.available_space.width = space;
        compute_node(tree, node, id, probe).size.width
    };
    let result = match value {
        Value::MinContent => Some(probe(AvailableSpace::MinContent)),
        Value::MaxContent => Some(probe(AvailableSpace::MaxContent)),
        Value::FitContent | Value::FitContentLimit(_) => {
            let limit = match value {
                Value::FitContentLimit(calc) => {
                    let basis = inputs
                        .parent_size
                        .width
                        .or((calc.percent == 0.0).then_some(0.0))?;
                    calc.resolve(basis)
                }
                _ => inputs
                    .known_dimensions
                    .width
                    .or(inputs.available_space.width.into_option())?,
            };
            let minimum = probe(AvailableSpace::MinContent);
            let maximum = probe(AvailableSpace::MaxContent);
            Some(limit.min(maximum).max(minimum))
        }
        _ => None,
    };
    result.map(|width| {
        // Width-dependent height must use the constrained intrinsic width,
        // including during the parent's content-size probes.
        let style = style_of(id);
        let adjustment = if style.box_sizing == taffy::BoxSizing::ContentBox {
            let padding = style
                .padding
                .resolve_or_zero(inputs.parent_size.width, resolve_calc);
            let border = style
                .border
                .resolve_or_zero(inputs.parent_size.width, resolve_calc);
            padding.left + padding.right + border.left + border.right
        } else {
            0.0
        };
        let minimum = style
            .min_size
            .width
            .maybe_resolve(inputs.parent_size.width, resolve_calc)
            .map(|value| value + adjustment)
            .unwrap_or(0.0);
        let maximum = style
            .max_size
            .width
            .maybe_resolve(inputs.parent_size.width, resolve_calc)
            .map(|value| value + adjustment)
            .unwrap_or(f32::INFINITY);
        width.min(maximum).max(minimum)
    })
}

fn proposal_axis(value: AvailableSpace) -> Option<f32> {
    match value {
        AvailableSpace::MinContent => Some(0.0),
        value => value.into_option(),
    }
}

/// Yoga's measure contract: a known extent is exact, a definite available
/// extent is an upper bound, and intrinsic probes are unconstrained.
fn axis(known: Option<f32>, available: AvailableSpace) -> (f32, MeasureMode) {
    match available {
        AvailableSpace::Definite(value) if known.is_some() => (value, MeasureMode::Exactly),
        AvailableSpace::Definite(value) => (value, MeasureMode::AtMost),
        AvailableSpace::MinContent => (0.0, MeasureMode::MinContent),
        AvailableSpace::MaxContent => (0.0, MeasureMode::Undefined),
    }
}

impl LayoutContainingBlock for LayoutTree {
    type OofItemStyle<'a> = &'a Style;

    fn get_oof_item_style(&self, id: NodeId) -> &Style {
        style_of(id)
    }
    // Hoisted children only feed rounding, which this tree does not use.
    fn clear_hoisted_children(&mut self, _id: NodeId) {}
    fn add_hoisted_children(&mut self, _id: NodeId, _hoisted: &[NodeId]) {}
}

impl CacheTree for LayoutTree {
    fn cache_get(&mut self, id: NodeId, input: &LayoutInput) -> Option<LayoutOutput> {
        cache_of(id).borrow_mut().get(input)
    }
    fn cache_store(&mut self, id: NodeId, input: &LayoutInput, output: LayoutOutput) {
        cache_of(id).borrow_mut().store(input, output);
    }
    fn cache_clear(&mut self, id: NodeId) {
        cache_of(id).borrow_mut().clear();
    }
}

fn cache_of<'a>(id: NodeId) -> &'a RefCell<Cache> {
    // SAFETY: see `cell`.
    let node = unsafe { cell(id) };
    match probe_kind(id) {
        Some(Probe::Measure) => &node.engine.probe_cache,
        Some(Probe::Place) => &node.engine.place_cache,
        None => &node.engine.cache,
    }
}

impl LayoutFlexboxContainer for LayoutTree {
    type FlexboxContainerStyle<'a> = &'a Style;
    type FlexboxItemStyle<'a> = &'a Style;

    fn get_flexbox_container_style(&self, id: NodeId) -> &Style {
        style_of(id)
    }
    fn get_flexbox_child_style(&self, id: NodeId) -> &Style {
        style_of(id)
    }
}

impl LayoutGridContainer for LayoutTree {
    type GridContainerStyle<'a> = &'a Style;
    type GridItemStyle<'a> = &'a Style;
    fn get_grid_container_style(&self, id: NodeId) -> &Style {
        style_of(id)
    }
    fn get_grid_child_style(&self, id: NodeId) -> &Style {
        style_of(id)
    }
}

fn definite(value: Option<f32>) -> AvailableSpace {
    value.map_or(AvailableSpace::MaxContent, AvailableSpace::Definite)
}

/// Lay out `root` against the owner viewport.
pub(super) fn layout_root(
    root: &LayoutNode,
    width: Option<f32>,
    height: Option<f32>,
    direction: Direction,
) {
    let inherited = if direction == Direction::Rtl {
        Direction::Rtl
    } else {
        Direction::Ltr
    };
    refresh_style(&root.0, inherited);
    let id = node_id(root);
    let style = style_of(id);
    let parent_size = taffy::Size { width, height };
    // A viewport-filling root: an auto dimension takes the owner's extent.
    let known = taffy::Size {
        width: style.size.width.is_auto().then_some(width).flatten(),
        height: style.size.height.is_auto().then_some(height).flatten(),
    };
    let inputs = LayoutInput {
        run_mode: RunMode::PerformLayout,
        sizing_mode: SizingMode::InherentSize,
        axis: RequestedAxis::Both,
        known_dimensions: known,
        known_dimensions_are_definite: taffy::Size {
            width: true,
            height: true,
        },
        parent_size,
        available_space: taffy::Size {
            width: definite(width),
            height: definite(height),
        },
        vertical_margins_are_collapsible: Line::FALSE,
    };
    let mut tree = LayoutTree;
    let output = tree.compute_child_layout(id, inputs);
    let resolve = resolve_calc;
    let style = style_of(id);
    let mut layout = Layout::with_order(0);
    layout.size = output.size;
    layout.padding = style.padding.resolve_or_zero(width, resolve);
    layout.border = style.border.resolve_or_zero(width, resolve);
    layout.margin = style.margin.resolve_or_zero(width, resolve);
    tree.set_unrounded_layout(id, &layout);
}

/// Lay out `child` of a custom layout inside its probe and return its size.
pub(super) fn layout_probe(
    child: &LayoutNode,
    width: Option<f32>,
    height: Option<f32>,
    direction: Direction,
    probe: Probe,
) -> Size {
    refresh_style(&child.0, direction);
    let inputs = LayoutInput {
        run_mode: RunMode::PerformLayout,
        sizing_mode: SizingMode::InherentSize,
        axis: RequestedAxis::Both,
        known_dimensions: taffy::Size { width, height },
        known_dimensions_are_definite: taffy::Size {
            width: true,
            height: true,
        },
        parent_size: taffy::Size { width, height },
        available_space: taffy::Size {
            width: definite(width),
            height: definite(height),
        },
        vertical_margins_are_collapsible: Line::FALSE,
    };
    let mut tree = LayoutTree;
    tree.compute_child_layout(probe_id(child, probe), inputs);
    let size = child.0.engine.layout().size;
    Size {
        width: size.width,
        height: size.height,
    }
}
