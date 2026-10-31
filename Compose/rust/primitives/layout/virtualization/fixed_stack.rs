//! Translation of Primitives/Layout/Virtualization/FixedStack.swift.
//!
//! Fixed-height rows are created only within the viewport plus overscan. Row
//! identity survives scrolling; persistent row data belongs in the model.
use std::cell::RefCell;
use std::rc::Rc;

use crate::modifiers::layout::LayoutFrameExt;
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollViewport, ScrollbarGutter};
use crate::{AnyView, BoxView, PrimitiveView, View};
use cove_core::Point;
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode, Node, Wrap};

use super::virtual_window::{VirtualRowKey, VirtualWindow};

/// Inclusive-exclusive row range covering `[offset, offset + height)`.
pub fn visible_range(
    count: usize,
    row_stride: f32,
    offset: f32,
    height: f32,
    overscan: usize,
) -> std::ops::Range<usize> {
    if count == 0 || row_stride <= 0.0 || height <= 0.0 {
        return 0..0;
    }
    let first = (offset / row_stride).floor().max(0.0) as usize;
    let last = ((offset + height) / row_stride).ceil() as usize;
    let start = first.saturating_sub(overscan).min(count);
    let end = (last + overscan).min(count);
    start..end.max(start)
}

/// Total extent of `count` rows at `row_stride`, excluding the trailing gap.
pub fn total_extent(count: usize, row_stride: f32, spacing: f32) -> f32 {
    if count == 0 {
        return 0.0;
    }
    (count as f32 * row_stride - spacing).max(0.0)
}

/// Vertically scrolling stack of fixed-height rows.
pub struct FixedVirtualStack<Id: Clone> {
    pub ids: Vec<Id>,
    pub row_height: f32,
    pub spacing: f32,
    pub overscan: usize,
    pub preload: f32,
    pub scroll_to_index: Option<usize>,
    pub on_scroll_offset_change: Option<Rc<dyn Fn(Point)>>,
    pub row_content: Rc<dyn Fn(&Id) -> AnyView + 'static>,
    viewport: cove_core::State<ScrollViewport>,
    window: Rc<RefCell<VirtualWindow>>,
    previous_target: cove_core::State<Option<usize>>,
    generation: u128,
}

impl<Id: Clone + 'static> FixedVirtualStack<Id> {
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
            on_scroll_offset_change: None,
            row_content: Rc::new(row_content),
            viewport: cove_core::State::new(ScrollViewport::default()),
            window: Rc::new(RefCell::new(VirtualWindow::default())),
            previous_target: cove_core::State::new(None),
            generation: 1,
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
        self
    }

    pub fn on_scroll_offset_change(mut self, handler: impl Fn(Point) + 'static) -> Self {
        self.on_scroll_offset_change = Some(Rc::new(handler));
        self
    }

    fn stride(&self) -> f32 {
        self.row_height + self.spacing
    }
}

impl<Id: Clone + 'static> View for FixedVirtualStack<Id> {
    fn body(&self) -> AnyView {
        let stride = self.stride();
        let count = self.ids.len();
        let viewport_size = self.viewport.get();
        let mut window = self.window.borrow_mut();
        // The first layout has no viewport yet. Revealing against a zero-height
        // viewport scrolls a row too far and the request is then consumed, so a
        // target that is already visible still starts the stack scrolled past
        // its own top.
        if let Some(target) = self.scroll_to_index
            && self.previous_target.get() != Some(target)
            && viewport_size.size.height > 0.0
        {
            self.previous_target.set(Some(target));
            let top = target.min(count.saturating_sub(1)) as f32 * stride;
            let bottom = top + self.row_height;
            let mut offset = window.offset;
            if top < offset.y {
                offset.y = top;
            } else if bottom > offset.y + viewport_size.size.height {
                offset.y = (bottom - self.row_height.max(viewport_size.size.height)).max(0.0);
            }
            window.offset = offset;
        }
        let total = total_extent(count, stride, self.spacing);
        let clamped = window
            .offset
            .y
            .max(0.0)
            .min((total - viewport_size.size.height).max(0.0));
        let visible = visible_range(count, stride, clamped, viewport_size.size.height, 0);
        let (prepared_offset, prepared_height) = VirtualWindow::preparation(
            f64::from(clamped),
            f64::from(viewport_size.size.height),
            self.preload,
        );
        let range = visible_range(
            count,
            stride,
            prepared_offset as f32,
            prepared_height as f32,
            self.overscan,
        );
        window.record(range.clone(), visible, count, viewport_size.size);
        let window_offset = window.offset;
        drop(window);

        let container_height = total;
        let mut rows: Vec<AnyView> = Vec::new();
        for index in range {
            let Some(id) = self.ids.get(index) else {
                continue;
            };
            let content = (self.row_content)(id);
            rows.push(AnyView::new(
                AnyView::new(RowSlot {
                    key: VirtualRowKey::new(self.generation, index, self.row_height),
                    content,
                })
                .frame_height(self.row_height),
            ));
        }
        let offset_state = cove_core::State::new(window_offset);
        let offset_binding = cove_core::Binding::new(
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
        let on_change = self.on_scroll_offset_change.clone();
        let viewport = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                self.spacing,
                AnyView::new(crate::TupleView::new(rows)),
            )
            .frame(None, None, None, None, None, Some(container_height)),
        );
        let scroll = ScrollView::new(ScrollAxis::Vertical, viewport)
            .scrollbar_gutter(ScrollbarGutter::Stable)
            .scroll_offset(offset_binding);
        let scroll = match on_change {
            Some(handler) => scroll.on_viewport_change(move |next| handler(next.offset)),
            None => scroll,
        };
        AnyView::new(AnyView::new(scroll).frame(None, None, Some(0.0), Some(0.0), None, None))
    }
}

/// Positions one virtual row absolutely at its index, so offscreen rows take no
/// layout space.
struct RowSlot {
    key: VirtualRowKey,
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
crate::impl_primitive_view!(RowSlot);
