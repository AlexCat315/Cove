//! Translation of `Primitives/Data/Lists/Reorder/{Preview,Reorderable}.swift`
//! (the model half).
//!
//! Transient drag geometry shared by the handle, the list and the floating
//! preview, plus the index arithmetic that turns a drag distance into a
//! destination. Pointer moves update the session in place; only starting and
//! ending a drag recompose the list. All geometry is expressed along the
//! list's main (scrolling) axis so vertical and horizontal lists share it.
//!
//! Behaviour checklist:
//! - the preview follows the pointer but never leaves the viewport
//! - the drop boundary is the source row shifted by whole row strides, clamped
//! - finishing converts the boundary to a destination index after removal
//! - a drop on the row's own position reports no move
//! - dropping springs the lifted preview onto its slot while the displaced
//!   rows slide to their new positions; reduced motion snaps instead
//! - keyboard stepping clamps instead of wrapping
//! - mounted rows always include the dragged and last-moved rows

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

use cove_core::scene::{
    AnyAnimationController, AnimatorScheduler, LayoutUpdateHook, Node, NodeResource, NodeWeak,
    SpringSolution,
};
use cove_core::{Point, Rect};

use crate::primitives::layout::ScrollAxis;

/// The settle spring for the lifted row and the displaced rows. Shared so a
/// drop reads as one motion instead of two clocks.
fn reorder_spring() -> SpringSolution {
    SpringSolution::new(0.3, 0.82)
}

/// Attachment naming the session that owns the landing painted on a preview
/// node. A parent rebuild replaces the session value; the session that last
/// bound the node keeps drawing it, and the orphaned one retires itself.
const SETTLE_TOKEN: &str = "__reorder.settle.token";

static NEXT_SETTLE_TOKEN: AtomicU64 = AtomicU64::new(1);

/// Live drag geometry for one reorder, in main-axis coordinates.
///
/// Deliberately a shared mutable session rather than per-frame state: moving
/// the preview must not recompose the list.
pub struct ReorderDragSession {
    /// Main (scrolling) axis of the list.
    axis: Cell<ScrollAxis>,
    /// Latest scroll offset of the list viewport along the main axis.
    scroll_main: Cell<f32>,
    /// Latest viewport extent along the main axis.
    viewport_extent: Cell<f32>,
    /// Dragged row's leading edge in viewport coordinates when the drag began.
    start_main: Cell<f32>,
    row_extent: Cell<f32>,
    preview_main: Cell<f32>,
    /// Node whose content offset carries the lifted row; weak in Swift, kept
    /// alive here by the preview host while it is mounted.
    preview: ReorderPreviewOffset,
    /// Whether a drag is in progress.
    active: Cell<bool>,
    /// Bumped on every `begin`; lets a settle completion tell whether the drag
    /// it belongs to is still the live one.
    epoch: Cell<u64>,
    /// Drop-landing / cancel-return spring state for the lifted preview.
    settling: Cell<bool>,
    release_main: Cell<f32>,
    settle_target: Cell<f32>,
    settle_elapsed: Cell<f64>,
    on_settled: RefCell<Option<Rc<dyn Fn()>>>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    /// The dimmed placeholder the preview is landing onto; its opacity blends
    /// over the settle so the handoff is a crossfade rather than a pop.
    placeholder: RefCell<Option<NodeWeak>>,
    reduced: Cell<bool>,
    /// Non-zero while this session owns the landing on the bound preview node.
    owner_token: Cell<u64>,
    self_ref: RefCell<Weak<Self>>,
}

impl Default for ReorderDragSession {
    fn default() -> Self {
        Self {
            axis: Cell::new(ScrollAxis::Vertical),
            scroll_main: Cell::new(0.0),
            viewport_extent: Cell::new(0.0),
            start_main: Cell::new(0.0),
            row_extent: Cell::new(0.0),
            preview_main: Cell::new(0.0),
            preview: ReorderPreviewOffset::new(),
            active: Cell::new(false),
            epoch: Cell::new(0),
            settling: Cell::new(false),
            release_main: Cell::new(0.0),
            settle_target: Cell::new(0.0),
            settle_elapsed: Cell::new(0.0),
            on_settled: RefCell::new(None),
            scheduler: RefCell::new(None),
            controller: RefCell::new(None),
            placeholder: RefCell::new(None),
            reduced: Cell::new(false),
            owner_token: Cell::new(0),
            self_ref: RefCell::new(Weak::new()),
        }
    }
}

/// Forwards the animator ticks to the session without keeping it alive.
struct ReorderSettleController {
    session: Weak<ReorderDragSession>,
}

impl AnyAnimationController for ReorderSettleController {
    fn is_finished(&self) -> bool {
        self.session
            .upgrade()
            .is_none_or(|session| !session.settling.get())
    }

    fn tick(&self, delta_time: f64) {
        if let Some(session) = self.session.upgrade() {
            session.tick(delta_time);
        }
    }

    fn finish_immediately(&self) {
        if let Some(session) = self.session.upgrade() {
            session.snap_settle();
        }
    }

    fn cancel(&self) {
        if let Some(session) = self.session.upgrade() {
            session.snap_settle();
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ReorderDragSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// The `Rc` form the settle controller can weakly reference.
    pub fn new_rc() -> Rc<Self> {
        let session = Rc::new(Self::default());
        *session.self_ref.borrow_mut() = Rc::downgrade(&session);
        session
    }

    pub fn set_axis(&self, axis: ScrollAxis) {
        self.axis.set(axis);
        self.preview.set_axis(axis);
    }

    pub fn axis(&self) -> ScrollAxis {
        self.axis.get()
    }

    pub fn set_scroll_main(&self, value: f32) {
        self.scroll_main.set(value);
    }

    pub fn scroll_main(&self) -> f32 {
        self.scroll_main.get()
    }

    pub fn set_viewport_extent(&self, value: f32) {
        self.viewport_extent.set(value);
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.get()
    }

    pub fn is_settling(&self) -> bool {
        self.settling.get()
    }

    /// Where the lifted row is drawn, in viewport coordinates along the axis.
    pub fn preview_main(&self) -> f32 {
        self.preview_main.get()
    }

    /// Starts a drag with the row's leading edge in *content* coordinates.
    pub fn begin(&self, row_main_in_content: f32, row_extent: f32) {
        self.cancel_settle();
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.row_extent.set(row_extent);
        self.start_main.set(row_main_in_content - self.scroll_main.get());
        self.active.set(true);
        *self.placeholder.borrow_mut() = None;
        self.move_by(0.0);
    }

    /// Restores the drag geometry a rebuild dropped, without restarting the
    /// drag or discarding a landing already in flight.
    pub fn ensure_begun(&self, row_main_in_content: f32, row_extent: f32) {
        if self.active.get() {
            return;
        }
        self.row_extent.set(row_extent);
        self.start_main
            .set(row_main_in_content - self.scroll_main.get());
        self.active.set(true);
        self.preview.apply(self.preview_main.get());
    }

    /// Binds the node that paints the lifted row, then positions it at the
    /// current drag offset so a rebuild mid-drag does not jump.
    pub fn bind_preview(&self, node: &cove_core::scene::Node) {
        self.preview.bind(node);
        self.preview.apply(self.preview_main.get());
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
        self.reduced.set(node.prefers_reduced_motion());
        self.preview.set_token(self.owner_token.get());
    }

    /// Binds the dimmed source row so the settle can blend its opacity.
    pub fn bind_placeholder(&self, node: &cove_core::scene::Node) {
        *self.placeholder.borrow_mut() = Some(node.downgrade());
    }

    /// Follows the pointer, staying inside the viewport so the preview rests at
    /// an edge while automatic scrolling moves the content beneath it.
    pub fn move_by(&self, travel: f32) {
        let row_extent = self.row_extent.get();
        let ceiling = (self.viewport_extent.get() - row_extent).max(0.0);
        let main = (self.start_main.get() + travel).clamp(0.0, ceiling);
        self.preview_main.set(main);
        self.preview.apply(main);
    }

    /// Springs the lifted preview onto `target_main` (viewport coordinates)
    /// and runs `on_finished` once it lands. Reduced motion lands and reports
    /// synchronously. Returns whether a spring is actually running.
    pub fn settle_to(self: &Rc<Self>, target_main: f32, on_finished: Rc<dyn Fn()>) -> bool {
        self.start_landing(self.preview_main.get(), target_main, on_finished)
    }

    /// Re-arms a landing whose session was replaced by a rebuild. `release`
    /// and `target` are the values the previous session was animating between.
    pub fn resume_settle(
        self: &Rc<Self>,
        release_main: f32,
        target_main: f32,
        on_finished: Rc<dyn Fn()>,
    ) -> bool {
        if self.settling.get() {
            return true;
        }
        self.start_landing(release_main, target_main, on_finished)
    }

    fn start_landing(
        self: &Rc<Self>,
        release_main: f32,
        target_main: f32,
        on_finished: Rc<dyn Fn()>,
    ) -> bool {
        let ceiling = (self.viewport_extent.get() - self.row_extent.get()).max(0.0);
        let target = target_main.clamp(0.0, ceiling);
        if self.reduced.get() || self.scheduler.borrow().is_none() {
            self.preview_main.set(target);
            self.preview.apply(target);
            on_finished();
            return false;
        }
        let release = release_main.clamp(0.0, ceiling);
        self.settling.set(true);
        self.release_main.set(release);
        self.settle_target.set(target);
        self.settle_elapsed.set(0.0);
        self.owner_token
            .set(NEXT_SETTLE_TOKEN.fetch_add(1, Ordering::Relaxed));
        *self.on_settled.borrow_mut() = Some(on_finished);
        let controller: Rc<dyn AnyAnimationController> = Rc::new(ReorderSettleController {
            session: Rc::downgrade(self),
        });
        *self.controller.borrow_mut() = Some(controller.clone());
        if let Some(scheduler) = self.scheduler.borrow().as_ref() {
            scheduler.register(controller);
        }
        self.preview_main.set(release);
        self.preview.apply(release);
        self.preview.set_token(self.owner_token.get());
        true
    }

    fn tick(&self, delta_time: f64) {
        if !self.settling.get() || !delta_time.is_finite() || delta_time < 0.0 {
            return;
        }
        // Only the session that last bound the preview node may move it; an
        // orphaned one from a replaced view value retires without reporting.
        if self
            .preview
            .token()
            .is_some_and(|token| token != self.owner_token.get())
        {
            self.cancel_settle();
            return;
        }
        self.settle_elapsed.set(self.settle_elapsed.get() + delta_time);
        let spring = reorder_spring();
        let duration = spring.settling_duration();
        if self.settle_elapsed.get() >= duration {
            self.snap_settle();
            return;
        }
        let t = self.settle_elapsed.get() as f32;
        let ceiling = (self.viewport_extent.get() - self.row_extent.get()).max(0.0);
        let main = spring
            .position(t, self.release_main.get(), 0.0, self.settle_target.get())
            .clamp(0.0, ceiling);
        self.preview_main.set(main);
        self.preview.apply(main);
        // Blend the placeholder up to full opacity as the preview nears it.
        let progress = (t / duration as f32).clamp(0.0, 1.0);
        if let Some(node) = self
            .placeholder
            .borrow()
            .as_ref()
            .and_then(Node::upgrade)
        {
            node.set_opacity(0.35 + 0.65 * progress);
        }
    }

    /// Lands immediately, reports completion, and releases the controller.
    fn snap_settle(&self) {
        if !self.settling.get() {
            return;
        }
        self.settling.set(false);
        let target = self.settle_target.get();
        self.preview_main.set(target);
        self.preview.apply(target);
        if let Some(node) = self
            .placeholder
            .borrow()
            .as_ref()
            .and_then(Node::upgrade)
        {
            node.set_opacity(1.0);
        }
        self.unregister_controller();
        let on_settled = self.on_settled.borrow_mut().take();
        if let Some(on_settled) = on_settled {
            on_settled();
        }
    }

    fn cancel_settle(&self) {
        self.settling.set(false);
        self.on_settled.borrow_mut().take();
        self.unregister_controller();
    }

    fn unregister_controller(&self) {
        if let (Some(scheduler), Some(controller)) = (
            self.scheduler.borrow().as_ref(),
            self.controller.borrow().as_ref(),
        ) {
            scheduler.unregister(controller);
        }
        self.controller.borrow_mut().take();
    }

    pub fn end(&self) {
        self.cancel_settle();
        self.active.set(false);
        self.preview_main.set(0.0);
        self.row_extent.set(0.0);
        self.start_main.set(0.0);
        self.preview.apply(0.0);
        *self.placeholder.borrow_mut() = None;
    }
}

/// The column-style reorder rules for a vertical list.
pub enum ReorderableListModel {}

impl ReorderableListModel {
    /// The drop boundary for `source` dragged by `delta` points.
    ///
    /// `travel` is pointer movement and `scrolled` is automatic scrolling since
    /// the drag began; their sum is the displacement in content coordinates.
    pub fn drop_boundary(
        source: usize,
        travel: f32,
        scrolled: f32,
        row_stride: f32,
        count: usize,
    ) -> usize {
        let stride = row_stride.max(1.0);
        let offset = ((travel + scrolled) / stride).round() as i64;
        let boundary = source as i64 + offset;
        boundary.clamp(0, count as i64) as usize
    }

    /// The final index after the row is removed, or `None` when the row would
    /// stay where it is.
    ///
    /// A boundary past the source shifts back by one because removing the row
    /// closes the gap it left.
    pub fn destination(source: usize, boundary: usize) -> Option<usize> {
        let destination = if boundary > source {
            boundary - 1
        } else {
            boundary
        };
        (destination != source).then_some(destination)
    }

    /// The boundary a drop indicator should draw at, or `None` when the drop
    /// would be a no-op (the indicator must not appear in that case).
    pub fn visible_boundary(source: usize, boundary: usize) -> Option<usize> {
        Self::destination(source, boundary).map(|_| boundary)
    }

    /// Keyboard stepping clamps at both ends rather than wrapping.
    pub fn stepped(index: usize, delta: i64, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }
        let last = count - 1;
        let destination = (index as i64 + delta).clamp(0, last as i64) as usize;
        (destination != index).then_some(destination)
    }

    /// Rows to keep mounted: the visible window plus the dragged and last
    /// keyboard-moved rows, so pointer capture and focus survive auto-scrolling.
    pub fn mounted_indices(
        visible: std::ops::Range<usize>,
        count: usize,
        retained: &[usize],
    ) -> Vec<usize> {
        let start = visible.start.min(count);
        let end = visible.end.min(count);
        let mut indices: Vec<usize> = (start..end).collect();
        for index in retained {
            if *index < count && !indices.contains(index) {
                indices.push(*index);
            }
        }
        indices
    }

    /// The accessible label for a handle, matching the authored-title form when
    /// the caller supplied one.
    pub fn handle_label(title: Option<&str>, index: usize, count: usize) -> String {
        match title.filter(|value| !value.is_empty()) {
            Some(title) => format!("Reorder {title}, position {} of {count}", index + 1),
            None => format!("Reorder item {} of {count}", index + 1),
        }
    }
}

/// Row height: the authored height after text scaling, raised to the minimum
/// control extent on a touch surface so the row is at least a finger target.
pub fn reorder_row_height(scaled: f32, is_touch: bool, minimum_control_extent: f32) -> f32 {
    // `max(scaler.scale(rowHeight), isTouch ? minimumControlExtent : 0)`.
    let floor = if is_touch {
        minimum_control_extent
    } else {
        0.0
    };
    scaled.max(floor)
}

/// The painted y of the drop indicator for `boundary`.
pub fn drop_indicator_y(
    boundary: usize,
    count: usize,
    row_stride: f32,
    content_height: f32,
) -> f32 {
    if boundary >= count {
        (content_height - 2.0).max(0.0)
    } else {
        boundary as f32 * row_stride - 1.0
    }
}

/// Shared handle for the preview node's scroll offset, so following the pointer
/// neither relayouts nor recomposes.
#[derive(Clone)]
pub struct ReorderPreviewOffset {
    node: Rc<RefCell<Option<NodeWeak>>>,
    axis: Cell<ScrollAxis>,
}

impl Default for ReorderPreviewOffset {
    fn default() -> Self {
        Self {
            node: Rc::new(RefCell::new(None)),
            axis: Cell::new(ScrollAxis::Vertical),
        }
    }
}

impl ReorderPreviewOffset {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_axis(&self, axis: ScrollAxis) {
        self.axis.set(axis);
    }

    pub fn bind(&self, node: &Node) {
        *self.node.borrow_mut() = Some(node.downgrade());
    }

    /// Applies `preview_main` through the node's content offset, along the
    /// list's main axis.
    pub fn apply(&self, preview_main: f32) {
        let node = self.node.borrow().as_ref().and_then(Node::upgrade);
        if let Some(node) = node {
            let offset = match self.axis.get() {
                ScrollAxis::Horizontal => Point {
                    x: -preview_main,
                    y: 0.0,
                },
                _ => Point {
                    x: 0.0,
                    y: -preview_main,
                },
            };
            node.set_content_offset(offset);
        }
    }

    /// Tags the bound node with the session that owns its landing.
    pub fn set_token(&self, token: u64) {
        if let Some(node) = self.node.borrow().as_ref().and_then(Node::upgrade) {
            node.set_attachment(SETTLE_TOKEN, token);
        }
    }

    /// The token written on the bound node, if any.
    pub fn token(&self) -> Option<u64> {
        self.node
            .borrow()
            .as_ref()
            .and_then(Node::upgrade)
            .and_then(|node| node.attachment::<u64>(SETTLE_TOKEN))
    }
}

/// The slide a displaced row plays when the dragged row lands in its slot.
///
/// Rows sit at a fixed slot (`index * stride`) and only their content moves,
/// so a shifted row starts painted one stride away from its slot and springs
/// home. Reduced motion never animates and the resource is a no-op.
pub struct ReorderRowShift {
    offset: Cell<f32>,
    axis: Cell<ScrollAxis>,
    node: RefCell<Option<NodeWeak>>,
    origin: Cell<Rect>,
    release: Cell<f32>,
    elapsed: Cell<f64>,
    running: Cell<bool>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
    self_ref: RefCell<Weak<ReorderRowShift>>,
}

impl ReorderRowShift {
    fn new() -> Rc<Self> {
        let shift = Rc::new(Self {
            offset: Cell::new(0.0),
            axis: Cell::new(ScrollAxis::Vertical),
            node: RefCell::new(None),
            origin: Cell::new(Rect::default()),
            release: Cell::new(0.0),
            elapsed: Cell::new(0.0),
            running: Cell::new(false),
            scheduler: RefCell::new(None),
            controller: RefCell::new(None),
            self_ref: RefCell::new(Weak::new()),
        });
        *shift.self_ref.borrow_mut() = Rc::downgrade(&shift);
        shift
    }

    /// Starts a slide from `offset` (the distance the row travels) to zero.
    /// Zero offset or reduced motion leaves the row exactly where layout puts
    /// it.
    pub fn begin_for(node: &Node, offset: f32, axis: ScrollAxis) {
        if offset == 0.0 || node.prefers_reduced_motion() {
            return;
        }
        let shift = Self::new();
        shift.offset.set(offset);
        shift.axis.set(axis);
        node.add_resource(shift);
    }

    /// Paints the row at its slot plus the current displacement.
    fn place(&self) {
        let node = self
            .node
            .borrow()
            .as_ref()
            .and_then(Node::upgrade)
            .filter(|node| node.parent().is_some());
        let Some(node) = node else { return };
        let origin = self.origin.get();
        let progress = if self.running.get() {
            self.release.get()
        } else {
            self.offset.get()
        };
        let frame = match self.axis.get() {
            ScrollAxis::Horizontal => {
                Rect::new(origin.x + progress, origin.y, origin.width, origin.height)
            }
            _ => Rect::new(origin.x, origin.y + progress, origin.width, origin.height),
        };
        node.set_frame(frame);
    }

    fn tick(&self, delta_time: f64) {
        if !self.running.get() || !delta_time.is_finite() || delta_time < 0.0 {
            return;
        }
        self.elapsed.set(self.elapsed.get() + delta_time);
        let spring = reorder_spring();
        let duration = spring.settling_duration();
        if self.elapsed.get() >= duration {
            self.finish();
            return;
        }
        let t = self.elapsed.get() as f32;
        self.release
            .set(spring.position(t, self.offset.get(), 0.0, 0.0));
        self.place();
    }

    fn finish(&self) {
        if self.running.get() {
            self.running.set(false);
            self.release.set(0.0);
            self.place();
        }
        if let (Some(scheduler), Some(controller)) = (
            self.scheduler.borrow().as_ref(),
            self.controller.borrow().as_ref(),
        ) {
            scheduler.unregister(controller);
        }
        self.controller.borrow_mut().take();
    }
}

/// Forwards animator ticks to the row shift without keeping it alive.
struct RowShiftController {
    shift: Weak<ReorderRowShift>,
}

impl AnyAnimationController for RowShiftController {
    fn is_finished(&self) -> bool {
        self.shift
            .upgrade()
            .is_none_or(|shift| !shift.running.get())
    }

    fn tick(&self, delta_time: f64) {
        if let Some(shift) = self.shift.upgrade() {
            shift.tick(delta_time);
        }
    }

    fn finish_immediately(&self) {
        if let Some(shift) = self.shift.upgrade() {
            shift.finish();
        }
    }

    fn cancel(&self) {
        self.finish_immediately();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl NodeResource for ReorderRowShift {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = Some(node.downgrade());
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
        // The slot frame is already resolved; draw the row one stride away
        // and spring it home. Every later layout pass resets the frame to
        // the slot, so the hook re-places the row while the slide runs.
        let frame = node.frame();
        self.origin.set(frame);
        self.release.set(self.offset.get());
        self.running.set(true);
        self.place();
        let controller: Rc<dyn AnyAnimationController> = Rc::new(RowShiftController {
            shift: self.self_ref.borrow().clone(),
        });
        *self.controller.borrow_mut() = Some(controller.clone());
        if let Some(scheduler) = self.scheduler.borrow().as_ref() {
            scheduler.register(controller);
        }
        let weak_self = self.self_ref.borrow().clone();
        node.set_layout_did_update(Some(
            Rc::new(move |owner: &Node| {
                let Some(shift) = weak_self.upgrade() else {
                    return;
                };
                if shift.running.get() {
                    shift.place();
                } else {
                    shift.origin.set(owner.frame());
                }
            }) as LayoutUpdateHook,
        ));
    }

    fn unmount(&self, node: &Node) {
        self.finish();
        node.set_layout_did_update(None);
        *self.scheduler.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
