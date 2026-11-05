//! Translation of Primitives/Layout/Virtualization/FixedStack.swift.
//!
//! Fixed-height rows are created only within the viewport plus overscan. Row
//! identity survives scrolling; persistent row data belongs in the model.
use crate::Scope;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crate::modifiers::layout::{FrameDimension, LayoutFrameExt};
use crate::modifiers::style::AppearanceExt;
use crate::primitives::data::lists::reorder_model::ROW_SHIFT_ANCHOR;
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollViewport, ScrollbarGutter};
use crate::{AnyView, BoxView, PrimitiveView, View, ViewExt};
use cove_composition::MemoizedView;
use cove_core::Point;
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode, Node, Wrap};

use super::virtual_window::{VirtualRowKey, VirtualWindow};

/// Row range covering `[offset, offset + max(stride, height))` plus `overscan`
/// rows on each side. A zero-height viewport still yields the first row, so the
/// stack mounts something before its first layout reports a viewport.
pub fn visible_range(
    count: usize,
    row_stride: f32,
    offset: f32,
    height: f32,
    overscan: usize,
) -> std::ops::Range<usize> {
    let stride = row_stride.max(1.0);
    let offset = offset.max(0.0);
    let first = ((offset / stride) as usize)
        .saturating_sub(overscan)
        .min(count);
    let last = (((offset + stride.max(height)) / stride).ceil() as usize + overscan)
        .max(first)
        .min(count);
    first..last
}

/// Total extent of `count` rows at `row_stride`, excluding the trailing gap.
pub fn total_extent(count: usize, row_stride: f32, spacing: f32) -> f32 {
    if count == 0 {
        return 0.0;
    }
    (count as f32 * row_stride - spacing).max(0.0)
}

/// Where a requested row lands in the viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VirtualScrollAlign {
    /// Reveal the row with the least movement.
    #[default]
    Auto,
    /// Put the row at the top edge.
    Start,
    /// Centre the row.
    Center,
    /// Put the row at the bottom edge.
    End,
}

/// Vertically scrolling stack of fixed-height rows.
pub struct FixedVirtualStack<Id: Clone> {
    pub ids: Vec<Id>,
    pub row_height: f32,
    pub spacing: f32,
    pub overscan: usize,
    pub preload: f32,
    pub scroll_to_index: Option<usize>,
    pub scroll_to_align: VirtualScrollAlign,
    pub on_scroll_offset_change: Option<Rc<dyn Fn(Point)>>,
    pub row_content: Rc<dyn Fn(&Id) -> AnyView + 'static>,
    viewport: cove_core::State<ScrollViewport>,
    /// Shared like Swift's `@State` class: the reference survives recomposition
    /// and mutating it never invalidates the body.
    window: cove_core::State<Arc<Mutex<VirtualWindow>>>,
    previous_target: cove_core::State<Option<(usize, VirtualScrollAlign)>>,
    generation: u128,
}

impl<Id: Clone + Hash + Eq + 'static> FixedVirtualStack<Id> {
    pub fn new(
        ids: Vec<Id>,
        row_height: f32,
        row_content: impl Fn(&Id) -> AnyView + 'static,
    ) -> Self {
        Self {
            ids,
            row_height: row_height.max(1.0),
            spacing: 0.0,
            overscan: 3,
            preload: 0.3,
            scroll_to_index: None,
            scroll_to_align: VirtualScrollAlign::Auto,
            on_scroll_offset_change: None,
            row_content: Rc::new(row_content),
            viewport: cove_core::State::new(ScrollViewport::default()),
            window: cove_core::State::new(Arc::new(Mutex::new(VirtualWindow::default()))),
            previous_target: cove_core::State::new(None),
            generation: next_generation(),
        }
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
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
        self.scroll_to_align = VirtualScrollAlign::Auto;
        self
    }

    /// Reveal `index` at the requested edge or centre.
    pub fn scroll_to_index_aligned(mut self, index: usize, align: VirtualScrollAlign) -> Self {
        self.scroll_to_index = Some(index);
        self.scroll_to_align = align;
        self
    }

    pub fn on_scroll_offset_change(mut self, handler: impl Fn(Point) + 'static) -> Self {
        self.on_scroll_offset_change = Some(Rc::new(handler));
        self
    }

    fn stride(&self) -> f32 {
        self.row_height + self.spacing
    }

    fn row(&self, index: usize) -> AnyView {
        let id = self.ids[index].clone();
        let gap = if index + 1 == self.ids.len() {
            0.0
        } else {
            self.spacing
        };
        let extent = self.row_height + gap;
        let row_height = self.row_height;
        let row_content = self.row_content.clone();
        let content_id = id.clone();
        AnyView::new(
            MemoizedView::new(
                VirtualRowKey::new(self.generation, index, self.stride()),
                move || {
                    AnyView::new(RowSlot {
                        extent,
                        content: AnyView::new(row_content(&content_id).frame_height(row_height)),
                    })
                    .clipped()
                },
            )
            .id(id),
        )
    }
}

impl<Id: Clone + Hash + Eq + 'static> View for FixedVirtualStack<Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        let stride = self.stride();
        let count = self.ids.len();
        let viewport = self.viewport.get();
        let window = self.window.get();
        if self.scroll_to_index.is_none() && self.previous_target.get().is_some() {
            self.previous_target.set(None);
        }
        let mut state = window.lock().unwrap_or_else(PoisonError::into_inner);
        // The first layout has no viewport yet. Revealing against a zero-height
        // viewport scrolls a row too far and the request is then consumed, so a
        // target that is already visible still starts the stack scrolled past
        // its own top.
        if let Some(target) = self.scroll_to_index
            && self.previous_target.get() != Some((target, self.scroll_to_align))
            && viewport.size.height > 0.0
        {
            self.previous_target
                .set(Some((target, self.scroll_to_align)));
            let top = target.min(count.saturating_sub(1)) as f32 * stride;
            let bottom = top + self.row_height;
            let max_offset =
                (total_extent(count, stride, self.spacing) - viewport.size.height).max(0.0);
            state.offset.y = match self.scroll_to_align {
                VirtualScrollAlign::Auto => {
                    if top < state.offset.y {
                        top
                    } else if bottom > state.offset.y + viewport.size.height {
                        (bottom - self.row_height.max(viewport.size.height)).max(0.0)
                    } else {
                        state.offset.y
                    }
                }
                VirtualScrollAlign::Start => top,
                VirtualScrollAlign::Center => {
                    top - ((viewport.size.height - self.row_height) * 0.5).max(0.0)
                }
                VirtualScrollAlign::End => bottom - viewport.size.height,
            }
            .clamp(0.0, max_offset);
        }
        let total = total_extent(count, stride, self.spacing);
        let clamped = state
            .offset
            .y
            .min((total - viewport.size.height).max(0.0))
            .max(0.0);
        let visible = visible_range(count, stride, clamped, viewport.size.height, 0);
        let (prepared_offset, prepared_height) = VirtualWindow::preparation(
            f64::from(clamped),
            f64::from(viewport.size.height),
            self.preload,
        );
        let range = visible_range(
            count,
            stride,
            prepared_offset as f32,
            prepared_height as f32,
            self.overscan,
        );
        state.record(range.clone(), visible, count, viewport.size);
        drop(state);

        let trailing = (total - range.end as f32 * stride
            + if range.end == count {
                self.spacing
            } else {
                0.0
            })
        .max(0.0);
        let mut rows: Vec<AnyView> = Vec::with_capacity(range.len() + 2);
        rows.push(spacer(range.start as f32 * stride));
        rows.extend(range.clone().map(|index| self.row(index)));
        rows.push(spacer(trailing));

        let offset_binding = cove_core::Binding::new(
            {
                let window = window.clone();
                move || window.lock().unwrap_or_else(PoisonError::into_inner).offset
            },
            {
                let window = window.clone();
                move |value: Point| {
                    let mut state = window.lock().unwrap_or_else(PoisonError::into_inner);
                    if state.offset != value {
                        state.offset = value;
                    }
                }
            },
        );
        let viewport_state = self.viewport.clone();
        let refill_window = window.clone();
        let spacing = self.spacing;
        let content = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::TupleView::new(rows)),
            )
            .frame(
                None,
                Some(FrameDimension::Points(total)),
                Some(0.0),
                None,
                None,
                None,
            ),
        );
        let mut scroll = ScrollView::new(ScrollAxis::Vertical, content)
            .scrollbar_gutter(ScrollbarGutter::Stable)
            .scroll_offset(offset_binding)
            .on_viewport_change(move |next: ScrollViewport| {
                let mut state = refill_window.lock().unwrap_or_else(PoisonError::into_inner);
                state.offset = next.offset;
                let total = total_extent(count, stride, spacing);
                let offset = next
                    .offset
                    .y
                    .min((total - next.size.height).max(0.0))
                    .max(0.0);
                let visible = visible_range(count, stride, offset, next.size.height, 0);
                let refill = state.needs_refill(visible, next.size);
                drop(state);
                if refill {
                    viewport_state.set(next);
                }
            });
        if let Some(handler) = self.on_scroll_offset_change.clone() {
            scroll = scroll.on_geometry_change(move |geometry| handler(geometry.offset));
        }
        AnyView::new(AnyView::new(scroll).frame(None, None, Some(0.0), Some(0.0), None, None))
    }

    /// `body` reads `viewport`, `window` and `previous_target` from `State`, so
    /// the scope must declare them; `window` is matched by name so its shared
    /// reference carries over to the next view value.
    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![
            crate::DynamicField::state("viewport", &self.viewport),
            crate::DynamicField::state("window", &self.window),
            crate::DynamicField::state("previous_target", &self.previous_target),
        ]
    }
}

/// Swift gives each stack value a fresh `UUID`, so memoised rows rebuild when
/// the parent recomposes and are reused while only the stack itself scrolls.
fn next_generation() -> u128 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    u128::from(NEXT.fetch_add(1, Ordering::Relaxed))
}

fn spacer(height: f32) -> AnyView {
    AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(crate::EmptyView),
        )
        .frame_height(height),
    )
}

/// Column box for one row's content at its fixed extent.
struct RowSlot {
    extent: f32,
    content: AnyView,
}

impl PrimitiveView for RowSlot {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_height(Some(self.extent));
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.extent));
    }

    fn update_node(&self, node: &Node) {
        // The slot is the clipped box around its row, so a row that a live
        // reorder paints outside its own slot has to be displaced through it.
        node.set_attachment(ROW_SHIFT_ANCHOR, true);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(RowSlot);
