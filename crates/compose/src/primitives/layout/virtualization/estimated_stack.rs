//! Translation of Primitives/Layout/Virtualization/EstimatedStack.swift.
//!
//! Rows of unknown height: unmeasured rows use the estimate, and each mounted
//! row's laid-out height replaces the estimate for the current width and text
//! scale. Recording a new height bumps a revision so the next frame recomputes
//! the window.
use crate::Scope;
use std::cell::RefCell;
use std::rc::Rc;

use crate::primitives::layout::ScrollViewport;
use crate::{AnyView, PrimitiveView, View};
use cove_core::scene::{Align, FlexDirection, LayoutNode, Node};
use cove_core::{Point, Size};

use super::row_index::VariableRowIndex;
use super::virtual_stack_body::virtual_scroll;
use super::virtual_window::VirtualWindow;

/// Per-row measured heights, keyed by row id so a reorder keeps its measurements.
pub struct EstimatedRowMetrics<Id: Ord + Clone> {
    measured: std::collections::BTreeMap<Id, f32>,
    revision: u64,
}

impl<Id: Ord + Clone> Default for EstimatedRowMetrics<Id> {
    fn default() -> Self {
        Self {
            measured: std::collections::BTreeMap::new(),
            revision: 0,
        }
    }
}

impl<Id: Ord + Clone> EstimatedRowMetrics<Id> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one measured height. Returns `true` when it changed, which is the
    /// signal to recompute the window.
    pub fn record(&mut self, id: Id, height: f32) -> bool {
        if !height.is_finite() || height < 0.0 {
            return false;
        }
        match self.measured.get(&id) {
            Some(existing) if (*existing - height).abs() < 0.5 => false,
            _ => {
                self.measured.insert(id, height);
                self.revision = self.revision.wrapping_add(1);
                true
            }
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn height_of(&self, id: &Id, estimate: f32) -> f32 {
        self.measured.get(id).copied().unwrap_or(estimate)
    }
}

struct EstimatedState<Id: Ord + Clone> {
    metrics: EstimatedRowMetrics<Id>,
    rows: VariableRowIndex<Id>,
    window: VirtualWindow,
    previous_target: Option<usize>,
    revision: u64,
}

/// Vertically scrolling stack whose row heights are measured as rows mount.
pub struct EstimatedVirtualStack<Id: Ord + Clone + 'static> {
    pub ids: Vec<Id>,
    pub estimate: f32,
    pub spacing: f32,
    pub overscan: usize,
    pub preload: f32,
    pub scroll_to_index: Option<usize>,
    pub row_content: Rc<dyn Fn(&Id) -> AnyView>,
    state: Rc<RefCell<EstimatedState<Id>>>,
    offset: cove_core::State<Point>,
    viewport: cove_core::State<ScrollViewport>,
}

impl<Id: Ord + Clone + 'static> EstimatedVirtualStack<Id> {
    pub fn new(
        ids: Vec<Id>,
        estimate: f32,
        row_content: impl Fn(&Id) -> AnyView + 'static,
    ) -> Self {
        Self {
            ids,
            estimate: if estimate.is_finite() {
                estimate.max(1.0)
            } else {
                44.0
            },
            spacing: 0.0,
            overscan: 3,
            preload: 0.3,
            scroll_to_index: None,
            row_content: Rc::new(row_content),
            state: Rc::new(RefCell::new(EstimatedState {
                metrics: EstimatedRowMetrics::new(),
                rows: VariableRowIndex::new(),
                window: VirtualWindow::default(),
                previous_target: None,
                revision: 0,
            })),
            offset: cove_core::State::new(Point::default()),
            viewport: cove_core::State::new(ScrollViewport::default()),
        }
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = if spacing.is_finite() {
            spacing.max(0.0)
        } else {
            0.0
        };
        self
    }

    pub fn overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    pub fn preload(mut self, preload: f32) -> Self {
        self.preload = preload;
        self
    }

    pub fn scroll_to_index(mut self, index: usize) -> Self {
        self.scroll_to_index = Some(index);
        self
    }

    /// Rebuild the index from the current measurements.
    pub fn synchronize(&self, width: f32, viewport: ScrollViewport) {
        let mut state = self.state.borrow_mut();
        let heights: Vec<f32> = self
            .ids
            .iter()
            .map(|id| state.metrics.height_of(id, self.estimate))
            .collect();
        state.rows.replace(&self.ids, &heights, self.spacing);
        state.revision = state.metrics.revision();
        let _ = (width, viewport);
    }
}

impl<Id: Ord + Clone + 'static> View for EstimatedVirtualStack<Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        let viewport = self.viewport.get();
        self.synchronize(viewport.size.width, viewport);
        let mut state = self.state.borrow_mut();
        let mut next_offset = state.window.offset.y as f64;
        if self.scroll_to_index.is_none() {
            state.previous_target = None;
        }
        if let Some(target) = self.scroll_to_index
            && state.previous_target != Some(target)
            && viewport.size.height > 0.0
            && !state.rows.is_empty()
        {
            next_offset = state.rows.offset_of_id(
                &self.ids[target.min(self.ids.len() - 1)],
                viewport.size.height as f64,
            );
            state.previous_target = Some(target);
        }
        state.window.offset.y = next_offset as f32;
        let visible = state
            .rows
            .visible_range(next_offset, viewport.size.height as f64);
        let count = state.rows.len();
        state.window.record(
            visible.clone(),
            visible,
            count,
            Size {
                width: viewport.size.width,
                height: viewport.size.height,
            },
        );
        let rows = state.rows.clone();
        drop(state);

        let offset = self.offset.get();
        let viewport_state = self.viewport.clone();
        let offset_state = self.offset.clone();
        virtual_scroll(
            &rows,
            &self.ids,
            offset,
            viewport,
            self.overscan,
            self.preload,
            1,
            &self.row_content,
            Some(Rc::new(move |next: Point| offset_state.set(next))),
            Some(Rc::new(move |next: ScrollViewport| {
                viewport_state.set(next)
            })),
        )
    }

    /// `body` reads `offset`, `viewport` from `State`, so the
    /// scope must declare those dependencies. Without this the writes never
    /// subscribe and the view keeps its first layout forever.
    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![
            crate::DynamicField::state("offset", &self.offset),
            crate::DynamicField::state("viewport", &self.viewport),
        ]
    }
}

/// Reports its laid-out height so the estimate can be replaced.
pub(crate) struct EstimatedRowHost {
    pub on_height: Rc<dyn Fn(f32)>,
    pub content: AnyView,
}

impl PrimitiveView for EstimatedRowHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let on_height = self.on_height.clone();
        let weak = node.downgrade();
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            let _ = Node::upgrade(&weak);
            on_height(owner.frame().height);
        }) as cove_core::scene::LayoutUpdateHook));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(EstimatedRowHost);
