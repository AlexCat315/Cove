//! Translation of Primitives/Data/Lists/Reorder/{Reorderable,Handle,Preview}.swift
//! (the view half; the drag arithmetic lives in [`reorder_model`]).
//!
//! List whose rows can be reordered with a drag handle or the keyboard, along
//! either the vertical or the horizontal axis (`.axis(ScrollAxis::Horizontal)`;
//! `Both` reorders vertically). The caller owns the order: `on_move` receives
//! the source index and the final index after the row is removed.
//!
//! The handle is a 44pt-wide target and is the only drag surface, so row content
//! keeps its own buttons and taps. While dragging, the list is painted in the
//! order the drop would produce: the lifted row's slot travels to the drop
//! position and every row it passes over slides one stride behind it. Escape
//! cancels without calling `on_move`. On release the lifted copy springs onto its
//! slot while the displaced rows spring from where they already are, and reduced
//! motion snaps both.
//!
//! Behaviour checklist:
//! - the drag position is pointer travel plus automatic scroll, so the drop
//!   boundary stays in content coordinates while the list scrolls
//! - the rows between the source and the drop hold their projected displacement
//!   while the drag is live, so the drop itself needs no second slide
//! - keyboard Up/Down move the focused handle; assistive clients use
//!   increment/decrement
//! - the dragged and last keyboard-moved rows always stay mounted so pointer
//!   capture and focus survive auto-scrolling
//! - the dragged row stays as a dimmed placeholder that travels with the
//!   projected drop while the lifted copy follows the pointer
//! - after a drop the placeholder crossfades up to full opacity as the lifted
//!   copy lands on it; a keyboard move springs the displaced rows into place

use crate::Scope;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use super::reorder_model::{
    ReorderDragSession, ReorderRowShift, ReorderableListModel, reorder_row_height,
};
use super::selection::{CollectionId, RowState};
use crate::modifiers::interaction::{AccessibilityExt, HitTestingExt};
use crate::modifiers::layout::{AbsolutePositionModifier, FlexExt, LayoutFrameExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::adaptive::apply_touch_control_extent;
use crate::primitives::interaction::edge_auto_scroll::EdgeAutoScroll;
use crate::primitives::layout::virtualization::fixed_stack::visible_range;
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollViewport, ScrollbarGutter};
use crate::primitives::media::{Icon, ui_icon_catalog};
use crate::theme::theme_for_node;
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_composition::Scancode;
use cove_composition::ViewExt;
use cove_core::State;
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, Align, EventPhase,
    EventResult, FlexDirection, HoverPhase, InputHandlerRoute, InteractionRegistryHolder, Justify,
    LayoutNode, Node, NodeWeak, PointerCaptureAttachmentKey, PointerCaptureHolder, PointerPhase,
    Wrap,
};
use cove_platform::{KeyEvent, MouseButton, SystemCursor};

const HANDLE_ACTIVE: &str = "__reorder.active";
/// The pointer coordinate along the list's main axis when the drag began.
const HANDLE_START_Y: &str = "__reorder.startY";
const HANDLE_HOVERED: &str = "__reorder.hovered";
/// The latest main-axis pointer coordinate; auto-scroll rereads it after each
/// scroll step.
const HANDLE_LAST_Y: &str = "__reorder.lastY";
const ROW_PLACEHOLDING: &str = "__reorder.placeholdering";

/// Everything a handle needs to drive one drag. Cloning keeps the same stores,
/// so a clone is the same list rather than a copy — this is what lets the
/// handle closures act without borrowing the view for the node's lifetime.
pub struct ReorderSession<Id> {
    session: Rc<ReorderDragSession>,
    dragging: State<Option<Id>>,
    boundary: State<Option<usize>>,
    retained: State<Option<Id>>,
    window: State<std::ops::Range<usize>>,
    width: State<f32>,
    height: State<f32>,
    /// Latest viewport offset along the main axis, kept in state so a rebuild
    /// can hand the same geometry to the replacement drag session.
    scroll: State<f32>,
    ids: Rc<RefCell<Vec<Id>>>,
    on_move: Rc<RefCell<Rc<dyn Fn(usize, usize)>>>,
    is_enabled: bool,
    row_height: Rc<Cell<f32>>,
    row_spacing: Rc<Cell<f32>>,
    axis: Rc<Cell<ScrollAxis>>,
    /// Rows displaced by the latest drop or keyboard move, as `(id, offset)`
    /// in the order they were laid out *before* the move. Each row consumes
    /// its entry once, when the rebuilt row mounts its slide.
    shifts: State<Vec<(Id, f32)>>,
    /// The landing in flight, as `(release, target)` in viewport coordinates.
    /// Kept as state so a parent rebuild can restart the spring where the
    /// previous session left it instead of dropping the preview at its origin.
    settle: State<Option<(f32, f32)>>,
    /// Mounted row nodes by id, so a live drag displaces rows without
    /// recomposing the list. Entries whose node is gone are pruned on use.
    row_nodes: Rc<RefCell<HashMap<Id, NodeWeak>>>,
    /// The hole index the mounted rows are displaced around, or `None` while
    /// they follow the authored order.
    live_destination: Rc<Cell<Option<usize>>>,
}

impl<Id: CollectionId> Clone for ReorderSession<Id> {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            dragging: self.dragging.clone(),
            boundary: self.boundary.clone(),
            retained: self.retained.clone(),
            window: self.window.clone(),
            width: self.width.clone(),
            height: self.height.clone(),
            scroll: self.scroll.clone(),
            ids: self.ids.clone(),
            on_move: self.on_move.clone(),
            is_enabled: self.is_enabled,
            row_height: self.row_height.clone(),
            row_spacing: self.row_spacing.clone(),
            axis: self.axis.clone(),
            shifts: self.shifts.clone(),
            settle: self.settle.clone(),
            row_nodes: self.row_nodes.clone(),
            live_destination: self.live_destination.clone(),
        }
    }
}

impl<Id: CollectionId> ReorderSession<Id> {
    /// The count of authored rows, as the last projection saw them.
    pub fn count(&self) -> usize {
        self.ids.borrow().len()
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging.get().is_some()
    }

    pub fn dragging_id(&self) -> Option<Id> {
        self.dragging.get()
    }

    fn index_of(&self, id: &Id) -> Option<usize> {
        self.ids.borrow().iter().position(|other| other == id)
    }

    pub fn is_row_dragging(&self, id: &Id) -> bool {
        self.dragging.get().as_ref() == Some(id)
    }

    pub fn axis(&self) -> ScrollAxis {
        self.axis.get()
    }

    pub fn begin_drag(&self, index: usize) {
        if !self.is_enabled {
            return;
        }
        let row_height = self.row_height.get();
        let stride = row_height + self.row_spacing.get();
        self.shifts.set(Vec::new());
        self.settle.set(None);
        self.clear_live_shifts();
        self.live_destination.set(Some(index));
        self.session.begin(index as f32 * stride, row_height);
        self.dragging.set(self.ids.borrow().get(index).cloned());
        self.boundary.set(Some(index));
    }

    pub fn update_drag(&self, travel: f32, scrolled: f32) {
        self.session.move_by(travel);
        let Some(dragging) = self.dragging.get() else {
            return;
        };
        let Some(source) = self.index_of(&dragging) else {
            return;
        };
        let stride = self.row_height.get() + self.row_spacing.get();
        let boundary =
            ReorderableListModel::drop_boundary(source, travel, scrolled, stride, self.count());
        self.boundary.set(Some(boundary));
        self.apply_live_shifts(source, boundary);
    }

    /// Applies the drop, then springs the lifted row onto its slot. `dragging`
    /// stays set until the landing finishes so the placeholder crossfade and
    /// the slide of the displaced rows read as one motion.
    pub fn finish_drag(self: &Rc<Self>) {
        let dragging = self.dragging.get();
        let boundary = self.boundary.get();
        self.boundary.set(None);
        let Some(dragging) = dragging else {
            self.session.end();
            return;
        };
        let Some(source) = self.index_of(&dragging) else {
            self.landing_finished();
            return;
        };
        let stride = self.row_height.get() + self.row_spacing.get();
        let destination = boundary.and_then(|b| ReorderableListModel::destination(source, b));
        if let Some(destination) = destination {
            // The live projection already moved the rows out of the way, so the
            // data change puts each where it is already painted. Clearing the
            // offsets before `on_move` hands the frames back to layout.
            self.clear_live_shifts();
            self.retained.set(Some(dragging.clone()));
            let on_move = self.on_move.borrow().clone();
            on_move(source, destination);
        }
        let landing = destination.unwrap_or(source) as f32 * stride;
        self.land_at(landing);
    }

    /// Cancels without reporting a move, but still springs the lifted row back
    /// onto its source slot instead of dropping it out from under the pointer.
    pub fn cancel_drag(self: &Rc<Self>) {
        self.boundary.set(None);
        let Some(dragging) = self.dragging.get() else {
            self.session.end();
            return;
        };
        match self.index_of(&dragging) {
            Some(source) => {
                let stride = self.row_height.get() + self.row_spacing.get();
                self.apply_live_shifts(source, source);
                self.land_at(source as f32 * stride);
            }
            None => {
                self.landing_finished();
            }
        }
    }

    /// Springs the preview to `content_main` and clears the drag once it
    /// lands. The epoch guard keeps a stale landing from clobbering a drag
    /// that started while it was still flying.
    fn land_at(self: &Rc<Self>, content_main: f32) {
        let target = content_main - self.session.scroll_main();
        let release = self.session.preview_main();
        let epoch = self.session.epoch();
        let weak = Rc::downgrade(self);
        let started = self.session.settle_to(
            target,
            Rc::new(move || {
                let Some(session) = weak.upgrade() else {
                    return;
                };
                if session.session.epoch() != epoch {
                    return;
                }
                session.landing_finished();
            }),
        );
        if started {
            self.settle.set(Some((release, target)));
        }
    }

    /// Continues a landing that a parent rebuild interrupted: the drag session
    /// is fresh, so re-arm the spring from the transferred release and target.
    fn resume_landing(self: &Rc<Self>) {
        if self.session.is_settling() {
            return;
        }
        let Some((release, target)) = self.settle.get() else {
            return;
        };
        let epoch = self.session.epoch();
        let weak = Rc::downgrade(self);
        self.session.resume_settle(
            release,
            target,
            Rc::new(move || {
                let Some(session) = weak.upgrade() else {
                    return;
                };
                if session.session.epoch() != epoch {
                    return;
                }
                session.landing_finished();
            }),
        );
    }

    fn landing_finished(&self) {
        self.dragging.set(None);
        self.retained.set(None);
        self.settle.set(None);
        self.session.end();
    }

    /// Re-adopts the viewport geometry for a drag that outlived the session
    /// value it started in: a parent rebuild replaces the drag session, whose
    /// measured extent and scroll offset then read as zero until the next
    /// viewport report.
    fn restore_drag(&self, row_main_in_content: f32, row_extent: f32, axis: ScrollAxis) {
        if self.session.is_active() {
            return;
        }
        let extent_main = if axis == ScrollAxis::Horizontal {
            self.width.get()
        } else {
            self.height.get()
        };
        self.session.set_scroll_main(self.scroll.get());
        self.session.set_viewport_extent(extent_main);
        self.session.ensure_begun(row_main_in_content, row_extent);
    }

    /// Queues the one-stride slide for every row the move displaces. Must run
    /// before `on_move` so the ids still describe the pre-move order.
    fn queue_shifts(&self, source: usize, destination: usize, stride: f32) {
        let ids = self.ids.borrow();
        let mut shifts = Vec::new();
        if source < destination {
            // Items at (source, destination] slide toward the freed slot.
            for slot in source..destination {
                if let Some(id) = ids.get(slot + 1) {
                    shifts.push((id.clone(), stride));
                }
            }
        } else {
            // Items at [destination, source) slide back by one stride.
            for slot in destination..source {
                if let Some(id) = ids.get(slot) {
                    shifts.push((id.clone(), -stride));
                }
            }
        }
        self.shifts.set(shifts);
    }

    /// Consumes the pending slide for `id`, if the latest move displaced it.
    pub fn take_shift(&self, id: &Id) -> Option<f32> {
        let mut shifts = self.shifts.get();
        let position = shifts.iter().position(|(other, _)| other == id)?;
        let offset = shifts.remove(position).1;
        self.shifts.set(shifts);
        Some(offset)
    }

    /// Records the node behind a mounted row so the live drag can move it.
    fn bind_row_node(&self, id: &Id, node: &Node) {
        let mut nodes = self.row_nodes.borrow_mut();
        if nodes.len() > 2 * self.count() + 8 {
            nodes.retain(|_, weak| weak.upgrade().is_some());
        }
        nodes.insert(id.clone(), node.downgrade());
    }

    /// Places a row that mounts mid-drag at the displacement its projection
    /// already holds, so auto-scrolling new rows into view does not break the
    /// ordering the drag is showing.
    fn restore_live_shift(&self, id: &Id, node: &Node, axis: ScrollAxis) {
        let Some(destination) = self.live_destination.get() else {
            return;
        };
        let Some(dragging) = self.dragging.get() else {
            return;
        };
        let Some(source) = self.index_of(&dragging) else {
            return;
        };
        let Some(index) = self.index_of(id) else {
            return;
        };
        let stride = self.row_height.get() + self.row_spacing.get();
        let offset = ReorderableListModel::live_offset(index, source, destination, stride);
        ReorderRowShift::set_offset(node, offset, axis);
    }

    /// Springs the mounted rows into the order the drop would produce. Only the
    /// range whose projection changed is touched.
    fn apply_live_shifts(&self, source: usize, boundary: usize) {
        let destination = ReorderableListModel::destination(source, boundary).unwrap_or(source);
        let previous = self.live_destination.get().unwrap_or(destination);
        if previous == destination {
            return;
        }
        self.live_destination.set(Some(destination));
        let stride = self.row_height.get() + self.row_spacing.get();
        let low = previous.min(destination).min(source);
        let high = previous.max(destination).max(source);
        let ids = self.ids.borrow();
        let nodes = self.row_nodes.borrow();
        for index in low..=high.min(ids.len().saturating_sub(1)) {
            let Some(node) = ids
                .get(index)
                .and_then(|id| nodes.get(id))
                .and_then(Node::upgrade)
            else {
                continue;
            };
            ReorderRowShift::slide_to(
                &node,
                ReorderableListModel::live_offset(index, source, destination, stride),
                self.axis.get(),
            );
        }
    }

    /// Hands every row's frame back to layout without animating. A new drag
    /// starts from the authored order and a completed drop moves every slot to
    /// where the row is already painted, so the offsets are stale either way.
    fn clear_live_shifts(&self) {
        let nodes = self.row_nodes.clone();
        for weak in nodes.borrow().values() {
            if let Some(node) = Node::upgrade(weak) {
                ReorderRowShift::clear(&node);
            }
        }
        self.live_destination.set(None);
    }

    /// Keeps the drag arithmetic on the same row metrics the rows were laid
    /// out with, including any future scaling or touch floor.
    fn set_row_metrics(&self, row_height: f32, row_spacing: f32) {
        self.row_height.set(row_height);
        self.row_spacing.set(row_spacing);
    }

    fn set_axis(&self, axis: ScrollAxis) {
        self.axis.set(axis);
        self.session.set_axis(axis);
    }

    /// Keyboard move: records the moved row so it stays mounted after the
    /// caller's reorder lands, and slides the displaced rows like a drop.
    pub fn move_row(&self, index: usize, delta: i64) {
        if !self.is_enabled {
            return;
        }
        let Some(destination) = ReorderableListModel::stepped(index, delta, self.count()) else {
            return;
        };
        let stride = self.row_height.get() + self.row_spacing.get();
        self.queue_shifts(index, destination, stride);
        self.retained.set(self.ids.borrow().get(index).cloned());
        let on_move = self.on_move.borrow().clone();
        on_move(index, destination);
    }

    /// Indices to mount: the visible window plus the dragged and last
    /// keyboard-moved rows.
    pub fn mounted_indices(&self) -> Vec<usize> {
        let mut retained = Vec::new();
        if let Some(id) = self.dragging.get()
            && let Some(index) = self.index_of(&id)
        {
            retained.push(index);
        }
        if let Some(id) = self.retained.get()
            && let Some(index) = self.index_of(&id)
            && !retained.contains(&index)
        {
            retained.push(index);
        }
        ReorderableListModel::mounted_indices(self.window.get(), self.count(), &retained)
    }

    pub fn preview_main(&self) -> f32 {
        self.session.preview_main()
    }
}

/// Keyboard-accessible reorderable list along a scroll axis.
pub struct ReorderableList<Item, Id, RowContent>
where
    Item: Clone + 'static,
    Id: CollectionId,
    RowContent: Fn(&Item, RowState) -> AnyView + 'static,
{
    pub data: Vec<Item>,
    /// Authored logical row extent along the main axis, before text scaling
    /// and the touch floor.
    pub row_height: f32,
    /// Authored logical row spacing, before text scaling.
    pub row_spacing: f32,
    pub is_enabled: bool,
    /// Scrolling and drag axis. `Both` reorders vertically.
    pub axis: ScrollAxis,
    pub row_label: Rc<dyn Fn(&Item) -> String>,
    pub row_content: RowContent,
    id_of: Rc<dyn Fn(&Item) -> Id>,
    session: Rc<ReorderSession<Id>>,
}

impl<Item, Id, RowContent> ReorderableList<Item, Id, RowContent>
where
    Item: Clone + 'static,
    Id: CollectionId,
    RowContent: Fn(&Item, RowState) -> AnyView + 'static,
{
    pub fn new(
        data: Vec<Item>,
        id_of: impl Fn(&Item) -> Id + 'static,
        row_content: RowContent,
    ) -> Self {
        let id_of = Rc::new(id_of);
        let ids: Vec<Id> = data.iter().map(|item| id_of(item)).collect();
        let session = Rc::new(ReorderSession {
            session: Rc::new(ReorderDragSession::new()),
            dragging: State::new(None),
            boundary: State::new(None),
            retained: State::new(None),
            window: State::new(0..24),
            width: State::new(0.0),
            height: State::new(0.0),
            scroll: State::new(0.0),
            ids: Rc::new(RefCell::new(ids)),
            on_move: Rc::new(RefCell::new(Rc::new(|_, _| {}))),
            is_enabled: true,
            row_height: Rc::new(Cell::new(36.0)),
            row_spacing: Rc::new(Cell::new(0.0)),
            axis: Rc::new(Cell::new(ScrollAxis::Vertical)),
            shifts: State::new(Vec::new()),
            settle: State::new(None),
            row_nodes: Rc::new(RefCell::new(HashMap::new())),
            live_destination: Rc::new(Cell::new(None)),
        });
        Self {
            data,
            row_height: 36.0,
            row_spacing: 0.0,
            is_enabled: true,
            axis: ScrollAxis::Vertical,
            row_label: Rc::new(|_| String::new()),
            row_content,
            id_of,
            session,
        }
    }
    /// Called with `(from, to)`, where `to` is the final index after removal.
    pub fn on_move(self, on_move: impl Fn(usize, usize) + 'static) -> Self {
        *self.session.on_move.borrow_mut() = Rc::new(on_move);
        self
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = row_height.max(1.0);
        self
    }

    pub fn row_spacing(mut self, row_spacing: f32) -> Self {
        self.row_spacing = row_spacing.max(0.0);
        self
    }

    pub fn is_enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// Reorders along `axis`; a horizontal list drags rows left/right with the
    /// same handle, live displacement, and settle behaviour. `Both` reorders
    /// vertically.
    pub fn axis(mut self, axis: ScrollAxis) -> Self {
        self.axis = axis;
        self
    }

    pub fn row_label(mut self, label: impl Fn(&Item) -> String + 'static) -> Self {
        self.row_label = Rc::new(label);
        self
    }

    /// The resolved row height: authored height after text scaling, raised to
    /// the minimum control extent on a touch surface.
    pub fn resolved_row_height(
        &self,
        scaled: f32,
        is_touch: bool,
        minimum_control_extent: f32,
    ) -> f32 {
        reorder_row_height(scaled, is_touch, minimum_control_extent)
    }

    /// Keyboard stepping from `index`, clamped rather than wrapped.
    pub fn step(&self, index: usize, delta: i64) -> Option<usize> {
        if !self.is_enabled {
            return None;
        }
        ReorderableListModel::stepped(index, delta, self.data.len())
    }

    pub fn is_dragging(&self) -> bool {
        self.session.is_dragging()
    }

    pub fn handle_label(&self, index: usize) -> String {
        let title = self.data.get(index).map(|item| (self.row_label)(item));
        ReorderableListModel::handle_label(title.as_deref(), index, self.data.len())
    }

    /// Publishes this build's projection into the shared session so the handle
    /// closures resolve indices against the same order the rows were built from.
    fn publish(&self) {
        let ids: Vec<Id> = self.data.iter().map(|item| (self.id_of)(item)).collect();
        *self.session.ids.borrow_mut() = ids;
    }
}

impl<Item, Id, RowContent> View for ReorderableList<Item, Id, RowContent>
where
    Item: Clone + 'static,
    Id: CollectionId,
    RowContent: Fn(&Item, RowState) -> AnyView + 'static,
{
    fn body(&self, _cx: &Scope) -> AnyView {
        self.publish();
        let axis = match self.axis {
            ScrollAxis::Horizontal => ScrollAxis::Horizontal,
            _ => ScrollAxis::Vertical,
        };
        let horizontal = axis == ScrollAxis::Horizontal;
        let row_extent = self.row_height;
        let row_spacing = self.row_spacing;
        let row_stride = row_extent + row_spacing;
        let count = self.data.len();
        let session = self.session.clone();
        session.set_row_metrics(row_extent, row_spacing);
        session.set_axis(axis);
        let dragging = session.dragging_id();
        if let Some(id) = &dragging
            && let Some(index) = session.index_of(id)
        {
            session.restore_drag(index as f32 * row_stride, row_extent, axis);
        }

        let mounted = mounted_window(&session, count);
        let rows: Vec<AnyView> = mounted
            .into_iter()
            .filter_map(|index| {
                let item = self.data.get(index)?.clone();
                let id = (self.id_of)(&item);
                let is_placeholder = dragging.as_ref() == Some(&id);
                let is_last = index + 1 == count;
                let label = self.handle_label(index);
                let content = (self.row_content)(
                    &item,
                    RowState {
                        // The placeholder is only a dimmed source row; only
                        // the lifted copy reads as the row in motion.
                        is_dragging: false,
                        ..RowState::default()
                    },
                );
                Some(AnyView::new(ReorderRowSlot {
                    index,
                    row_stride,
                    row_extent,
                    axis,
                    is_placeholder,
                    is_last,
                    id,
                    session: session.clone(),
                    handle: ReorderHandle {
                        index,
                        count,
                        label,
                        is_enabled: self.is_enabled,
                        is_dragging: is_placeholder,
                        row_extent,
                        axis,
                        session: session.clone(),
                    },
                    content,
                }))
            })
            .collect();

        let content_extent = (count as f32 * row_stride - row_spacing).max(0.0);
        let cross = if horizontal {
            session.height.get()
        } else {
            session.width.get()
        };
        let direction = if horizontal {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        };

        let column = AnyView::new(
            AnyView::new(BoxView::new(
                direction,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(rows)),
            ))
            .frame(
                Some(crate::modifiers::layout::FrameDimension::Points(
                    if horizontal {
                        content_extent.max(1.0)
                    } else {
                        cross.max(1.0)
                    },
                )),
                Some(crate::modifiers::layout::FrameDimension::Points(
                    if horizontal {
                        cross.max(1.0)
                    } else {
                        content_extent.max(1.0)
                    },
                )),
                None,
                None,
                None,
                None,
            ),
        );

        // The drag displaces the rows themselves, so there is nothing to draw
        // over the list: the projected order is the affordance.

        let scroll_session = session.clone();
        let scroll_axis = axis;
        let scroll = ScrollView::new(
            scroll_axis,
            // A flex parent rather than a `ZStack`: only flex layout stretches
            // the absolutely positioned row slots across the content.
            AnyView::new(BoxView::new(
                direction,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                column,
            )),
        )
        .scrollbar_gutter(ScrollbarGutter::Stable)
        .on_viewport_change(move |viewport: ScrollViewport| {
            let drag = &scroll_session.session;
            let offset_main = main_of(scroll_axis, viewport.offset.x, viewport.offset.y);
            let extent_main = if scroll_axis == ScrollAxis::Horizontal {
                viewport.size.width
            } else {
                viewport.size.height
            };
            drag.set_scroll_main(offset_main);
            drag.set_viewport_extent(extent_main);
            if (offset_main - scroll_session.scroll.get()).abs() > 0.5 {
                scroll_session.scroll.set(offset_main);
            }
            if (viewport.size.width - scroll_session.width.get()).abs() > 0.5 {
                scroll_session.width.set(viewport.size.width);
            }
            if (viewport.size.height - scroll_session.height.get()).abs() > 0.5 {
                scroll_session.height.set(viewport.size.height);
            }
            let visible = visible_range(
                scroll_session.count(),
                (scroll_session.row_height.get() + scroll_session.row_spacing.get()).max(1.0),
                offset_main,
                extent_main,
                4,
            );
            if scroll_session.window.get() != visible {
                scroll_session.window.set(visible);
            }
        });

        let mut surface = vec![AnyView::new(AnyView::new(scroll).flex_with(
            1.0,
            1.0,
            Some(0.0),
        ))];
        if let Some(dragging) = dragging
            && let Some(item) = self.data.iter().find(|item| (self.id_of)(item) == dragging)
        {
            let preview = (self.row_content)(
                item,
                RowState {
                    is_dragging: true,
                    ..RowState::default()
                },
            );
            surface.push(AnyView::new(
                AnyView::new(ReorderPreviewHost {
                    session: session.clone(),
                    row_extent,
                    cross,
                    axis,
                    content: preview,
                })
                .with_modifier(AbsolutePositionModifier::new(
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                ))
                .allows_hit_testing(false),
            ));
        }

        let column = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(surface)),
            ))
            .flex_with(1.0, 1.0, Some(0.0)),
        );
        // A vertical list sits in the same inset box as `List`; a horizontal
        // strip of cards keeps its own surfaces.
        if axis == ScrollAxis::Vertical {
            AnyView::new(column.with_modifier(super::list::InsetListBox))
        } else {
            column
        }
    }

    /// Swift keeps the drag, viewport and mount window in `@State`; declaring
    /// them carries the values over when the parent rebuilds and repaints the
    /// list when the viewport reports a new width or window.
    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        let session = self.session.clone();
        vec![
            crate::DynamicField::state("dragging", &session.dragging),
            crate::DynamicField::state("boundary", &session.boundary),
            crate::DynamicField::state("retained", &session.retained),
            crate::DynamicField::state("window", &session.window),
            crate::DynamicField::state("width", &session.width),
            crate::DynamicField::state("height", &session.height),
            crate::DynamicField::state("scroll", &session.scroll),
            crate::DynamicField::state("shifts", &session.shifts),
            crate::DynamicField::state("settle", &session.settle),
        ]
    }
}

fn main_of(axis: ScrollAxis, x: f32, y: f32) -> f32 {
    match axis {
        ScrollAxis::Horizontal => x,
        _ => y,
    }
}

fn mounted_window<Id: CollectionId>(session: &Rc<ReorderSession<Id>>, count: usize) -> Vec<usize> {
    let mut retained = Vec::new();
    if let Some(id) = session.dragging.get()
        && let Some(index) = session.index_of(&id)
    {
        retained.push(index);
    }
    if let Some(id) = session.retained.get()
        && let Some(index) = session.index_of(&id)
        && !retained.contains(&index)
    {
        retained.push(index);
    }
    ReorderableListModel::mounted_indices(session.window.get(), count, &retained)
}

/// One row: content plus its drag handle, absolutely positioned so offscreen
/// rows take no layout space.
struct ReorderRowSlot<Id: CollectionId> {
    index: usize,
    row_stride: f32,
    row_extent: f32,
    axis: ScrollAxis,
    is_placeholder: bool,
    is_last: bool,
    id: Id,
    session: Rc<ReorderSession<Id>>,
    handle: ReorderHandle<Id>,
    content: AnyView,
}

impl<Id: CollectionId> PrimitiveView for ReorderRowSlot<Id> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        use cove_core::scene::{Edge, PositionType};
        layout.set_position_type(PositionType::Absolute);
        if self.axis == ScrollAxis::Horizontal {
            layout.set_position(0.0, Edge::Top);
            layout.set_position(self.index as f32 * self.row_stride, Edge::Left);
            layout.set_position(0.0, Edge::Bottom);
            layout.set_width(Some(self.row_extent));
        } else {
            layout.set_position(0.0, Edge::Left);
            layout.set_position(self.index as f32 * self.row_stride, Edge::Top);
            layout.set_position(0.0, Edge::Right);
            layout.set_height(Some(self.row_extent));
        }
    }

    fn update_node(&self, node: &Node) {
        node.set_corner_radius(6.0);
        self.session.bind_row_node(&self.id, node);
        if self.is_placeholder {
            // Dim once and bind for the settle crossfade; the landing raises
            // the opacity itself, so a rebuild mid-settle must not clip it
            // back to the starting value.
            self.session.session.bind_placeholder(node);
            if node.attachment::<bool>(ROW_PLACEHOLDING) != Some(true) {
                node.set_attachment(ROW_PLACEHOLDING, true);
                node.set_opacity(0.35);
            }
        } else {
            node.remove_attachment(ROW_PLACEHOLDING);
            node.set_opacity(1.0);
        }
        // A displaced row slides from its old slot into the new one.
        if let Some(offset) = self.session.take_shift(&self.id) {
            ReorderRowShift::begin_for(node, offset, self.axis);
        } else {
            self.session.restore_live_shift(&self.id, node, self.axis);
        }
        node.set_is_hit_testable(false);
        // A hairline between rows gives the list the same grouped rhythm as
        // `List`; the lifted placeholder keeps a clean edge while it moves.
        if !self.is_placeholder && !self.is_last {
            let weak = node.downgrade();
            let horizontal = self.axis == ScrollAxis::Horizontal;
            node.update_draw(false, move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                let frame = owner.frame();
                let color = theme_for_node(&owner).colors.divider;
                let rect = if horizontal {
                    cove_core::Rect::new(
                        origin.x + frame.width - 0.5,
                        origin.y + 12.0,
                        0.5,
                        (frame.height - 12.0).max(0.0),
                    )
                } else {
                    cove_core::Rect::new(
                        origin.x + 12.0,
                        origin.y + frame.height - 0.5,
                        (frame.width - 12.0).max(0.0),
                        0.5,
                    )
                };
                let _ = list.add_rect(rect, color);
            });
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let inner = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            4.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    AnyView::new(self.content.clone())
                        .flex_with(1.0, 1.0, Some(0.0))
                        .clipped(),
                ),
                AnyView::new(self.handle.clone()),
            ])),
        ));
        let inner = if self.axis == ScrollAxis::Horizontal {
            inner.frame_width(self.row_extent)
        } else {
            inner.frame_height(self.row_extent)
        };
        vec![AnyView::new(AnyView::new(inner).clipped())]
    }
}

impl<Id: CollectionId> View for ReorderRowSlot<Id> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Drag/focus handle behind one reorderable row. Interaction state lives on the
/// node so a parent recompose mid-drag keeps the pointer transaction.
#[derive(Clone)]
struct ReorderHandle<Id: CollectionId> {
    index: usize,
    count: usize,
    label: String,
    is_enabled: bool,
    is_dragging: bool,
    row_extent: f32,
    axis: ScrollAxis,
    session: Rc<ReorderSession<Id>>,
}

impl<Id: CollectionId> PrimitiveView for ReorderHandle<Id> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(44.0));
        layout.set_height(Some(self.row_extent));
        layout.set_flex_shrink(0.0);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let session = self.session.clone();
        let index = self.index;
        let count = self.count;
        let is_enabled = self.is_enabled;
        let axis = self.axis;

        // A touch surface reserves a finger-sized target on both axes, so the
        // 44pt-wide grip also grows to 44pt tall.
        apply_touch_control_extent(node, self.is_enabled);
        node.set_corner_radius(6.0);
        node.set_is_focusable(self.is_enabled);
        node.set_cursor(if self.is_enabled {
            Some(SystemCursor::Pointer)
        } else {
            None
        });
        node.set_attachment(
            cove_composition::LayoutDebugAttachmentKey::DEBUG_NAME,
            format!("reorder-handle-{index}"),
        );
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: self.label.clone(),
            value: format!("Position {} of {count}", index + 1),
            help: crate::i18n::tr("Drag the handle, or use Up and Down to move this row."),
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.is_enabled,
                is_selected: self.is_dragging,
                ..Default::default()
            },
            ..Default::default()
        }));

        if self.is_enabled {
            let up = session.clone();
            let down = session.clone();
            node.set_accessibility_actions(AccessibilityActions {
                increment: Some(Rc::new(move || up.move_row(index, 1))),
                decrement: Some(Rc::new(move || down.move_row(index, -1))),
                ..Default::default()
            });
        }

        let paint_node = node.clone();
        let paint: Rc<dyn Fn()> = Rc::new(move || {
            let active = paint_node.attachment::<bool>(HANDLE_ACTIVE) == Some(true);
            let hovered = paint_node.attachment::<bool>(HANDLE_HOVERED) == Some(true);
            let theme = theme_for_node(&paint_node);
            let color = if active {
                Some(theme.colors.state_layer_pressed)
            } else if hovered {
                Some(theme.colors.state_layer_hover)
            } else {
                Some(cove_core::Color::CLEAR)
            };
            paint_node.set_background_color(color);
        });
        paint();

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };

        let hover_node = node.downgrade();
        let hover_paint = paint.clone();
        registry.set_hover(
            node,
            Rc::new(move |phase: HoverPhase| {
                if let Some(owner) = Node::upgrade(&hover_node) {
                    owner.set_attachment(HANDLE_HOVERED, phase == HoverPhase::Enter && is_enabled);
                    hover_paint();
                }
            }),
        );

        // Recomposition reruns `update_node`; keep one resource so the drag
        // that started on it also receives the pointer updates and ticks.
        let auto_scroll = node.first_resource::<EdgeAutoScroll>().unwrap_or_else(|| {
            let auto_scroll = EdgeAutoScroll::new();
            node.add_resource(auto_scroll.clone());
            auto_scroll
        });

        let pointer_session = session.clone();
        let pointer_node = node.clone();
        let pointer_paint = paint.clone();
        let pointer_scroll = auto_scroll.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(
                move |event: &cove_platform::MouseButtonEvent,
                      phase: PointerPhase,
                      delivery: EventPhase| {
                    if delivery != EventPhase::Target
                        || !is_enabled
                        || event.button != MouseButton::Left
                    {
                        return EventResult::Ignored;
                    }
                    match phase {
                        PointerPhase::Down => {
                            let main = main_of(axis, event.x, event.y);
                            pointer_node.set_attachment(HANDLE_ACTIVE, true);
                            pointer_node.set_attachment(HANDLE_START_Y, main);
                            pointer_node.set_attachment(HANDLE_LAST_Y, main);
                            let report_node = pointer_node.downgrade();
                            let report_session = pointer_session.clone();
                            let report_scroll = pointer_scroll.clone();
                            pointer_scroll.begin(
                                cove_core::Point::new(event.x, event.y),
                                axis,
                                Rc::new(move || {
                                    if let Some(owner) = Node::upgrade(&report_node)
                                        && let Some(start) = owner.attachment::<f32>(HANDLE_START_Y)
                                        && let Some(last) = owner.attachment::<f32>(HANDLE_LAST_Y)
                                    {
                                        report_session.update_drag(
                                            last - start,
                                            report_scroll.scrolled_distance(),
                                        );
                                    }
                                }),
                            );
                            if let Some(capture) = PointerCaptureHolder::current() {
                                capture.acquire(&pointer_node, false);
                            }
                            pointer_paint();
                            pointer_session.begin_drag(index);
                            EventResult::Handled
                        }
                        PointerPhase::Up => {
                            if pointer_node.attachment::<bool>(HANDLE_ACTIVE) != Some(true) {
                                return EventResult::Ignored;
                            }
                            pointer_node.set_attachment(HANDLE_ACTIVE, false);
                            pointer_node.remove_attachment(HANDLE_START_Y);
                            pointer_scroll.end();
                            PointerCaptureHolder::release_current();
                            pointer_paint();
                            pointer_session.finish_drag();
                            EventResult::Handled
                        }
                    }
                },
            ),
        );

        let motion_session = session.clone();
        let motion_node = node.clone();
        let motion_scroll = auto_scroll.clone();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(
                move |event: &cove_platform::MouseMotionEvent, _phase: EventPhase| {
                    if motion_node.attachment::<bool>(HANDLE_ACTIVE) != Some(true) {
                        return EventResult::Ignored;
                    }
                    let main = main_of(axis, event.x, event.y);
                    motion_node.set_attachment(HANDLE_LAST_Y, main);
                    motion_scroll.update(cove_core::Point::new(event.x, event.y));
                    if let Some(start) = motion_node.attachment::<f32>(HANDLE_START_Y) {
                        motion_session.update_drag(main - start, motion_scroll.scrolled_distance());
                    }
                    EventResult::Handled
                },
            ),
        );

        let key_session = session.clone();
        let key_node = node.downgrade();
        let key_scroll = auto_scroll.clone();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event: &KeyEvent, phase: EventPhase| {
                if phase == EventPhase::Capture || !is_enabled {
                    return EventResult::Ignored;
                }
                match event.scancode {
                    Scancode::ARROW_UP | Scancode::ARROW_LEFT => {
                        key_session.move_row(index, -1);
                        EventResult::Handled
                    }
                    Scancode::ARROW_DOWN | Scancode::ARROW_RIGHT => {
                        key_session.move_row(index, 1);
                        EventResult::Handled
                    }
                    Scancode::ESCAPE => {
                        let Some(owner) = Node::upgrade(&key_node) else {
                            return EventResult::Ignored;
                        };
                        if owner.attachment::<bool>(HANDLE_ACTIVE) != Some(true) {
                            return EventResult::Ignored;
                        }
                        owner.set_attachment(HANDLE_ACTIVE, false);
                        owner.remove_attachment(HANDLE_START_Y);
                        key_scroll.end();
                        PointerCaptureHolder::release_current();
                        key_session.cancel_drag();
                        EventResult::Handled
                    }
                    _ => EventResult::Ignored,
                }
            }),
        );

        let clear_node = node.downgrade();
        node.set_attachment(
            PointerCaptureAttachmentKey::DID_LOSE_CAPTURE,
            Rc::new(move || {
                if let Some(owner) = Node::upgrade(&clear_node) {
                    owner.remove_attachment(HANDLE_START_Y);
                    owner.set_attachment(HANDLE_ACTIVE, false);
                }
            }) as Rc<dyn Fn()>,
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            AnyView::new(Icon::with_semantic(
                ui_icon_catalog::action::grip(),
                16.0,
                crate::theme::SemanticColorRef::on_surface_muted(),
            ))
            .accessibility_hidden(true),
        )]
    }
}

impl<Id: CollectionId> View for ReorderHandle<Id> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Covers the list viewport and paints the lifted row at the session's
/// position through its content offset, so following the pointer neither
/// relayouts nor recomposes. It never takes input.
struct ReorderPreviewHost<Id: CollectionId> {
    session: Rc<ReorderSession<Id>>,
    row_extent: f32,
    cross: f32,
    axis: ScrollAxis,
    content: AnyView,
}

impl<Id: CollectionId> PrimitiveView for ReorderPreviewHost<Id> {
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
        self.session.session.bind_preview(node);
        self.session.resume_landing();
        node.set_clips_to_bounds(true);
        node.set_is_hit_testable(false);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let main = self.row_extent;
        let cross = self.cross.max(1.0);
        let (frame_width, frame_height) = if self.axis == ScrollAxis::Horizontal {
            (main, cross)
        } else {
            (cross, main)
        };
        vec![AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        AnyView::new(self.content.clone())
                            .flex_with(1.0, 1.0, Some(0.0))
                            .clipped(),
                    ),
                    AnyView::new(
                        AnyView::new(Icon::with_semantic(
                            ui_icon_catalog::action::grip(),
                            16.0,
                            crate::theme::SemanticColorRef::on_surface_muted(),
                        ))
                        .frame_size(44.0, main),
                    ),
                ])),
            ))
            .frame(
                Some(crate::modifiers::layout::FrameDimension::Points(
                    frame_width.max(1.0),
                )),
                Some(crate::modifiers::layout::FrameDimension::Points(
                    frame_height.max(1.0),
                )),
                None,
                None,
                None,
                None,
            )
            .semantic_background(crate::theme::SemanticColorRef::surface())
            .corner_radius(8.0)
            .semantic_border(crate::theme::SemanticColorRef::border(), 1.0)
            .shadow(cove_core::Color::from_rgba8(0, 0, 0, 56), 0.0, 8.0, 24.0)
            .modifier(cove_composition::LayoutDebugModifier::debug_name(
                "reorder-preview",
            )),
        )]
    }
}

impl<Id: CollectionId> View for ReorderPreviewHost<Id> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
