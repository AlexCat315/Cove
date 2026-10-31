//! Shared body for the virtualized row stacks.
//!
//! Leading spacer, mounted rows, trailing spacer. Offscreen rows are unmounted,
//! so persistent row data must live in the model. Each stack supplies its own
//! row extents; this module owns the window arithmetic and the scroll wiring.
use std::rc::Rc;

use crate::modifiers::layout::LayoutFrameExt;
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollViewport, ScrollbarGutter};
use crate::{AnyView, BoxView, PrimitiveView, View};
use cove_core::Point;
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode, Node, Wrap};

use super::virtual_window::{VirtualRowKey, VirtualWindow};

/// Row extent query supplied by the concrete stack.
pub trait RowExtents {
    fn count(&self) -> usize;
    fn extent(&self, index: usize) -> f32;
    fn offset_of(&self, index: usize) -> f64;
    fn total(&self) -> f64;
    fn spacing(&self) -> f32;
    fn anchor_at(&self, offset: f64) -> Option<usize>;
    fn offset_for(&self, index: usize, viewport: f64) -> f64;
    fn clamp(&self, offset: f64, viewport: f64) -> f64;
}

/// `VariableRowIndex` is the row-extent source for both the known-height and the
/// measured stacks; the impl lives here so there is exactly one.
impl<Id: Ord + Clone> RowExtents for super::row_index::VariableRowIndex<Id> {
    fn count(&self) -> usize {
        self.len()
    }

    fn extent(&self, index: usize) -> f32 {
        self.height_of(index)
    }

    fn offset_of(&self, index: usize) -> f64 {
        super::row_index::VariableRowIndex::offset_of(self, index)
    }

    fn total(&self) -> f64 {
        self.total_height()
    }

    fn spacing(&self) -> f32 {
        0.0
    }

    fn anchor_at(&self, offset: f64) -> Option<usize> {
        if self.is_empty() {
            None
        } else {
            Some(self.row_at(offset))
        }
    }

    fn offset_for(&self, index: usize, viewport: f64) -> f64 {
        self.clamp(
            super::row_index::VariableRowIndex::offset_of(self, index),
            viewport,
        )
    }

    fn clamp(&self, offset: f64, viewport: f64) -> f64 {
        super::row_index::VariableRowIndex::clamp(self, offset, viewport)
    }
}

/// Resolved window for one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VirtualFrame {
    pub range: std::ops::Range<usize>,
    pub before: f32,
    pub after: f32,
    pub total: f32,
}

/// Compute the mounted window plus the spacers that place it.
pub fn resolve_frame(
    rows: &impl RowExtents,
    offset: f64,
    viewport: f64,
    overscan: usize,
    preload: f32,
) -> VirtualFrame {
    let count = rows.count();
    let clamped = rows.clamp(offset, viewport);
    let (prepared_offset, prepared_height) = VirtualWindow::preparation(clamped, viewport, preload);
    let start = rows.anchor_at(prepared_offset).unwrap_or(0);
    let end_row = rows
        .anchor_at(prepared_offset + prepared_height)
        .unwrap_or(count.saturating_sub(1));
    let start = start.saturating_sub(overscan).min(count);
    let end = (end_row + 1 + overscan).min(count).max(start);
    let before = rows.offset_of(start) as f32;
    let total = rows.total() as f32;
    let after = (total - rows.offset_of(end) as f32).max(0.0);
    VirtualFrame {
        range: start..end,
        before,
        after,
        total,
    }
}

/// Builds the scroll view for a virtualized stack.
pub fn virtual_scroll<Id: Clone + 'static>(
    rows: &impl RowExtents,
    ids: &[Id],
    offset: Point,
    viewport: ScrollViewport,
    overscan: usize,
    preload: f32,
    generation: u128,
    row_content: &Rc<dyn Fn(&Id) -> AnyView + 'static>,
    on_offset_change: Option<Rc<dyn Fn(Point)>>,
    on_viewport_change: Option<Rc<dyn Fn(ScrollViewport)>>,
) -> AnyView {
    let frame = resolve_frame(
        rows,
        offset.y as f64,
        viewport.size.height as f64,
        overscan,
        preload,
    );
    let mut children: Vec<AnyView> = Vec::new();
    if frame.before > 0.0 {
        children.push(AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::EmptyView),
            )
            .frame_height(frame.before),
        ));
    }
    for index in frame.range.clone() {
        let Some(id) = ids.get(index) else { continue };
        let content = (row_content)(id);
        let extent = rows.extent(index);
        let gap = if index + 1 < rows.count() {
            rows.spacing()
        } else {
            0.0
        };
        children.push(AnyView::new(
            AnyView::new(VirtualRowSlot {
                key: VirtualRowKey::new(generation, index, extent),
                content,
            })
            .frame_height(extent + gap),
        ));
    }
    if frame.after > 0.0 {
        children.push(AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::EmptyView),
            )
            .frame_height(frame.after),
        ));
    }
    let content = AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(crate::TupleView::new(children)),
        )
        .frame(None, None, Some(0.0), None, None, Some(frame.total)),
    );
    // The offset binding is a `State<Point>` so it can cross the `Binding`
    // `Send + Sync` bound; the authoritative window stays on the stack.
    let offset_state = cove_core::State::new(offset);
    offset_state.set(offset);
    let binding = cove_core::Binding::new(
        {
            let offset_state = offset_state.clone();
            move || offset_state.get()
        },
        {
            let offset_state = offset_state.clone();
            move |value: Point| {
                if offset_state.get() != value {
                    offset_state.set(value);
                }
            }
        },
    );
    let scroll = ScrollView::new(ScrollAxis::Vertical, content)
        .scrollbar_gutter(ScrollbarGutter::Stable)
        .scroll_offset(binding);
    let scroll = match on_viewport_change.clone() {
        Some(handler) => scroll.on_viewport_change(move |next| handler(next)),
        None => scroll,
    };
    let scroll = match on_offset_change {
        Some(handler) => scroll.on_viewport_change(move |next| handler(next.offset)),
        None => scroll,
    };
    AnyView::new(AnyView::new(scroll).frame(None, None, Some(0.0), Some(0.0), None, None))
}

/// One mounted row at its index.
struct VirtualRowSlot {
    key: VirtualRowKey,
    content: AnyView,
}

impl PrimitiveView for VirtualRowSlot {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_height(Some(self.key.extent));
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.key.extent));
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(VirtualRowSlot);
