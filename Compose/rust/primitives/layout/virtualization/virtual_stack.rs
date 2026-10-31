//! Translation of Primitives/Layout/Virtualization/VirtualStack.swift.
//!
//! A vertically scrolling stack that constructs only the rows around the
//! viewport. Row identity survives scrolling; keep persistent row data in the
//! model, since fully offscreen rows are unmounted. IDs must be unique.
//!
//! Pass a constant `row_height` for uniform rows, a closure for known variable
//! heights, or `estimated_row_height` when heights are only known after
//! rendering. The closure receives the element, the available content width and
//! the inherited text scale, and returns the complete row height; heights are
//! cached until data, width, spacing or text scale change, and scrolling locates
//! rows in O(log n). `preload` adds a fraction of the viewport across both
//! edges; `overscan` is the minimum extra row count per edge.
use std::rc::Rc;

use crate::{AnyView, View};

use super::estimated_stack::EstimatedVirtualStack;
use super::fixed_stack::FixedVirtualStack;
use super::variable_stack::VariableVirtualStack;

/// How row extents are supplied.
pub enum VirtualRows {
    /// Uniform row height.
    Fixed(f32),
    /// Heights known before layout: `(element_id, width, text_scale) -> height`.
    Variable(Rc<dyn Fn(&u64, f32, f32) -> f32>),
    /// Heights measured as rows mount, starting from an estimate.
    Estimated(f32),
}

/// Vertically scrolling stack that mounts only the rows around the viewport.
pub struct VirtualStack<Id: Ord + Clone + 'static> {
    pub rows: VirtualRows,
    pub spacing: f32,
    pub overscan: usize,
    pub preload: f32,
    pub scroll_to_index: Option<usize>,
    marker: std::marker::PhantomData<fn() -> Id>,
}

impl<Id: Ord + Clone + 'static> VirtualStack<Id> {
    fn resolved_spacing(&self) -> f32 {
        if self.spacing.is_finite() {
            self.spacing.max(0.0)
        } else {
            0.0
        }
    }
}

impl<Id: Ord + Clone + 'static> VirtualStack<Id> {
    /// Uniform rows.
    pub fn fixed(row_height: f32) -> Self {
        Self {
            rows: VirtualRows::Fixed(row_height.max(1.0)),
            spacing: 0.0,
            overscan: 3,
            preload: 0.3,
            scroll_to_index: None,
            marker: std::marker::PhantomData,
        }
    }

    /// Rows whose heights are known before layout.
    pub fn variable(row_height: impl Fn(&u64, f32, f32) -> f32 + 'static) -> Self {
        Self {
            rows: VirtualRows::Variable(Rc::new(row_height)),
            spacing: 0.0,
            overscan: 3,
            preload: 0.3,
            scroll_to_index: None,
            marker: std::marker::PhantomData,
        }
    }

    /// Rows of unknown height; each mounted row replaces the estimate.
    pub fn estimated(estimated_row_height: f32) -> Self {
        Self {
            rows: VirtualRows::Estimated(if estimated_row_height.is_finite() {
                estimated_row_height.max(1.0)
            } else {
                44.0
            }),
            spacing: 0.0,
            overscan: 3,
            preload: 0.3,
            scroll_to_index: None,
            marker: std::marker::PhantomData,
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

    /// Build the concrete stack for a data set and row factory.
    pub fn build(&self, ids: Vec<Id>, row_content: impl Fn(&Id) -> AnyView + 'static) -> AnyView {
        let spacing = self.resolved_spacing();
        let content = Rc::new(row_content);
        match &self.rows {
            VirtualRows::Fixed(height) => {
                let mut stack = FixedVirtualStack::new(ids, *height, move |id| content(id))
                    .spacing(spacing)
                    .overscan(self.overscan)
                    .preload(self.preload);
                if let Some(index) = self.scroll_to_index {
                    stack = stack.scroll_to_index(index);
                }
                AnyView::new(stack)
            }
            VirtualRows::Variable(row_height) => {
                let measure = row_height.clone();
                let ids_for_measure = ids.clone();
                let mut stack = VariableVirtualStack::new(
                    ids,
                    move |id, width, scale| {
                        let index = ids_for_measure
                            .iter()
                            .position(|candidate| candidate == id)
                            .unwrap_or(0);
                        measure(&(index as u64), width, scale)
                    },
                    move |id| content(id),
                )
                .spacing(spacing)
                .overscan(self.overscan)
                .preload(self.preload);
                if let Some(index) = self.scroll_to_index {
                    stack = stack.scroll_to_index(index);
                }
                AnyView::new(stack)
            }
            VirtualRows::Estimated(estimate) => {
                let mut stack = EstimatedVirtualStack::new(ids, *estimate, move |id| content(id))
                    .spacing(spacing)
                    .overscan(self.overscan)
                    .preload(self.preload);
                if let Some(index) = self.scroll_to_index {
                    stack = stack.scroll_to_index(index);
                }
                AnyView::new(stack)
            }
        }
    }
}
