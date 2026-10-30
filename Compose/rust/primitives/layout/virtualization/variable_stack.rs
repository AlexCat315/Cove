//! Translation of Primitives/Layout/Virtualization/VariableStack.swift.
//!
//! Rows whose heights are known before layout. Heights are cached until the
//! data, width, spacing or text scale change, and scrolling locates rows in
//! O(log n).
use std::cell::RefCell;
use std::rc::Rc;

use cove_core::{Point, Size};
use crate::primitives::layout::ScrollViewport;
use crate::{AnyView, View};

use super::row_index::VariableRowIndex;
use super::row_metrics::VariableRowMetrics;
use super::virtual_stack_body::{virtual_scroll, RowExtents};
use super::virtual_window::VirtualWindow;

/// Authoritative window state for one variable stack.
struct VariableState<Id: Ord + Clone> {
    metrics: VariableRowMetrics<Id>,
    window: VirtualWindow,
    viewport: ScrollViewport,
    previous_target: Option<usize>,
    generation: u64,
}

/// Vertically scrolling stack of rows with known variable heights.
pub struct VariableVirtualStack<Id: Ord + Clone + 'static> {
    pub ids: Vec<Id>,
    pub spacing: f32,
    pub overscan: usize,
    pub preload: f32,
    pub scroll_to_index: Option<usize>,
    pub row_height: Rc<dyn Fn(&Id, f32, f32) -> f32>,
    pub row_content: Rc<dyn Fn(&Id) -> AnyView>,
    state: Rc<RefCell<VariableState<Id>>>,
    offset: cove_core::State<Point>,
    viewport: cove_core::State<ScrollViewport>,
}

impl<Id: Ord + Clone + 'static> VariableVirtualStack<Id> {
    pub fn new(
        ids: Vec<Id>,
        row_height: impl Fn(&Id, f32, f32) -> f32 + 'static,
        row_content: impl Fn(&Id) -> AnyView + 'static,
    ) -> Self {
        Self {
            ids,
            spacing: 0.0,
            overscan: 3,
            preload: 0.3,
            scroll_to_index: None,
            row_height: Rc::new(row_height),
            row_content: Rc::new(row_content),
            state: Rc::new(RefCell::new(VariableState {
                metrics: VariableRowMetrics::new(),
                window: VirtualWindow::default(),
                viewport: ScrollViewport::default(),
                previous_target: None,
                generation: 1,
            })),
            offset: cove_core::State::new(Point::default()),
            viewport: cove_core::State::new(ScrollViewport::default()),
        }
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = if spacing.is_finite() { spacing.max(0.0) } else { 0.0 };
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
}

impl<Id: Ord + Clone + 'static> View for VariableVirtualStack<Id> {
    fn body(&self) -> AnyView {
        let viewport = self.viewport.get();
        // Wait for a real width rather than measuring the entire collection
        // twice during initial mount (first at zero, then at the viewport).
        let width = viewport.size.width.max(0.0);
        let mut state = self.state.borrow_mut();
        let mut next_offset = state.window.offset.y as f64;
        if width > 0.0 {
            let heights: Vec<f32> = self
                .ids
                .iter()
                .map(|id| (self.row_height)(id, width, 1.0))
                .collect();
            let generation = state.generation;
            next_offset = state.metrics.synchronize(
                &self.ids,
                &heights,
                generation,
                width,
                1.0,
                self.spacing,
                next_offset,
                viewport.size.height as f64,
            );
        }
        if self.scroll_to_index.is_none() {
            state.previous_target = None;
        }
        if let Some(target) = self.scroll_to_index
            && state.previous_target != Some(target)
            && viewport.size.height > 0.0
            && !state.metrics.rows().is_empty()
        {
            next_offset = state
                .metrics
                .rows()
                .offset_for(&self.ids[target.min(self.ids.len() - 1)], viewport.size.height as f64);
            state.previous_target = Some(target);
        }
        state.window.offset.y = next_offset as f32;
        let visible = state
            .metrics
            .rows()
            .visible_range(next_offset, viewport.size.height as f64);
        let count = state.metrics.rows().len();
        state.window.record(
            visible.clone(),
            visible,
            count,
            Size { width: viewport.size.width, height: viewport.size.height },
        );
        let rows = state.metrics.rows().clone();
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
            Some(Rc::new(move |next: ScrollViewport| viewport_state.set(next))),
        )
    }
}
